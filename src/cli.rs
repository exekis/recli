use clap::{Parser, Subcommand};

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
    /// Start capturing terminal session
    Start,
    
    /// Stop current capturing session
    Stop,
    
    /// Show status of recli daemon
    Status,
    
    /// Show recent command history
    Recent {
        /// Number of recent commands to show
        #[arg(short = 'n', long, default_value = "10")]
        count: usize,
    },
    
    /// Clear command history
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
    pub fn handle_command(&self) -> Result<(), Box<dyn std::error::Error>> {
        match &self.command {
            RecliCommands::Start => {
                self.verbose_print("Starting recli session...");
                self.handle_start()
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

    fn handle_start(&self) -> Result<(), Box<dyn std::error::Error>> {
        // TODO: Start PTY session and command logging
        println!("Starting recli session... (TODO: implement)");
        Ok(())
    }

    fn handle_stop(&self) -> Result<(), Box<dyn std::error::Error>> {
        // TODO: Stop running session
        println!("Stopping recli session... (TODO: implement)");
        Ok(())
    }

    fn handle_status(&self) -> Result<(), Box<dyn std::error::Error>> {
        // TODO: Check if session is running
        println!("Recli status... (TODO: implement)");
        Ok(())
    }

    fn handle_recent(&self, count: usize) -> Result<(), Box<dyn std::error::Error>> {
        // TODO: Show recent commands from log
        println!("Showing {} recent commands... (TODO: implement)", count);
        Ok(())
    }

    fn handle_clear(&self) -> Result<(), Box<dyn std::error::Error>> {
        // TODO: Clear command history
        println!("Clearing command history... (TODO: implement)");
        Ok(())
    }
}
