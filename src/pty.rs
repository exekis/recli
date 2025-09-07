use crate::command_detector::CommandDetector;
use crate::error::{RecliError, Result};
use crate::io::{InputHandler, OutputHandler};
use crate::session::{SessionManager, LogEvent};
use crossterm::{
    event::{self, Event},
    terminal::{disable_raw_mode, enable_raw_mode},
};
use portable_pty::{CommandBuilder, PtySize};
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
}

impl PtySession {
    /// new PTY session
    pub fn new(verbose: bool) -> Self {
        Self {
            verbose,
            command_detector: None,
            current_input: String::new(),
            session_manager: None,
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
        let command_detector = self.command_detector.clone();
        let output_task = tokio::spawn(async move {
            let mut buffer = [0u8; 8192];
            loop {
                match pty_reader.read(&mut buffer) {
                    Ok(0) => break, // EOF - shell exited
                    Ok(n) => {
                        let mut processed = buffer[..n].to_vec();

                        // if we have command detection, process through it
                        if let Some(detector) = &command_detector {
                            if let Ok(mut detector) = detector.lock() {
                                processed = detector.process_output(&processed);
                            }
                        } else {
                            processed = OutputHandler::process_output(&processed);
                        }

                        if OutputHandler::forward_to_stdout(&processed).is_err() {
                            break;
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
                        // capture text for current command and delimit on enter
                        if let crossterm::event::KeyCode::Enter = key_event.code {
                            self.log_command_if_ready();
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
    fn log_command_if_ready(&self) {
        let cmd = self.current_input.trim();
        if cmd.is_empty() {
            return;
        }

        let cwd = std::env::current_dir()
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_else(|_| "/unknown".to_string());

        if let Some(sm) = &self.session_manager {
            if let Ok(sm) = sm.lock() {
                sm.send_log_event(LogEvent::CommandStart { cmd: cmd.to_string(), cwd: cwd.clone() });
                // end immediately since we cannot reliably capture exit status here
                sm.send_log_event(LogEvent::CommandEnd { exit_code: 0, cwd });
            }
        }
    }
}
