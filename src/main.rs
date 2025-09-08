use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::env;
use std::fs;
use std::io::{self, Write};
use std::path::PathBuf;
use std::process::Command;
use std::time::Instant;

#[derive(Debug, Clone, Serialize, Deserialize)]
struct CommandEntry {
    cmd: String,
    exit_code: i32,
    output: String,
    stderr: String,
    cwd: String,
    timestamp: String,
    duration_ms: u64,
}

#[derive(Debug, Serialize, Deserialize)]
struct CommandLog {
    entries: Vec<CommandEntry>,
}

struct CommandLogger {
    session_id: String,
    log_dir: PathBuf,
    entries: Vec<CommandEntry>,
}

impl CommandLogger {
    fn new() -> io::Result<Self> {
        let session_id = chrono::Local::now().format("%Y%m%d_%H%M%S").to_string();
        let home = env::var("HOME").unwrap_or_else(|_| "/tmp".to_string());
        let log_dir = PathBuf::from(home)
            .join(".recli")
            .join("logs")
            .join(&session_id);
        
        fs::create_dir_all(&log_dir)?;
        
        Ok(CommandLogger {
            session_id,
            log_dir,
            entries: Vec::new(),
        })
    }
    
    fn run_command(&mut self, cmd: &str) -> i32 {
        let cwd = env::current_dir()
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_else(|_| String::from("/"));
        
        let timestamp = Utc::now().to_rfc3339();
        let start = Instant::now();
        
        // special handling for cd command
        if cmd.trim().starts_with("cd ") {
            let path = cmd.trim()[3..].trim();
            let target = if path.is_empty() {
                env::var("HOME").unwrap_or_else(|_| String::from("/"))
            } else {
                path.to_string()
            };
            
            match env::set_current_dir(shellexpand::tilde(&target).as_ref()) {
                Ok(_) => {
                    let new_cwd = env::current_dir()
                        .map(|p| p.to_string_lossy().to_string())
                        .unwrap_or_else(|_| String::from("/"));
                    
                    self.entries.push(CommandEntry {
                        cmd: cmd.to_string(),
                        exit_code: 0,
                        output: String::new(),
                        stderr: String::new(),
                        cwd: new_cwd,
                        timestamp,
                        duration_ms: start.elapsed().as_millis() as u64,
                    });
                    return 0;
                }
                Err(e) => {
                    eprintln!("cd: {}", e);
                    self.entries.push(CommandEntry {
                        cmd: cmd.to_string(),
                        exit_code: 1,
                        output: String::new(),
                        stderr: format!("cd: {}", e),
                        cwd,
                        timestamp,
                        duration_ms: start.elapsed().as_millis() as u64,
                    });
                    return 1;
                }
            }
        }
        
        // run regular commands
        let output = if cfg!(target_os = "windows") {
            Command::new("cmd")
                .args(&["/C", cmd])
                .current_dir(&cwd)
                .output()
        } else {
            Command::new("sh")
                .args(&["-c", cmd])
                .current_dir(&cwd)
                .output()
        };
        
        let duration_ms = start.elapsed().as_millis() as u64;
        
        match output {
            Ok(output) => {
                let stdout = String::from_utf8_lossy(&output.stdout).to_string();
                let stderr = String::from_utf8_lossy(&output.stderr).to_string();
                let exit_code = output.status.code().unwrap_or(-1);
                
                // print to terminal
                print!("{}", stdout);
                eprint!("{}", stderr);
                io::stdout().flush().unwrap();
                io::stderr().flush().unwrap();
                
                self.entries.push(CommandEntry {
                    cmd: cmd.to_string(),
                    exit_code,
                    output: stdout,
                    stderr,
                    cwd,
                    timestamp,
                    duration_ms,
                });
                
                exit_code
            }
            Err(e) => {
                eprintln!("Error: {}", e);
                self.entries.push(CommandEntry {
                    cmd: cmd.to_string(),
                    exit_code: -1,
                    output: String::new(),
                    stderr: format!("Error: {}", e),
                    cwd,
                    timestamp,
                    duration_ms,
                });
                -1
            }
        }
    }
    
    fn save(&self) -> io::Result<()> {
        let log_file = self.log_dir.join("commands.json");
        let log = CommandLog {
            entries: self.entries.clone(),
        };
        
        let json = serde_json::to_string_pretty(&log)?;
        fs::write(&log_file, json)?;
        
        println!("\nSession saved to: {}", log_file.display());
        Ok(())
    }
    
    fn interactive_shell(&mut self) -> io::Result<()> {
        println!("Recording session to: {}", self.log_dir.display());
        println!("Type 'exit' to quit\n");
        
        loop {
            // show prompt
            let cwd = env::current_dir()
                .map(|p| p.to_string_lossy().to_string())
                .unwrap_or_else(|_| String::from("/"));
            
            print!("{} $ ", cwd);
            io::stdout().flush()?;
            
            // read command
            let mut cmd = String::new();
            io::stdin().read_line(&mut cmd)?;
            let cmd = cmd.trim();
            
            if cmd.is_empty() {
                continue;
            }
            
            if cmd == "exit" || cmd == "quit" {
                break;
            }
            
            self.run_command(cmd);
        }
        
        self.save()?;
        Ok(())
    }
}

fn main() -> io::Result<()> {
    let args: Vec<String> = env::args().collect();
    let mut logger = CommandLogger::new()?;
    
    if args.len() > 1 {
        // run single command mode
        let cmd = args[1..].join(" ");
        let exit_code = logger.run_command(&cmd);
        logger.save()?;
        std::process::exit(exit_code);
    } else {
        // interactive mode
        logger.interactive_shell()?;
    }
    
    Ok(())
}
