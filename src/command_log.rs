use crate::error::Result;
use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommandEntry {
    pub cmd: String, // command
    pub cwd: String, // current working directory
    pub timestamp: String,
    pub exit_code: i32,
    pub output: String,
    pub duration_ms: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommandLog {
    pub entries: Vec<CommandEntry>,
    #[serde(skip)]
    pub current_cmd: String,
    #[serde(skip)]
    pub current_output: String,
    #[serde(skip)]
    pub current_start_time: Option<std::time::Instant>,
}

// >>> methods >>>

impl CommandLog {
    pub fn new() -> CommandLog {
        CommandLog {
            entries: Vec::new(),
            current_cmd: String::new(),
            current_output: String::new(),
            current_start_time: None,
        }
    }

    pub fn start_command(&mut self, cmd_string: String, _cwd: String) {
        self.current_cmd = cmd_string;
        self.current_output = String::new();
        self.current_start_time = Some(std::time::Instant::now());
    }

    pub fn append_output(&mut self, output: &str) {
        // if no active command, start a synthetic one so output is not lost
        if self.current_cmd.is_empty() {
            // best effort cwd
            let cwd = std::env::current_dir()
                .map(|p| p.to_string_lossy().to_string())
                .unwrap_or_else(|_| "/unknown".to_string());
            self.start_command("<captured>".to_string(), cwd);
        }
        self.current_output.push_str(output);
    }

    pub fn finish_command(&mut self, exit_code: i32, cwd: String) {
    // use rfc3339 utc to be cosmos-ready and schema-stable
    let timestamp = Utc::now().to_rfc3339();

        let duration_ms = self
            .current_start_time
            .map(|start| start.elapsed().as_millis() as u64);

        let entry = CommandEntry {
            cmd: self.current_cmd.clone(),
            cwd,
            timestamp,
            exit_code,
            output: self.current_output.clone(),
            duration_ms,
        };

        self.entries.push(entry);
        self.current_cmd = String::new();
        self.current_output = String::new();
        self.current_start_time = None;
    }

    pub fn get_recent(&self, count: usize) -> Vec<&CommandEntry> {
        let start = if self.entries.len() > count {
            self.entries.len() - count
        } else {
            0
        };
        self.entries[start..].iter().collect()
    }

    pub fn get_all(&self) -> &Vec<CommandEntry> {
        &self.entries
    }

    pub fn clear(&mut self) {
        self.entries.clear();
        self.current_cmd = String::new();
        self.current_output = String::new();
        self.current_start_time = None;
    }

    /// push a pending command into entries if one is in progress
    pub fn force_flush(&mut self, cwd: String) {
        if !self.current_cmd.is_empty() {
            // finish with exit code 0 by default
            self.finish_command(0, cwd);
        }
    }

    pub fn save_to_file(&self, log_dir: &Path) -> Result<()> {
        let commands_file = log_dir.join("commands.json");
        let json_data = serde_json::to_string_pretty(self)?;
        fs::write(commands_file, json_data)?;
        Ok(())
    }

    pub fn load_from_file(log_dir: &Path) -> Result<CommandLog> {
        let commands_file = log_dir.join("commands.json");
        if !commands_file.exists() {
            return Ok(CommandLog::new());
        }

        let json_data = fs::read_to_string(commands_file)?;
        let mut log: CommandLog = serde_json::from_str(&json_data)?;

        // initialize non-serialized fields
        log.current_cmd = String::new();
        log.current_output = String::new();
        log.current_start_time = None;

        Ok(log)
    }
}
