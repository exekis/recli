use crate::error::Result;
use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommandEntry {
    pub cmd: String, // command
    pub cwd: String, // current working directory
    pub timestamp: String,
    pub exit_code: i32,
    pub output_preview: String,
    pub output_path: Option<String>,
    pub pipestatus: Option<Vec<i32>>,
    pub duration_ms: Option<u64>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CommandLog {
    pub entries: Vec<CommandEntry>,
    #[serde(skip)]
    pub current_cmd: String,
    #[serde(skip)]
    pub current_preview: String,
    #[serde(skip)]
    pub current_start_time: Option<std::time::Instant>,
    #[serde(skip)]
    pub current_out_file: Option<(PathBuf, std::fs::File)>,
}

// >>> methods >>>

impl CommandLog {
    pub fn new() -> CommandLog {
        CommandLog {
            entries: Vec::new(),
            current_cmd: String::new(),
            current_preview: String::new(),
            current_start_time: None,
            current_out_file: None,
        }
    }

    pub fn start_command(&mut self, cmd_string: String, _cwd: String, log_dir: &Path) {
        self.current_cmd = cmd_string;
        self.current_preview = String::new();
        self.current_start_time = Some(std::time::Instant::now());
        // open a temp file to stream raw bytes, will rename on finish
        let tmp = log_dir.join("current.out");
        match std::fs::File::create(&tmp) {
            Ok(f) => self.current_out_file = Some((tmp, f)),
            Err(_) => self.current_out_file = None,
        }
    }

    pub fn append_output(&mut self, output: &str) {
        // legacy path if used elsewhere: write to preview and file
        self.append_output_bytes(output.as_bytes());
    }

    pub fn append_output_bytes(&mut self, bytes: &[u8]) {
        if let Some((_, f)) = self.current_out_file.as_mut() {
            let _ = f.write_all(bytes);
            let _ = f.flush();
        }
        // build a small utf-8 preview, capped
        if self.current_preview.len() < 8 * 1024 {
            let remaining = 8 * 1024 - self.current_preview.len();
            let snippet = String::from_utf8_lossy(&bytes[..bytes.len().min(remaining)]);
            self.current_preview.push_str(&snippet);
        }
    }

    pub fn finish_command(&mut self, exit_code: i32, pipestatus: Option<Vec<i32>>, cwd: String, log_dir: &Path) {
        // avoid creating empty entries if no command was started
        if self.current_cmd.is_empty() {
            return;
        }
    // use rfc3339 utc to be cosmos-ready and schema-stable
    let timestamp = Utc::now().to_rfc3339();

        let duration_ms = self
            .current_start_time
            .map(|start| start.elapsed().as_millis() as u64);

        // finalize sidecar file
        let mut output_path: Option<String> = None;
        if let Some((tmp_path, mut f)) = self.current_out_file.take() {
            let _ = f.flush();
            let seq = self.entries.len();
            let filename = format!("{}-{}.out", timestamp.replace(':', "-"), seq);
            let final_path = log_dir.join(filename);
            let _ = std::fs::rename(&tmp_path, &final_path);
            output_path = Some(final_path.file_name().unwrap_or_default().to_string_lossy().to_string());
        }

        let entry = CommandEntry {
            cmd: self.current_cmd.clone(),
            cwd,
            timestamp,
            exit_code,
            output_preview: self.current_preview.clone(),
            output_path,
            pipestatus,
            duration_ms,
        };

        self.entries.push(entry);
        self.current_cmd = String::new();
        self.current_preview = String::new();
        self.current_start_time = None;
        self.current_out_file = None;
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
    self.current_preview = String::new();
        self.current_start_time = None;
    self.current_out_file = None;
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
    log.current_preview = String::new();
    log.current_start_time = None;
    log.current_out_file = None;

        Ok(log)
    }
}
