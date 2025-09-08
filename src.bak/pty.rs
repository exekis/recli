use crate::error::{RecliError, Result};
use crate::io::OutputHandler;
use crate::session::SessionManager;
use crate::command_detector::CommandDetector;
use crossterm::{
    event::{self, Event},
    terminal::{disable_raw_mode, enable_raw_mode},
};
use portable_pty::{CommandBuilder, PtySize};
use std::io::{Read, Write};
use std::sync::{Arc, Mutex};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

/// PTY session with a shell
pub struct PtySession {
    verbose: bool,
    // hold the session manager so we can emit log events and persist on exit
    session_manager: Option<Arc<Mutex<SessionManager>>>,
    // set when we receive a termination signal to end the loop
    terminated: Arc<AtomicBool>,
}

impl PtySession {
    /// new PTY session
    pub fn new(verbose: bool) -> Self {
        Self {
            verbose,
            session_manager: None,
            terminated: Arc::new(AtomicBool::new(false)),
        }
    }

    /// new PTY session with command logging
    pub fn new_with_logging(verbose: bool, session_manager: SessionManager) -> Self {
    let session_manager = Arc::new(Mutex::new(session_manager));

        Self {
            verbose,
            session_manager: Some(session_manager),
            terminated: Arc::new(AtomicBool::new(false)),
        }
    }

    /// start and run the PTY session
    pub async fn run(&mut self, shell: &str) -> Result<()> {
        self.verbose_print(&format!("Starting PTY session with shell: {}", shell));

        // create PTY system and get terminal size
        let pty_system = portable_pty::native_pty_system();
        let pty_size = self.get_terminal_size()?;

        // create PTY pair and spawn shell
        let pty_pair = pty_system
            .openpty(pty_size)
            .map_err(|e| RecliError::Pty(e.into()))?;

        let mut cmd = CommandBuilder::new(shell);
        // ensure child shell is interactive so it displays a prompt and processes commands
        cmd.cwd(std::env::current_dir()?);
        if let Ok(term) = std::env::var("TERM") {
            cmd.env("TERM", term);
        }

        // launch zsh with a controlled zdotdir so our hook always loads without touching user config
        if shell.contains("zsh") {
            match Self::ensure_zsh_bootstrap_files() {
                Ok(zdotdir) => {
                    // rebuild command to force zsh to read $ZDOTDIR/.zshrc
                    let mut z = CommandBuilder::new("zsh");
                    // preserve environment like TERM
                    if let Ok(term) = std::env::var("TERM") {
                        z.env("TERM", term);
                    }
                    if let Ok(cwd) = std::env::current_dir() {
                        z.cwd(cwd);
                    }
                    z.env("ZDOTDIR", zdotdir.to_string_lossy().to_string());
                    // enable debug marker emission from hook when verbose
                    if self.verbose {
                        z.env("RECLI_DEBUG_MARKERS", "1");
                    }
                    // show which zdotdir we are using in verbose mode for easy verification
                    self.verbose_print(&format!("Using ZDOTDIR: {}", zdotdir.display()));
                    z.arg("-i");
                    cmd = z;
                }
                Err(e) => {
                    self.verbose_print(&format!("failed to prepare zsh bootstrap files: {}", e));
                    // fallback to interactive shell
                    cmd.arg("-i");
                }
            }
        } else {
            // fallback to user's shell interactively
            cmd.arg("-i");
            // enable debug marker emission from hook when verbose
            if self.verbose {
                cmd.env("RECLI_DEBUG_MARKERS", "1");
            }
        }

        let mut child = pty_pair
            .slave
            .spawn_command(cmd)
            .map_err(|e| RecliError::Pty(e.into()))?;

        self.verbose_print(&format!(
            "PTY session started with PID: {:?}",
            child.process_id()
        ));

        // set up terminal for raw input
        enable_raw_mode().map_err(|e| RecliError::Terminal(format!("{:?}", e.kind())))?;

        // get PTY handles
        let mut pty_reader = pty_pair
            .master
            .try_clone_reader()
            .map_err(|e| RecliError::Pty(e.into()))?;
        let mut pty_writer = pty_pair
            .master
            .take_writer()
            .map_err(|e| RecliError::Pty(e.into()))?;

        // forward raw stdin bytes to the pty to preserve all control/meta sequences and ime input
        // this avoids lossy translation of key events
        let mut stdin = std::io::stdin();
        std::thread::spawn(move || {
            let mut buf = [0u8; 8192];
            loop {
                match stdin.read(&mut buf) {
                    Ok(0) => break, // eof
                    Ok(n) => {
                        let _ = pty_writer.write_all(&buf[..n]);
                    }
                    Err(_) => break,
                }
            }
        });

        // spawn background task for pty output and run it through the command detector
        // this allows us to infer command boundaries from prompts and capture full output reliably
        let sm_for_output = self.session_manager.clone();
        // create a shared detector instance with optional debug
        let detector_shared: Option<Arc<Mutex<CommandDetector>>> = sm_for_output
            .as_ref()
            .map(|sm| Arc::new(Mutex::new(CommandDetector::new_with_debug(sm.clone(), self.verbose))));

        let detector_for_output = detector_shared.clone();
        let verbose_flag = self.verbose;
        let output_task = tokio::spawn(async move {
            let mut buffer = [0u8; 8192];
            loop {
                match pty_reader.read(&mut buffer) {
                    Ok(0) => {
                        // eof - shell exited
                        if verbose_flag {
                            eprintln!("[PTY] EOF detected, shell exited");
                        }
                        // flush any active command so it gets recorded
                        if let Some(det) = &detector_for_output {
                            if let Ok(mut det) = det.lock() {
                                det.finish();
                            }
                        }
                        break;
                    }
                    Ok(n) => {
                        // feed through detector to emit structured events while preserving original output
                        if verbose_flag {
                            eprintln!("[PTY] Read {} bytes from PTY", n);
                        }
                        let processed = if let Some(det) = &detector_for_output {
                            if let Ok(mut det) = det.lock() {
                                det.process_output(&buffer[..n])
                            } else {
                                OutputHandler::process_output(&buffer[..n])
                            }
                        } else {
                            OutputHandler::process_output(&buffer[..n])
                        };
                        if OutputHandler::forward_to_stdout(&processed).is_err() {
                            break;
                        }
                    }
                    Err(e) => {
                        if verbose_flag {
                            eprintln!("[PTY] Error reading from PTY: {}", e);
                        }
                        break;
                    }
                }
            }
        });

        // listen for sigterm to end gracefully
        #[cfg(unix)]
        {
            let term_flag = self.terminated.clone();
            tokio::spawn(async move {
                if let Ok(mut sig) = tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate()) {
                    let _ = sig.recv().await;
                    term_flag.store(true, Ordering::Relaxed);
                    eprintln!("[RECLI] received termination signal, ending session...");
                }
            });
        }

        // input handling loop
        let result = self
            .input_loop(&mut child, &pty_pair)
            .await;

        // cleanup
        disable_raw_mode().map_err(|e| RecliError::Terminal(format!("{:?}", e.kind())))?;
        // flush the detector to end any open command before aborting output task
        if let Some(det) = &detector_shared {
            if let Ok(mut det) = det.lock() {
                det.finish();
            }
        }
        output_task.abort();

    // persist logs by stopping the session when we own it
            if let Some(sm) = &self.session_manager {
                if let Ok(mut sm) = sm.lock() {
                    if let Ok(Some(log_dir)) = sm.stop_session_async().await {
                        println!("\rsession ended, logs saved to: {}", log_dir.display());
                    }
                }
            }

        self.verbose_print("PTY session ended");
        result
    }

    /// input handling loop
    async fn input_loop(
        &mut self,
        child: &mut Box<dyn portable_pty::Child + Send + Sync>,
        pty_pair: &portable_pty::PtyPair,
    ) -> Result<()> {
        loop {
            // honor termination flag set by sigterm handler
            if self.terminated.load(Ordering::Relaxed) {
                break;
            }
            // if shell process is still alive
            if let Ok(Some(exit_status)) = child.try_wait() {
                self.verbose_print(&format!(
                    "Shell process exited with status: {:?}",
                    exit_status
                ));
                break;
            }

            // poll for input events (nonblocking with timeout)
            if event::poll(Duration::from_millis(50))
                .map_err(|e| RecliError::Terminal(format!("{:?}", e.kind())))?
            {
                match event::read().map_err(|e| RecliError::Terminal(format!("{:?}", e.kind())))? {
                    Event::Key(_key_event) => {
                        // ignore key events here, raw stdin forwarder handles input
                    }
                    Event::Resize(cols, rows) => {
                        self.handle_resize(cols, rows, pty_pair)?;
                    }
                    Event::Mouse(_) => {
                        // ignore mouse events for now
                    }
                    Event::FocusGained | Event::FocusLost => {
                        // ignore focus events
                    }
                    Event::Paste(_text) => {
                        // ignore explicit paste events; raw stdin forwarder already sends bytes
                    }
                }
            }
        }
        Ok(())
    }

    /// handle terminal resize events
    fn handle_resize(&self, cols: u16, rows: u16, pty_pair: &portable_pty::PtyPair) -> Result<()> {
        let new_size = PtySize {
            rows,
            cols,
            pixel_width: 0,
            pixel_height: 0,
        };

        pty_pair
            .master
            .resize(new_size)
            .map_err(|e| RecliError::Pty(e.into()))?;

        self.verbose_print(&format!("Terminal resized to {}x{}", cols, rows));
        Ok(())
    }

    /// get current terminal size
    fn get_terminal_size(&self) -> Result<PtySize> {
        let (cols, rows) = crossterm::terminal::size()
            .map_err(|e| RecliError::Terminal(format!("{:?}", e.kind())))?;

        Ok(PtySize {
            rows,
            cols,
            pixel_width: 0,
            pixel_height: 0,
        })
    }

    /// print verbose message if verbose mode is enabled
    fn verbose_print(&self, message: &str) {
        if self.verbose {
            eprintln!("[RECLI] {}", message);
        }
    }

    // command lifecycle is managed by command_detector from pty output
}

impl PtySession {
        fn ensure_zsh_bootstrap_files() -> std::io::Result<std::path::PathBuf> {
            use std::fs;
                use std::path::PathBuf;

                let home = std::env::var("HOME").unwrap_or_else(|_| "/tmp".to_string());
                let dir = PathBuf::from(&home).join(".recli");
                fs::create_dir_all(&dir)?;

                                // hook file that emits markers to stderr; always write latest version
                                let hook = dir.join("recli.zsh");
                                let hook_content = r#"# --- recli hook (v5) ---
                # state tracking
                typeset -g RECLI_INITIALIZED=0
                typeset -g RECLI_IN_COMMAND=0
                typeset -g RECLI_LAST_CMD=""

                # emit marker to stderr
                function _recli_emit() {
                    local marker="$1"
                    printf '\x1e%s\n' "$marker" >&2
                }

                # preexec: start of command
                function _recli_preexec() {
                    if (( RECLI_INITIALIZED == 1 )); then
                        RECLI_IN_COMMAND=1
                        RECLI_LAST_CMD="$1"
                        _recli_emit "RECLI_START:$1"
                    fi
                }

                # precmd: before prompt shows
                function _recli_precmd() {
                    local exit_code=$?
                    local -a ps=("${pipestatus[@]}")
                    if (( RECLI_INITIALIZED == 0 )); then
                        RECLI_INITIALIZED=1
                        return 0
                    fi
                    if (( RECLI_IN_COMMAND == 1 )); then
                        RECLI_IN_COMMAND=0
                        _recli_emit "RECLI_END:$exit_code"
                        _recli_emit "RECLI_PIPE:[${(j:,:)ps}]"
                        _recli_emit "RECLI_PWD:$PWD"
                    fi
                }

                # register hooks safely: remove old and add new
                typeset -ag precmd_functions
                typeset -ag preexec_functions
                precmd_functions=("${(@)precmd_functions:#_recli_precmd}")
                preexec_functions=("${(@)preexec_functions:#_recli_preexec}")
                precmd_functions=(_recli_precmd ${precmd_functions})
                preexec_functions+=(_recli_preexec)

                # optional debug marker
                if [[ -n "${RECLI_DEBUG_MARKERS:-}" ]]; then
                    _recli_emit "RECLI_DEBUG:hook_loaded_v5"
                fi
                "#;
                                fs::write(&hook, hook_content)?;

            // bootstrap .zshrc: user's ~/.zshrc FIRST, then our hook LAST; always write latest version
                let bootstrap = dir.join(".zshrc");
            let bootstrap_content = r#"# --- recli bootstrap .zshrc (v3) ---
        # source user config first (including p10k instant prompt)
        [[ -r ~/.zshrc ]] && source ~/.zshrc

        # give p10k a moment to finish initialization if present
        if typeset -f p10k &>/dev/null; then
            sleep 0.1
        fi

        # load recli hooks last
        [[ -r ~/.recli/recli.zsh ]] && source ~/.recli/recli.zsh

        # debug: print arrays to stderr if requested
        if [[ -n "${RECLI_DEBUG_MARKERS:-}" ]]; then
            typeset -p precmd_functions preexec_functions >&2
        fi
        "#;
            fs::write(&bootstrap, bootstrap_content)?;

                // remove compiled zsh caches that could shadow fresh text
                let _ = fs::remove_file(dir.join(".zshrc.zwc"));
                let _ = fs::remove_file(dir.join("recli.zsh.zwc"));

                Ok(dir)
        }
}
