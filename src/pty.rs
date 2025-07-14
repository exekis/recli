use crate::error::{RecliError, Result};
use crate::io::{InputHandler, OutputHandler};
use crossterm::{
    event::{self, Event},
    terminal::{disable_raw_mode, enable_raw_mode},
};
use portable_pty::{CommandBuilder, PtySize};
use std::io::{Read, Write};
use std::time::Duration;

/// PTY session with a shell
pub struct PtySession {
    verbose: bool,
}

impl PtySession {
    /// new PTY session
    pub fn new(verbose: bool) -> Self {
        Self { verbose }
    }

    /// start and run the PTY session
    pub async fn run(&self, shell: &str) -> Result<()> {
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
        let output_task = tokio::spawn(async move {
            let mut buffer = [0u8; 8192];
            loop {
                match pty_reader.read(&mut buffer) {
                    Ok(0) => break, // EOF - shell exited
                    Ok(n) => {
                        let processed = OutputHandler::process_output(&buffer[..n]);
                        if OutputHandler::forward_to_stdout(&processed).is_err() {
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

        self.verbose_print("PTY session ended");
        result
    }

    /// input handling loop
    async fn input_loop(
        &self,
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
}
