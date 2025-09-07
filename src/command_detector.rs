use crate::session::{LogEvent, SessionManager};
use regex::Regex;
use std::collections::VecDeque;
use std::sync::{Arc, Mutex};

#[derive(Debug)]
pub struct CommandDetector {
    prompt_patterns: Vec<Regex>,
    current_line: String,
    output_buffer: VecDeque<String>,
    in_command: bool,
    current_command: Option<String>,
    session_manager: Arc<Mutex<SessionManager>>,
}

impl CommandDetector {
    pub fn new(session_manager: Arc<Mutex<SessionManager>>) -> Self {
        // common shell prompt patterns
        let prompt_patterns = vec![
            // zsh/bash prompts: typically end with $ or %
            Regex::new(r"[$%#>]\s*$").unwrap(),
            // more specific patterns
            Regex::new(r"\$\s*$").unwrap(), // bash
            Regex::new(r"%\s*$").unwrap(),  // zsh
            Regex::new(r"#\s*$").unwrap(),  // root
            Regex::new(r">\s*$").unwrap(),  // windows or custom
        ];

        Self {
            prompt_patterns,
            current_line: String::new(),
            output_buffer: VecDeque::new(),
            in_command: false,
            current_command: None,
            session_manager,
        }
    }

    pub fn process_output(&mut self, data: &[u8]) -> Vec<u8> {
        // convert bytes to string, handling partial UTF-8 carefully
        let text = String::from_utf8_lossy(data);

        for ch in text.chars() {
            match ch {
                '\r' => {
                    // carriage return - might be part of \r\n
                    continue;
                }
                '\n' => {
                    // line feed - process the complete line
                    self.process_line();
                    self.current_line.clear();
                }
                '\x08' | '\x7f' => {
                    // backspace or delete - remove last character
                    self.current_line.pop();
                }
                c if c.is_control() => {
                    // other control characters - typically ANSI escape sequences
                    // for now, just add them to preserve terminal formatting
                    self.current_line.push(c);
                }
                c => {
                    // regular character
                    self.current_line.push(c);
                }
            }
        }

        // return the original data unchanged for terminal display
        data.to_vec()
    }

    fn process_line(&mut self) {
        let line = self.current_line.trim().to_string(); // clone to avoid borrow conflicts

        if line.is_empty() {
            return;
        }

        // check if this looks like a command prompt
        if self.is_prompt_line(&line) {
            self.handle_prompt_line(&line);
        } else if self.in_command {
            // this is command output
            self.handle_command_output(&line);
        }

        // keep recent output for context
        self.output_buffer.push_back(line);
        if self.output_buffer.len() > 100 {
            self.output_buffer.pop_front();
        }
    }

    fn is_prompt_line(&self, line: &str) -> bool {
        // remove ANSI escape sequences for pattern matching
        let clean_line = self.strip_ansi_codes(line);

        for pattern in &self.prompt_patterns {
            if pattern.is_match(&clean_line) {
                return true;
            }
        }
        false
    }

    fn handle_prompt_line(&mut self, line: &str) {
        // if we were in a command, finish it
        if self.in_command {
            self.finish_current_command();
        }

        // extract command from prompt line
        if let Some(command) = self.extract_command_from_prompt(line) {
            self.start_new_command(command);
        }
    }

    fn extract_command_from_prompt(&self, line: &str) -> Option<String> {
        // try to extract the command part from a prompt line
        // this is tricky because prompts vary widely
        let clean_line = self.strip_ansi_codes(line);

        // look for common patterns: prompt ends with $ % # > followed by command
        for pattern in &self.prompt_patterns {
            if let Some(captures) = pattern.find(&clean_line) {
                let after_prompt = &clean_line[captures.end()..].trim();
                if !after_prompt.is_empty() {
                    return Some(after_prompt.to_string());
                }
            }
        }

        // if no pattern matched but line doesn't look like output, treat as command
        if !clean_line.contains("error") && !clean_line.contains("warning") {
            // simple heuristic: if it starts with a word, might be a command
            let parts: Vec<&str> = clean_line.split_whitespace().collect();
            if !parts.is_empty() && !parts[0].starts_with('[') {
                return Some(clean_line.to_string());
            }
        }

        None
    }

    fn start_new_command(&mut self, command: String) {
        self.in_command = true;
        self.current_command = Some(command.clone());

        // get current working directory (best effort)
        let cwd = std::env::current_dir()
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_else(|_| "/unknown".to_string());

        // send log event
        if let Ok(session_manager) = self.session_manager.lock() {
            session_manager.send_log_event(LogEvent::CommandStart { cmd: command, cwd });
        }
    }

    fn handle_command_output(&mut self, line: &str) {
        // send output to logger
        if let Ok(session_manager) = self.session_manager.lock() {
            session_manager.send_log_event(LogEvent::Output {
                data: format!("{}\n", line),
            });
        }
    }

    fn finish_current_command(&mut self) {
        if let Some(_command) = &self.current_command {
            // get current working directory
            let cwd = std::env::current_dir()
                .map(|p| p.to_string_lossy().to_string())
                .unwrap_or_else(|_| "/unknown".to_string());

            // TODO: try to detect exit code (very difficult without shell integration)
            let exit_code = 0; // assume success for now

            // send log event
            if let Ok(session_manager) = self.session_manager.lock() {
                session_manager.send_log_event(LogEvent::CommandEnd { exit_code, cwd });
            }
        }

        self.in_command = false;
        self.current_command = None;
    }

    fn strip_ansi_codes(&self, text: &str) -> String {
        // simple ANSI escape sequence removal
        // this is a basic implementation - could be more comprehensive
        let ansi_regex = Regex::new(r"\x1b\[[0-9;]*[a-zA-Z]").unwrap();
        ansi_regex.replace_all(text, "").to_string()
    }
}
