use crate::command_detector::CommandDetector;
use crate::error::{RecliError, Result};
use crate::io::{InputHandler, OutputHandler};
use crate::session::{SessionManager, LogEvent};
use crossterm::{
    event::{self, Event},
    terminal::{disable_raw_mode, enable_raw_mode},
};
use portable_pty::{CommandBuilder, PtySize};
use regex::Regex;
use std::sync::atomic::{AtomicBool, Ordering};
use std::io::{Read, Write};
use std::sync::{Arc, Mutex};
use std::time::Duration;

/// PTY session with a shell
pub struct PtySession {
    verbose: bool,
    command_detector: Option<Arc<Mutex<CommandDetector>>>,
    // capture currently typed input to delineate commands on enter
    current_input: String,
    // hold the session manager so we can emit log events and persist on exit
    session_manager: Option<Arc<Mutex<SessionManager>>>,
    // track the echoed current line from pty output
    echo_line: Arc<Mutex<String>>,
    // whether a command is currently active (between enter and next prompt)
    in_command: Arc<AtomicBool>,
}

impl PtySession {
    /// new PTY session
    pub fn new(verbose: bool) -> Self {
        Self {
            verbose,
            command_detector: None,
            current_input: String::new(),
            session_manager: None,
            echo_line: Arc::new(Mutex::new(String::new())),
            in_command: Arc::new(AtomicBool::new(false)),
        }
    }

    /// new PTY session with command logging
    pub fn new_with_logging(verbose: bool, session_manager: SessionManager) -> Self {
        let session_manager = Arc::new(Mutex::new(session_manager));
        let command_detector = Arc::new(Mutex::new(CommandDetector::new(session_manager.clone())));

        Self {
            verbose,
            command_detector: Some(command_detector),
            current_input: String::new(),
            session_manager: Some(session_manager),
            echo_line: Arc::new(Mutex::new(String::new())),
            in_command: Arc::new(AtomicBool::new(false)),
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
        cmd.cwd(std::env::current_dir()?);

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

        // spawn background task for PTY output
        let session_manager = self.session_manager.clone();
        let in_command = self.in_command.clone();
        let echo_line = self.echo_line.clone();
        let output_task = tokio::spawn(async move {
            let mut buffer = [0u8; 8192];
            // prompt detection regex (common ascii and powerline prompts)
            let prompt_re = Regex::new(r"([\$%#>]|❯|➜|)\s*$").unwrap();
            loop {
                match pty_reader.read(&mut buffer) {
                    Ok(0) => break, // EOF - shell exited
                    Ok(n) => {
                        let processed = OutputHandler::process_output(&buffer[..n]);
                        if OutputHandler::forward_to_stdout(&processed).is_err() {
                            break;
                        }

                        // forward output to log only when a command is active
                        if in_command.load(Ordering::SeqCst) {
                            if let Some(sm) = &session_manager {
                                if let Ok(sm) = sm.lock() {
                                    let text = String::from_utf8_lossy(&processed).to_string();
                                    sm.send_log_event(LogEvent::Output { data: text });
                                }
                            }
                        }

                        // update echo line and detect prompts
                        let text = String::from_utf8_lossy(&processed);
            for ch in text.chars() {
                            match ch {
                '\r' => { /* ignore */ }
                '\n' => {
                                    // on newline, check if previous line looked like a prompt
                                    let line_snapshot = {
                                        let s = echo_line.lock().unwrap();
                                        s.clone()
                                    };
                                    let clean = strip_ansi(&line_snapshot);
                                    if prompt_re.is_match(&clean) {
                                        // prompt printed: previous command likely finished
                                        if in_command.swap(false, Ordering::SeqCst) {
                                            if let Some(sm) = &session_manager {
                                                if let Ok(sm) = sm.lock() {
                                                    // we do not know exit code reliably
                                                    let cwd = std::env::current_dir()
                                                        .map(|p| p.to_string_lossy().to_string())
                                                        .unwrap_or_else(|_| "/unknown".to_string());
                                                    sm.send_log_event(LogEvent::CommandEnd { exit_code: 0, cwd });
                                                }
                                            }
                                        }
                                    }
                                    // reset line after newline
                                    if let Ok(mut s) = echo_line.lock() { s.clear(); }
                                }
                                '\x08' | '\x7f' => { // backspace/delete
                                    if let Ok(mut s) = echo_line.lock() { s.pop(); }
                                }
                                c => {
                                    if let Ok(mut s) = echo_line.lock() { s.push(c); }
                                }
                            }
                        }
                    }
                    Err(_) => break,
                }
            }
        });

        // input handling loop
        let result = self
            .input_loop(&mut child, &mut pty_writer, &pty_pair)
            .await;

        // cleanup
        disable_raw_mode().map_err(|e| RecliError::Terminal(format!("{:?}", e.kind())))?;
        output_task.abort();

        // persist logs by stopping the session when we own it
        if let Some(sm) = &self.session_manager {
            if let Ok(mut sm) = sm.lock() {
                // if a command is still open, close it to persist
                if self.in_command.swap(false, Ordering::SeqCst) {
                    let cwd = std::env::current_dir()
                        .map(|p| p.to_string_lossy().to_string())
                        .unwrap_or_else(|_| "/unknown".to_string());
                    sm.send_log_event(LogEvent::CommandEnd { exit_code: 0, cwd });
                }
                if let Ok(Some(log_dir)) = sm.stop_session() {
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
        pty_writer: &mut Box<dyn Write + Send>,
        pty_pair: &portable_pty::PtyPair,
    ) -> Result<()> {
        loop {
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
                    Event::Key(key_event) => {
                        // check for ctrl+x termination hotkey
                        if key_event.modifiers.contains(crossterm::event::KeyModifiers::CONTROL) {
                            if let crossterm::event::KeyCode::Char('x') = key_event.code {
                                println!("\r\n[RECLI] Session terminated by user (Ctrl+X)");
                                break;
                            }
                        }
                        // also handle control character 0x18 that some terminals send for ctrl+x
                        if let crossterm::event::KeyCode::Char(c) = key_event.code {
                            if c as u32 == 0x18 {
                                println!("\r\n[RECLI] Session terminated by user (Ctrl+X)");
                                break;
                            }
                        }
                        // capture text for current command and delimit on enter
                        if let crossterm::event::KeyCode::Enter = key_event.code {
                            self.log_command_start_if_ready();
                            // reset input buffer after logging
                            // this mirrors behavior in logging_pty
                            self.current_input.clear();
                        } else if let crossterm::event::KeyCode::Char(c) = key_event.code {
                            self.current_input.push(c);
                        } else if let crossterm::event::KeyCode::Backspace = key_event.code {
                            self.current_input.pop();
                        }

                        self.handle_key_event(key_event, pty_writer)?;
                    }
                    Event::Resize(cols, rows) => {
                        self.handle_resize(cols, rows, pty_pair)?;
                    }
                    Event::Mouse(_) => {
                        // gIgnore mouse events for now
                    }
                    Event::FocusGained | Event::FocusLost => {
                        // ignore focus events
                    }
                    Event::Paste(text) => {
                        // handle paste events
                        pty_writer.write_all(text.as_bytes())?;
                    }
                }
            }
        }
        Ok(())
    }

    /// handle a key event by converting it to PTY input
    fn handle_key_event(
        &self,
        key_event: crossterm::event::KeyEvent,
        pty_writer: &mut Box<dyn Write + Send>,
    ) -> Result<()> {
        if let Some(bytes) = InputHandler::key_to_bytes(key_event) {
            pty_writer.write_all(&bytes)?;
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
    
    /// log the accumulated input as a command on enter
    fn log_command_start_if_ready(&self) {
        // first try to extract the command from the on-screen line after the prompt
        let screen_line = self
            .echo_line
            .lock()
            .ok()
            .map(|s| strip_ansi(&s).trim().to_string())
            .unwrap_or_default();
        let mut effective_cmd = extract_cmd_after_prompt(&screen_line);
        if effective_cmd.is_empty() {
            // fallback to typed buffer
            effective_cmd = self.current_input.trim().to_string();
        }
        if effective_cmd.is_empty() { return; }

        let cwd = std::env::current_dir()
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_else(|_| "/unknown".to_string());

        if let Some(sm) = &self.session_manager {
            if let Ok(sm) = sm.lock() {
                sm.send_log_event(LogEvent::CommandStart { cmd: effective_cmd, cwd });
                // mark in-command; we will end on next detected prompt
                self.in_command.store(true, Ordering::SeqCst);
            }
        }
    }
}

// remove ansi escape codes for prompt detection
fn strip_ansi(input: &str) -> String {
    let re = Regex::new(r"\x1B\[[0-9;]*[ -/]*[@-~]").unwrap();
    re.replace_all(input, "").into_owned()
}

// try to take the content after a typical prompt ending char
fn extract_cmd_after_prompt(line: &str) -> String {
    // look for last occurrence of prompt enders
    let markers = ["$", "%", "#", ">", "❯", "➜", ""]; 
    let mut idx: Option<usize> = None;
    for m in markers.iter() {
        if let Some(i) = line.rfind(m) {
            idx = Some(match idx {
                Some(cur) => if i > cur { i } else { cur },
                None => i,
            });
        }
    }
    if let Some(i) = idx {
        // take text after marker and any following space
        let tail = &line[i+1..];
        return tail.trim_start().to_string();
    }
    // if no marker, return full line (it might be a bare input line)
    line.to_string()
}
