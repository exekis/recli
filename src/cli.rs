use crate::pty::PtySession;
use crate::session::SessionManager;
use clap::{Parser, Subcommand};
use crate::schema::{log_event::LogEventV1, validation::validate_event};
use chrono::{DateTime, Local, Utc, NaiveDateTime, TimeZone};
use std::fs;
use std::path::PathBuf;
use hostname;
use crate::config::Config;
use crate::util::telemetry;

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

    /// validate local logs against canonical schema
    Validate {
        /// path to a session log directory or base logs dir (defaults to ~/.recli/logs)
        #[arg(short, long)]
        path: Option<String>,
    },

    /// show effective configuration (env + file)
    Config,
}

impl Cli {
    /// parse command line arguments
    pub fn parse_args() -> Self {
        Cli::parse()
    }

    /// get the shell to use with fallback logic
    pub fn get_shell(&self) -> String {
        self.shell
            .clone()
            .unwrap_or_else(|| std::env::var("SHELL").unwrap_or_else(|_| "/bin/bash".to_string()))
    }

    /// print startup information if verbose mode is enabled
    pub fn print_startup_info(&self, shell: &str) {
        if self.verbose {
            println!("Recli CLI Enhancer Starting...");
            println!("  Verbose mode: enabled");
            println!(
                "  Config file: {}",
                self.config.as_deref().unwrap_or("None")
            );
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
    // initialize config and telemetry first
    let cfg = Config::load(self.config.as_deref());
    telemetry::init(&cfg.logging.level);
        match &self.command {
            RecliCommands::Start => {
                self.verbose_print("Starting recli session...");
                self.handle_start().await
            }
            RecliCommands::Stop => {
                self.verbose_print("Stopping recli session...");
                self.handle_stop()
            }
            RecliCommands::Status => {
                self.verbose_print("Checking recli status...");
                self.handle_status()
            }
            RecliCommands::Recent { count } => {
                self.verbose_print(&format!("Showing {} recent commands...", count));
                self.handle_recent(*count)
            }
            RecliCommands::Clear => {
                self.verbose_print("Clearing command history...");
                self.handle_clear()
            }
            RecliCommands::Validate { path } => {
                self.verbose_print("Validating logs against schema...");
                self.handle_validate(path.as_deref())
            }
            RecliCommands::Config => {
                println!("{}", serde_json::to_string_pretty(&cfg)?);
                Ok(())
            }
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

    // start logging pty session with prompt-based detector
    let mut pty = PtySession::new_with_logging(self.verbose, session_manager);
    pty.run(&shell).await?;

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
            println!(
                "all terminal commands and outputs saved to: {}",
                log_dir.display()
            );
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
        println!(
            "showing {} recent commands... (TODO: implement loading from active session)",
            count
        );
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

    fn handle_validate(&self, path: Option<&str>) -> Result<(), Box<dyn std::error::Error>> {
        let base_dir = if let Some(p) = path {
            PathBuf::from(p)
        } else {
            let home = std::env::var("HOME").unwrap_or_else(|_| "/tmp".to_string());
            PathBuf::from(home).join(".recli").join("logs")
        };

        let (session_dirs, single) = if base_dir.join("commands.json").exists() {
            (vec![base_dir.clone()], true)
        } else {
            let mut dirs = Vec::new();
            if base_dir.exists() {
                for entry in fs::read_dir(&base_dir)? {
                    let entry = entry?;
                    if entry.file_type()?.is_dir() && entry.path().join("commands.json").exists() {
                        dirs.push(entry.path());
                    }
                }
            }
            (dirs, false)
        };

        if session_dirs.is_empty() {
            println!("no session logs found at {}", base_dir.display());
            return Ok(());
        }

        let mut total = 0usize;
        let mut valid = 0usize;
        let mut invalid = 0usize;

        for dir in session_dirs {
            let commands_path = dir.join("commands.json");
            let meta_path = dir.join("session_metadata.json");

            let session_meta: Option<crate::session::SessionConfig> = fs::read_to_string(&meta_path)
                .ok()
                .and_then(|s| serde_json::from_str(&s).ok());

            let host = hostname::get()
                .ok()
                .and_then(|h| h.into_string().ok())
                .unwrap_or_else(|| "unknown-host".to_string());

            if let Ok(text) = fs::read_to_string(&commands_path) {
                if let Ok(cmd_log) = serde_json::from_str::<crate::command_log::CommandLog>(&text) {
                    for (idx, entry) in cmd_log.entries.iter().enumerate() {
                        total += 1;
                        let ts_rfc3339 = normalize_to_rfc3339(&entry.timestamp)
                            .unwrap_or_else(|| Utc::now().to_rfc3339());

                        let session_id = session_meta
                            .as_ref()
                            .map(|m| m.session_id.clone())
                            .unwrap_or_else(|| "unknown-session".to_string());

                        let id = LogEventV1::make_id(
                            &host,
                            &session_id,
                            &ts_rfc3339,
                            &entry.cmd,
                            &idx.to_string(),
                        );

                        let event = LogEventV1 {
                            id,
                            schema_version: 1,
                            timestamp: ts_rfc3339,
                            host: host.clone(),
                            app: "recli".to_string(),
                            session_id,
                            level: if entry.exit_code == 0 { "INFO".into() } else { "ERROR".into() },
                            command: entry.cmd.clone(),
                            exit_code: Some(entry.exit_code),
                            error_type: None,
                            message: entry.output.clone(),
                            tags: vec![],
                            raw: None,
                        };

                        match validate_event(&event) {
                            Ok(_) => valid += 1,
                            Err(e) => {
                                invalid += 1;
                                println!(
                                    "invalid event in {}: {} — {}",
                                    dir.display(),
                                    entry.cmd,
                                    e
                                );
                            }
                        }
                    }
                }
            }
        }

        println!(
            "validation complete: total={}, valid={}, invalid={}",
            total, valid, invalid
        );
        if single && invalid > 0 {
            return Err("validation failed for some records".into());
        }
        Ok(())
    }
}

fn normalize_to_rfc3339(ts: &str) -> Option<String> {
    // try parse as rfc3339 first
    if let Ok(dt) = DateTime::parse_from_rfc3339(ts) {
        return Some(dt.with_timezone(&Utc).to_rfc3339());
    }
    // try common local format used earlier: "%Y-%m-%d %H:%M:%S"
    if let Ok(naive) = NaiveDateTime::parse_from_str(ts, "%Y-%m-%d %H:%M:%S") {
        // attempt to interpret as local time
        if let Some(local_dt) = Local.from_local_datetime(&naive).single() {
            return Some(local_dt.with_timezone(&Utc).to_rfc3339());
        }
        // fallback to treat as utc if local is ambiguous
        let utc_dt = chrono::DateTime::<Utc>::from_naive_utc_and_offset(naive, Utc);
        return Some(utc_dt.to_rfc3339());
    }
    None
}
