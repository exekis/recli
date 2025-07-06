use clap::Parser;

/// Recli: lightweight, emulator-agnostic CLI enhancer
#[derive(Parser)]
#[command(name = "recli")]
#[command(author, version, about, long_about = None)]
struct Cli {
    /// verbose output
    #[arg(short, long)]
    verbose: bool,

    /// config file path
    #[arg(short, long)]
    config: Option<String>,

    /// port number to use
    #[arg(short, long, default_value = "8080")]
    port: u16,

    /// number of threads
    #[arg(short = 'j', long, default_value = "1")]
    threads: usize,
}

fn main() {
    let args = Cli::parse(); // parse CLI arguments with clap
    
    if args.verbose {
        println!("Verbose mode enabled!");
    }
    
    println!("Recli initialized with the following settings:");
    println!("  Verbose: {}", args.verbose);
    println!("  Config file: {}", args.config.as_deref().unwrap_or("None"));
    println!("  Port: {}", args.port);
    println!("  Threads: {}", args.threads);
    
    println!("\nReady to build your hotkey hooks and PTY wrapper.");
}
