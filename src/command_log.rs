use clap::Command;
use serde::Serialize;
use serde::Deserialize;
use std::alloc::System;
use std::time::{SystemTime, UNIX_EPOCH};
use chrono::Local;


#[derive(Debug, Clone, Serialize, Deserialize)]
struct CommandEntry {
    pub cmd: String, // command
    pub cwd: String, // current working directory
    pub timestamp: String,
    pub exit_code: i32,
    pub output_string: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct CommandLog {
    pub cmd_entries: Vec<CommandEntry>,
    pub curr_cmd: String,
    pub curr_output: String,
}

// >>> methods >>>

impl CommandLog {
        
    fn new() -> CommandLog {
        let cmd_log = CommandLog{cmd_entries: Vec::new(), curr_cmd: String::new(), curr_output: String::new()};
        cmd_log
    }

    fn start_command(&mut self, cmd_string: String, cwd: String) {
        self.curr_cmd = cmd_string;
        self.curr_output = String::new();
    }

    fn append_output(&mut self, output: &str) {
        self.curr_output.push_str(output);
    }

    fn finish_command(&mut self, exit_code: i32, cwd: String) {
        let now = Local::now();
        let timestamp = now.format("%Y-%m-%d %H:%M:%S").to_string();
        let entry = CommandEntry{cmd: self.curr_cmd.clone(), cwd: cwd, timestamp: timestamp, exit_code: exit_code, output_string: self.curr_output.clone()};
        self.cmd_entries.push(entry);
        self.curr_cmd = String::new();
        self.curr_output = String::new();
    }

    // // >>> helper methods >>>
    // fn get_recent() -> {
    //     // TODO: implement get_recent
    //     todo!()
    // }

    // fn get_all() -> {
    //     // TODO: implement get_all
    //     todo!()
    // }

    // fn clear() -> {
    //     // TODO: implement clear
    //     todo!()
    // }

    // fn clear() -> {
    //     // TODO: implement clear (duplicate)
    //     todo!()
    // }

    // fn save_to_file() -> {
    //     // TODO: implement save_to_file
    //     todo!()
    // }

    // fn load_from_file() -> {
    //     // TODO: implement load_from_file
    //     todo!()
    // }

}
