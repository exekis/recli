use crate::command_log::CommandLog;
use crate::error::{RecliError, Result};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use tokio::sync::mpsc;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionConfig {
    pub session_id: String,
    pub log_dir: PathBuf,
    pub started_at: String,
    pub shell: String,
}

#[derive(Debug)]
pub struct SessionManager {
    config: Option<SessionConfig>,
    command_log: Arc<Mutex<CommandLog>>,
    pid_file: PathBuf,
    log_sender: Option<mpsc::UnboundedSender<LogEvent>>,
}

#[derive(Debug, Clone)]
pub enum LogEvent {
    CommandStart { cmd: String, cwd: String },
    Output { data: String },
    CommandEnd { exit_code: i32, cwd: String },
}

impl SessionManager {
    pub fn new() -> Self {
        let home_dir = std::env::var("HOME").unwrap_or_else(|_| "/tmp".to_string());
        let pid_file = Path::new(&home_dir).join(".recli").join("session.pid");
        
        Self {
            config: None,
            command_log: Arc::new(Mutex::new(CommandLog::new())),
            pid_file,
            log_sender: None,
        }
    }

    pub fn is_session_active(&self) -> bool {
        if !self.pid_file.exists() {
            return false;
        }

        // check if pid file contains a valid running process
        if let Ok(pid_str) = fs::read_to_string(&self.pid_file) {
            if let Ok(pid) = pid_str.trim().parse::<u32>() {
                // check if process is still running
                return self.process_exists(pid);
            }
        }
        false
    }

    pub fn start_session(&mut self, shell: &str, verbose: bool) -> Result<SessionConfig> {
        if self.is_session_active() {
            return Err(RecliError::Session("session already active".to_string()));
        }

        // create session directory
        let session_id = self.generate_session_id();
        let log_dir = self.create_log_directory(&session_id)?;
        
        let config = SessionConfig {
            session_id: session_id.clone(),
            log_dir: log_dir.clone(),
            started_at: chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string(),
            shell: shell.to_string(),
        };

        // create pid file directory if it doesn't exist
        if let Some(parent) = self.pid_file.parent() {
            fs::create_dir_all(parent)?;
        }

        // write current process pid to file
        let pid = std::process::id();
        fs::write(&self.pid_file, pid.to_string())?;

        // set up logging channel
        let (tx, mut rx) = mpsc::unbounded_channel();
        self.log_sender = Some(tx);

        let command_log = Arc::clone(&self.command_log);
        let config_clone = config.clone();

        // spawn logging task
        tokio::spawn(async move {
            while let Some(event) = rx.recv().await {
                let mut log = command_log.lock().unwrap();
                match event {
                    LogEvent::CommandStart { cmd, cwd } => {
                        log.start_command(cmd, cwd);
                    }
                    LogEvent::Output { data } => {
                        log.append_output(&data);
                    }
                    LogEvent::CommandEnd { exit_code, cwd } => {
                        log.finish_command(exit_code, cwd);
                        // save to file after each command
                        if let Err(e) = log.save_to_file(&config_clone.log_dir) {
                            eprintln!("failed to save command log: {}", e);
                        }
                    }
                }
            }
        });

        self.config = Some(config.clone());
        
        if verbose {
            println!("session started with id: {}", session_id);
            println!("logs will be saved to: {}", log_dir.display());
        }

        Ok(config)
    }

    pub fn stop_session(&mut self) -> Result<Option<PathBuf>> {
        if !self.is_session_active() {
            return Ok(None);
        }

        let log_dir = self.config.as_ref().map(|c| c.log_dir.clone());

        // save final log
        if let Some(config) = &self.config {
            let log = self.command_log.lock().unwrap();
            log.save_to_file(&config.log_dir)?;
            
            // save session metadata
            let metadata_file = config.log_dir.join("session_metadata.json");
            let metadata = serde_json::to_string_pretty(config)?;
            fs::write(metadata_file, metadata)?;
        }

        // cleanup
        if self.pid_file.exists() {
            fs::remove_file(&self.pid_file)?;
        }
        
        self.config = None;
        self.log_sender = None;

        Ok(log_dir)
    }

    pub fn get_status(&self) -> String {
        if let Some(config) = &self.config {
            format!(
                "active session: {}\nstarted: {}\nlog directory: {}",
                config.session_id,
                config.started_at,
                config.log_dir.display()
            )
        } else if self.is_session_active() {
            "session is active but config not loaded".to_string()
        } else {
            "no active session".to_string()
        }
    }

    pub fn send_log_event(&self, event: LogEvent) {
        if let Some(sender) = &self.log_sender {
            let _ = sender.send(event);
        }
    }

    fn generate_session_id(&self) -> String {
        let now = chrono::Local::now();
        format!("recli_session_{}", now.format("%Y%m%d_%H%M%S"))
    }

    fn create_log_directory(&self, session_id: &str) -> Result<PathBuf> {
        let home_dir = std::env::var("HOME").unwrap_or_else(|_| "/tmp".to_string());
        let base_dir = Path::new(&home_dir).join(".recli").join("logs");
        let log_dir = base_dir.join(session_id);
        
        fs::create_dir_all(&log_dir)?;
        Ok(log_dir)
    }

    fn process_exists(&self, pid: u32) -> bool {
        // on unix systems check if process exists by sending signal 0
        #[cfg(unix)]
        {
            unsafe {
                libc::kill(pid as i32, 0) == 0
            }
        }
        
        #[cfg(not(unix))]
        {
            // fallback for non-unix systems
            false
        }
    }
}
