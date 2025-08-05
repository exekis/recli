use crate::error::{RecliError, Result};
use crate::io::{InputHandler, OutputHandler};
use crate::session::{LogEvent, SessionManager};
use crossterm::{
    event::{self, Event, KeyCode, KeyModifiers},
    terminal::{disable_raw_mode, enable_raw_mode},
};
use portable_pty::{CommandBuilder, PtySize};
use std::io::{Read, Write};
use std::sync::{Arc, Mutex};
use std::time::Duration;

/// enhanced PTY session with logging
pub struct LoggingPtySession {
    verbose: bool,
    session_manager: Arc<Mutex<SessionManager>>,
    current_input: String,
    output_buffer: String,
}

impl LoggingPtySession {
    pub fn new(verbose: bool, session_manager: SessionManager) -> Self {
        Self {
            verbose,
            session_manager: Arc::new(Mutex::new(session_manager)),
            current_input: String::new(),
            output_buffer: String::new(),
        }
    }

    pub async fn run(&mut self, shell: &str) -> Result<()> {
        self.verbose_print(&format!("Starting logging PTY session with shell: {}", shell));

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

        self.verbose_print(&format!("PTY session started with PID: {:?}", child.process_id()));

        // set up terminal for raw input
        enable_raw_mode()
            .map_err(|e| RecliError::Terminal(format!("{:?}", e.kind())))?;

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
        let session_manager_clone = Arc::clone(&self.session_manager);
        let output_task = tokio::spawn(async move {
            let mut buffer = [0u8; 8192];
            loop {
                match pty_reader.read(&mut buffer) {
                    Ok(0) => break, // EOF - shell exited
                    Ok(n) => {
                        let data = &buffer[..n];
                        let text = String::from_utf8_lossy(data);
                        
                        // log all output
                        if let Ok(session_manager) = session_manager_clone.lock() {
                            session_manager.send_log_event(LogEvent::Output { 
                                data: text.to_string() 
                            });
                        }
                        
                        // forward to stdout
                        if OutputHandler::forward_to_stdout(data).is_err() {
                            break;
                        }
                    }
                    Err(_) => break,
                }
            }
        });

        // input handling loop
        let result = self.input_loop(&mut child, &mut pty_writer, &pty_pair).await;

        // cleanup
        disable_raw_mode()
            .map_err(|e| RecliError::Terminal(format!("{:?}", e.kind())))?;
        output_task.abort();

        // stop session and save logs
        if let Ok(mut session_manager) = self.session_manager.lock() {
            if let Ok(Some(log_dir)) = session_manager.stop_session() {
                println!("\rsession ended, logs saved to: {}", log_dir.display());
            }
        }

        self.verbose_print("PTY session ended");
        result
    }

    async fn input_loop(
        &mut self,
        child: &mut Box<dyn portable_pty::Child + Send + Sync>,
        pty_writer: &mut Box<dyn Write + Send>,
        pty_pair: &portable_pty::PtyPair,
    ) -> Result<()> {
        loop {
            // if shell process is still alive
            if let Ok(Some(exit_status)) = child.try_wait() {
                self.verbose_print(&format!("Shell process exited with status: {:?}", exit_status));
                break;
            }

            // poll for input events (nonblocking with timeout)
            if event::poll(Duration::from_millis(50))
                .map_err(|e| RecliError::Terminal(format!("{:?}", e.kind())))?
            {
                match event::read().map_err(|e| RecliError::Terminal(format!("{:?}", e.kind())))? {
                    Event::Key(key_event) => {
                        // check for Ctrl+X to exit
                        if key_event.modifiers.contains(KeyModifiers::CONTROL) {
                            if let KeyCode::Char('x') = key_event.code {
                                println!("\r\n[RECLI] Session terminated by user (Ctrl+X)");
                                break;
                            }
                        }

                        // check for Enter to log command
                        if let KeyCode::Enter = key_event.code {
                            self.log_command_if_ready();
                            self.current_input.clear();
                        } else if let KeyCode::Char(c) = key_event.code {
                            self.current_input.push(c);
                        } else if let KeyCode::Backspace = key_event.code {
                            self.current_input.pop();
                        }

                        self.handle_key_event(key_event, pty_writer)?;
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
                    Event::Paste(text) => {
                        // handle paste events
                        self.current_input.push_str(&text);
                        pty_writer.write_all(text.as_bytes())?;
                    }
                }
            }
        }
        Ok(())
    }

    fn log_command_if_ready(&self) {
        let cmd = self.current_input.trim();
        if !cmd.is_empty() {
            let cwd = std::env::current_dir()
                .map(|p| p.to_string_lossy().to_string())
                .unwrap_or_else(|_| "/unknown".to_string());

            if let Ok(session_manager) = self.session_manager.lock() {
                session_manager.send_log_event(LogEvent::CommandStart { 
                    cmd: cmd.to_string(), 
                    cwd: cwd.clone()
                });
                
                // immediately end the command with success (we can't easily detect real exit codes)
                session_manager.send_log_event(LogEvent::CommandEnd { 
                    exit_code: 0, 
                    cwd 
                });
            }
        }
    }

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

    fn handle_resize(
        &self,
        cols: u16,
        rows: u16,
        pty_pair: &portable_pty::PtyPair,
    ) -> Result<()> {
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

    fn verbose_print(&self, message: &str) {
        if self.verbose {
            eprintln!("[RECLI] {}", message);
        }
    }
}
