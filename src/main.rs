use recli::cli::Cli;
use recli::error::Result;

#[tokio::main]
async fn main() -> Result<()> {
    // parse command line arguments
    let cli = Cli::parse_args();

    // handle the subcommand
    if let Err(e) = cli.handle_command().await {
        eprintln!("Error: {}", e);
        std::process::exit(1);
    }

    Ok(())
}
