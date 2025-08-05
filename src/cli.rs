use clap::{Parser, Subcommand};
use crate::session::SessionManager;
use crate::logging_pty::LoggingPtySession;

/// CLI configuration for Recli
#[derive(Parser, Debug, Clone)]
#[command(name = "recli")]
#[command(author, version, about, long_about = None)]
pub struct Cli {
    /// enable verbose output
    #[arg(short, long, global = true)]
    pub verbose: bool,

    /// path to config file
    #[arg(short, long, global = true)]
    pub config: Option<String>,

    /// shell to use (defaults to user's default shell)
    #[arg(short, long, global = true)]
    pub shell: Option<String>,

    #[command(subcommand)]
    pub command: RecliCommands,
}

#[derive(Subcommand, Debug, Clone)]
pub enum RecliCommands {
    /// start capturing terminal session
    Start,
    
    /// stop current capturing session
    Stop,
    
    /// show status of recli daemon
    Status,
    
    /// show recent command history
    Recent {
        /// number of recent commands to show
        #[arg(short = 'n', long, default_value = "10")]
        count: usize,
    },
    
    /// clear command history
    Clear,
}

impl Cli {
    /// parse command line arguments
    pub fn parse_args() -> Self {
        Cli::parse()
    }

    /// get the shell to use with fallback logic
    pub fn get_shell(&self) -> String {
        self.shell.clone().unwrap_or_else(|| {
            std::env::var("SHELL").unwrap_or_else(|_| "/bin/bash".to_string())
        })
    }

    /// print startup information if verbose mode is enabled
    pub fn print_startup_info(&self, shell: &str) {
        if self.verbose {
            println!("Recli CLI Enhancer Starting...");
            println!("  Verbose mode: enabled");
            println!("  Config file: {}", self.config.as_deref().unwrap_or("None"));
            println!("  Shell: {}", shell);
            println!("Starting PTY session...");
        }
    }

    /// print verbose message if verbose mode is enabled
    pub fn verbose_print(&self, message: &str) {
        if self.verbose {
            println!("{}", message);
        }
    }

    /// handle the subcommand execution
    pub async fn handle_command(&self) -> Result<(), Box<dyn std::error::Error>> {
        match &self.command {
            RecliCommands::Start => {
                self.verbose_print("Starting recli session...");
                self.handle_start().await
            },
            RecliCommands::Stop => {
                self.verbose_print("Stopping recli session...");
                self.handle_stop()
            },
            RecliCommands::Status => {
                self.verbose_print("Checking recli status...");
                self.handle_status()
            },
            RecliCommands::Recent { count } => {
                self.verbose_print(&format!("Showing {} recent commands...", count));
                self.handle_recent(*count)
            },
            RecliCommands::Clear => {
                self.verbose_print("Clearing command history...");
                self.handle_clear()
            },
        }
    }

    async fn handle_start(&self) -> Result<(), Box<dyn std::error::Error>> {
        let mut session_manager = SessionManager::new();
        
        if session_manager.is_session_active() {
            println!("session already active");
            return Ok(());
        }

        let shell = self.get_shell();
        self.print_startup_info(&shell);
        
        let config = session_manager.start_session(&shell, self.verbose)?;
        println!("session started with id: {}", config.session_id);
        println!("logs will be saved to: {}", config.log_dir.display());
        
        // start logging pty session
        let mut logging_pty = LoggingPtySession::new(self.verbose, session_manager);
        logging_pty.run(&shell).await?;
        
        Ok(())
    }

    fn handle_stop(&self) -> Result<(), Box<dyn std::error::Error>> {
        let mut session_manager = SessionManager::new();
        
        if !session_manager.is_session_active() {
            println!("no active session");
            return Ok(());
        }

        if let Some(log_dir) = session_manager.stop_session()? {
            println!("session stopped successfully");
            println!("all terminal commands and outputs saved to: {}", log_dir.display());
        } else {
            println!("no session was active");
        }
        
        Ok(())
    }

    fn handle_status(&self) -> Result<(), Box<dyn std::error::Error>> {
        let session_manager = SessionManager::new();
        println!("{}", session_manager.get_status());
        Ok(())
    }

    fn handle_recent(&self, count: usize) -> Result<(), Box<dyn std::error::Error>> {
        let session_manager = SessionManager::new();
        
        if !session_manager.is_session_active() {
            println!("no active session");
            return Ok(());
        }

        // TODO: load recent commands from current session log
        println!("showing {} recent commands... (TODO: implement loading from active session)", count);
        Ok(())
    }

    fn handle_clear(&self) -> Result<(), Box<dyn std::error::Error>> {
        let session_manager = SessionManager::new();
        
        if !session_manager.is_session_active() {
            println!("no active session");
            return Ok(());
        }

        // TODO: clear current session log
        println!("clearing command history... (TODO: implement for active session)");
        Ok(())
    }
}
