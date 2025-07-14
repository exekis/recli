use recli::cli::Cli;
use recli::error::Result;
use recli::pty::PtySession;

#[tokio::main]
async fn main() -> Result<()> {
    // parse command line arguments
    let cli = Cli::parse_args();
    
    // get shell to use
    let shell = cli.get_shell();
    
    // print startup information
    cli.print_startup_info(&shell);
    
    // create and run PTY session
    let pty_session = PtySession::new(cli.verbose);
    pty_session.run(&shell).await?;
    
    Ok(())
}
