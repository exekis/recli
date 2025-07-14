use clap::Parser;

/// CLI configuration for Recli
#[derive(Parser, Debug, Clone)]
#[command(name = "recli")]
#[command(author, version, about, long_about = None)]
pub struct Cli {
    /// enable verbose output
    #[arg(short, long)]
    pub verbose: bool,

    /// path to config file
    #[arg(short, long)]
    pub config: Option<String>,

    /// shell to use (defaults to user's default shell)
    #[arg(short, long)]
    pub shell: Option<String>,
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
}
