use crate::session::{LogEvent, SessionManager};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

// streaming scanner that preserves bytes and strips recli markers without corrupting utf-8
#[derive(Debug)]
pub struct CommandDetector {
    // holds tail bytes if a marker starts near the end of a chunk
    partial_marker: Option<Vec<u8>>,
    in_command: bool,
    // suppress logging until first newline after start to avoid echoing the typed line
    skip_until_eol: bool,
    // time when skip started to avoid eating real output if no newline arrives
    skip_started_at: Option<Instant>,
    pending_exit_code: Option<i32>,
    pending_pipestatus: Option<Vec<i32>>,
    pending_pwd: Option<String>,
    last_pwd: Option<String>,
    session_manager: Arc<Mutex<SessionManager>>,
}

impl CommandDetector {
    pub fn new(session_manager: Arc<Mutex<SessionManager>>) -> Self {
        Self {
            partial_marker: None,
            in_command: false,
            skip_until_eol: false,
            skip_started_at: None,
            pending_exit_code: None,
            pending_pipestatus: None,
            pending_pwd: None,
            last_pwd: None,
            session_manager,
        }
    }

    /// byte-preserving pass-through with in-band marker stripping
    pub fn process_output(&mut self, data: &[u8]) -> Vec<u8> {
        // stitch any partial marker from last time
        let mut buf = Vec::with_capacity(
            self.partial_marker.as_ref().map(|v| v.len()).unwrap_or(0) + data.len(),
        );
        if let Some(mut tail) = self.partial_marker.take() {
            buf.append(&mut tail);
        }
        buf.extend_from_slice(data);

        let mut out: Vec<u8> = Vec::with_capacity(buf.len());
        let mut i = 0;
    while i < buf.len() {
            if buf[i] == 0x1e {
                // found rs: look for newline or cr that ends the marker
                let mut j = i + 1;
                while j < buf.len() && buf[j] != b'\n' && buf[j] != b'\r' {
                    j += 1;
                }
                if j >= buf.len() {
                    // incomplete marker: stash and stop
                    self.partial_marker = Some(buf[i..].to_vec());
                    break;
                }

                // parse marker payload between i+1 .. j as ascii
                let marker = String::from_utf8_lossy(&buf[i + 1..j]);
                self.handle_marker(&marker);

                // skip marker and its line ending
                i = j + 1;
                continue;
            }

            // if a command just started, drop everything until we hit the first newline
            if self.in_command && self.skip_until_eol {
                // stop skipping either at newline or after a small grace window
                let timed_out = self
                    .skip_started_at
                    .map(|t| t.elapsed() >= Duration::from_millis(50))
                    .unwrap_or(false);
                if buf[i] == b'\n' || buf[i] == b'\r' || timed_out {
                    self.skip_until_eol = false;
                    self.skip_started_at = None;
                } else {
                    i += 1;
                    continue;
                }
            }

            // normal byte; keep exact
            out.push(buf[i]);
            i += 1;
        }

        // stream-log display bytes during an active command
    if self.in_command && !out.is_empty() {
            if let Ok(sm) = self.session_manager.lock() {
        sm.send_log_event(LogEvent::Output { data: out.clone() });
            }
        }

        out
    }

    pub fn finish(&mut self) {
        self.partial_marker = None;
        if self.in_command {
            let cwd = self.last_pwd.clone().unwrap_or_else(|| {
                std::env::current_dir()
                    .map(|p| p.to_string_lossy().to_string())
                    .unwrap_or_else(|_| "/unknown".to_string())
            });
            self.send_end_event(0, cwd);
            self.in_command = false;
            self.pending_exit_code = None;
            self.pending_pwd = None;
        }
    }

    fn handle_marker(&mut self, marker: &str) {
        if let Some(rest) = marker.strip_prefix("RECLI_START:") {
            self.start_command(rest.to_string());
            return;
        }
        if let Some(rest) = marker.strip_prefix("RECLI_END:") {
            self.pending_exit_code = rest.trim().parse::<i32>().ok();
            self.try_finish_when_ready();
            return;
        }
        if let Some(rest) = marker.strip_prefix("RECLI_PWD:") {
            let pwd = rest.to_string();
            self.pending_pwd = Some(pwd.clone());
            self.last_pwd = Some(pwd);
            self.try_finish_when_ready();
            return;
        }
        if let Some(rest) = marker.strip_prefix("RECLI_PIPE:") {
            // expect format like: [0,1,0]
            let s = rest.trim();
            if let Some(inner) = s.strip_prefix('[').and_then(|x| x.strip_suffix(']')) {
                let mut v = Vec::new();
                for part in inner.split(',') {
                    if let Ok(n) = part.trim().parse::<i32>() {
                        v.push(n);
                    }
                }
                if !v.is_empty() {
                    self.pending_pipestatus = Some(v);
                    self.try_finish_when_ready();
                }
            }
            return;
        }
        // ignore unknown markers
    }

    fn start_command(&mut self, cmd: String) {
        if self.in_command {
            let cwd = self.last_pwd.clone().unwrap_or_else(|| {
                std::env::current_dir()
                    .map(|p| p.to_string_lossy().to_string())
                    .unwrap_or_else(|_| "/unknown".to_string())
            });
            self.send_end_event(0, cwd);
        }
    self.in_command = true;
    self.skip_until_eol = true;
    self.skip_started_at = Some(Instant::now());
        self.pending_exit_code = None;
    self.pending_pipestatus = None;
        self.pending_pwd = None;

        let cwd = self.last_pwd.clone().unwrap_or_else(|| {
            std::env::current_dir()
                .map(|p| p.to_string_lossy().to_string())
                .unwrap_or_else(|_| "/unknown".to_string())
        });
        if let Ok(sm) = self.session_manager.lock() {
            sm.send_log_event(LogEvent::CommandStart { cmd, cwd });
        }
    }

    fn try_finish_when_ready(&mut self) {
        if self.in_command {
            if let (Some(ec), Some(pwd)) = (self.pending_exit_code, self.pending_pwd.clone()) {
                let pipe = self.pending_pipestatus.clone();
                self.send_end_event_with_pipe(ec, pipe, pwd);
                self.in_command = false;
                self.pending_exit_code = None;
                self.pending_pipestatus = None;
                self.pending_pwd = None;
            }
        }
    }

    fn send_end_event(&mut self, exit_code: i32, cwd: String) {
        if let Ok(sm) = self.session_manager.lock() {
            sm.send_log_event(LogEvent::CommandEnd { exit_code, pipestatus: None, cwd });
        }
    }

    fn send_end_event_with_pipe(&mut self, exit_code: i32, pipestatus: Option<Vec<i32>>, cwd: String) {
        if let Ok(sm) = self.session_manager.lock() {
            sm.send_log_event(LogEvent::CommandEnd { exit_code, pipestatus, cwd });
        }
    }
}
