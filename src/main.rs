use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::env;
use std::error::Error as StdError;
use std::fs;
use std::io::{self, Write};
use std::path::PathBuf;
use std::process::Command;
use std::time::Instant;
use azure_data_cosmos::prelude::*;
use azure_data_cosmos::CosmosEntity;
use azure_core::error::{Error as AzureError, ErrorKind as AzureErrorKind};

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

// session document stored as a single blob per session in cosmos db
#[derive(Debug, Clone, Serialize, Deserialize)]
struct SessionDoc {
    id: String,          // e.g., same as session_id or a new uuid
    session_id: String,  // pk: must match container pk (/session_id)
    host: String,
    user: String,
    started_at: String,  // iso8601
    ended_at: String,    // iso8601
    entries: Vec<CommandEntry>,
}

struct CommandLogger {
    session_id: String,
    log_dir: PathBuf,
    entries: Vec<CommandEntry>,
    cosmos_client: Option<CosmosClient>,
    cosmos_database: Option<String>,
    cosmos_container: Option<String>,
}

impl CosmosEntity for SessionDoc {
    type Entity = String;
    fn partition_key(&self) -> Self::Entity { self.session_id.clone() }
}

impl CommandLogger {
    async fn new() -> io::Result<Self> {
        // load .env file if it exists
        dotenv::dotenv().ok();
        
        let session_id = chrono::Local::now().format("%Y%m%d_%H%M%S").to_string();
        let home = env::var("HOME").unwrap_or_else(|_| "/tmp".to_string());
        let log_dir = PathBuf::from(home)
            .join(".recli")
            .join("logs")
            .join(&session_id);
        
        fs::create_dir_all(&log_dir)?;
        
        // initialize cosmos db client if credentials are available
        let cosmos_client = Self::init_cosmos_client();
        let cosmos_database = env::var("RECLI_AZURE__COSMOS__DB").ok();
        let cosmos_container = env::var("RECLI_AZURE__COSMOS__CONTAINER").ok();
        
        Ok(CommandLogger {
            session_id,
            log_dir,
            entries: Vec::new(),
            cosmos_client,
            cosmos_database,
            cosmos_container,
        })
    }
    
    fn init_cosmos_client() -> Option<CosmosClient> {
        // helper: clean and normalize endpoint
        fn normalize_endpoint(mut ep: String) -> String {
            ep = ep.trim().to_string();
            // remove quotes if present
            ep = ep.trim_matches('"').to_string();
            // remove trailing slash
            if ep.ends_with('/') { 
                ep.pop(); 
            }
            // remove port :443 (it's the default for https)
            if ep.ends_with(":443") { 
                ep.truncate(ep.len() - 4); 
            }
            ep
        }

        // helper: extract account name from full endpoint url
        fn extract_account_name(endpoint: &str) -> Option<String> {
            let url = endpoint.strip_prefix("https://").or_else(|| endpoint.strip_prefix("http://")).unwrap_or(endpoint);
            // expect format: account.documents.azure.com
            let account = url.split('.').next()?;
            if account.is_empty() { None } else { Some(account.to_string()) }
        }

        // try to get cosmos db connection from environment
        if let Ok(conn_str) = env::var("RECLI_AZURE__COSMOS__CONNSTR") {
            // parse connection string
            // format: accountendpoint=https://xxx.documents.azure.com:443/;accountkey=xxx==
            let mut endpoint = String::new();
            let mut key = String::new();
            
            for part in conn_str.split(';') {
                let p = part.trim();
                if let Some(value) = p.strip_prefix("AccountEndpoint=") {
                    endpoint = normalize_endpoint(value.to_string());
                } else if let Some(value) = p.strip_prefix("AccountKey=") {
                    key = value.trim().to_string();
                }
            }
            
            // validate the endpoint and key
            if !endpoint.is_empty() && !key.is_empty() {
                // extract account name from endpoint - azure_data_cosmos expects account name, not full url
                if let Some(account_name) = extract_account_name(&endpoint) {
                    // create the authorization token and client
                    if let Ok(auth) = AuthorizationToken::primary_key(&key) {
                        eprintln!("debug: parsed endpoint: {}", endpoint);
                        eprintln!("debug: extracted account: {}", account_name);
                        eprintln!("debug: creating client with account name");
                        return Some(CosmosClient::new(account_name, auth));
                    }
                }
            }
        }
        
        // alternative: use individual env vars
        if let (Ok(account), Ok(key)) = (
            env::var("RECLI_AZURE__COSMOS__ACCOUNT"),
            env::var("RECLI_AZURE__COSMOS__KEY")
        ) {
            let account_name = account.trim().to_string();
            if let Ok(auth) = AuthorizationToken::primary_key(&key) {
                eprintln!("debug: using cosmos account: {}", account_name);
                return Some(CosmosClient::new(account_name, auth));
            }
        }
        
        None
    }
    
    // print detailed http error info from azure core
    fn log_cosmos_error(context: &str, err: &AzureError) {
        eprintln!("! {}: {}", context, err);
        match err.kind() {
            AzureErrorKind::HttpResponse { status, error_code, .. } => {
                eprintln!("  http.status      = {:?}", status);
                eprintln!("  http.error_code  = {:?}", error_code);
            }
            other => {
                eprintln!("  non-http error kind = {:?}", other);
            }
        }
        // Print sources for more context (timeouts, dns, tls)
        let mut src = err.source();
        let mut i = 0;
        while let Some(s) = src {
            eprintln!("  source[{i}] = {}", s);
            src = s.source();
            i += 1;
        }
    }

    async fn upload_session_to_cosmos(&self) -> azure_core::error::Result<()> {
        // single upsert of the entire session document at the very end
        let (client, db_name, container_name) = match (
            &self.cosmos_client,
            &self.cosmos_database,
            &self.cosmos_container,
        ) {
            (Some(c), Some(d), Some(k)) => (c, d, k),
            _ => return Ok(()), // cosmos not configured → nothing to do
        };

        let host = hostname::get()
            .ok()
            .and_then(|h| h.into_string().ok())
            .unwrap_or_else(|| "unknown".to_string());
        let user = std::env::var("USER").unwrap_or_else(|_| "unknown".to_string());

        // started_at: first entry or now; ended_at: now
        let started_at = self
            .entries
            .first()
            .map(|e| e.timestamp.clone())
            .unwrap_or_else(|| chrono::Utc::now().to_rfc3339());
        let ended_at = chrono::Utc::now().to_rfc3339();

        // 0) warm-up tiny upsert to validate connectivity/auth
        #[derive(Debug, Clone, Serialize, Deserialize)]
        struct PingDoc { id: String, session_id: String, kind: &'static str, ts: String }
        impl CosmosEntity for PingDoc { type Entity = String; fn partition_key(&self) -> Self::Entity { self.session_id.clone() } }

        let ping = PingDoc {
            id: format!("_recli_ping_{}", self.session_id),
            session_id: self.session_id.clone(),
            kind: "recli_ping",
            ts: chrono::Utc::now().to_rfc3339(),
        };

        let db = client.database_client(db_name.clone());
        let col = db.collection_client(container_name.clone());

        if let Err(e) = col
            .create_document(ping)
            .is_upsert(true)
            .into_future()
            .await
        {
            Self::log_cosmos_error("cosmos ping upsert failed", &e);
            return Err(e);
        }

        // 1) real session upsert
        let doc = SessionDoc {
            id: self.session_id.clone(),         // upsert by session_id
            session_id: self.session_id.clone(), // pk=/session_id
            host,
            user,
            started_at,
            ended_at,
            entries: self.entries.clone(),
        };

        if let Err(e) = col
            .create_document(doc)
            .is_upsert(true)
            .into_future()
            .await
        {
            Self::log_cosmos_error("cosmos session upsert failed", &e);
            return Err(e);
        }

        eprintln!("✓ Session uploaded to Cosmos DB");
        Ok(())
    }
    
    async fn run_command(&mut self, cmd: &str) -> i32 {
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
                    
                    let entry = CommandEntry {
                        cmd: cmd.to_string(),
                        exit_code: 0,
                        output: String::new(),
                        stderr: String::new(),
                        cwd: new_cwd,
                        timestamp,
                        duration_ms: start.elapsed().as_millis() as u64,
                    };
                    
                    self.entries.push(entry);
                    return 0;
                }
                Err(e) => {
                    let entry = CommandEntry {
                        cmd: cmd.to_string(),
                        exit_code: 1,
                        output: String::new(),
                        stderr: format!("cd: {}", e),
                        cwd,
                        timestamp,
                        duration_ms: start.elapsed().as_millis() as u64,
                    };
                    
                    eprintln!("cd: {}", e);
                    
                    self.entries.push(entry);
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
                
                let entry = CommandEntry {
                    cmd: cmd.to_string(),
                    exit_code,
                    output: stdout,
                    stderr,
                    cwd,
                    timestamp,
                    duration_ms,
                };
                
                self.entries.push(entry);
                exit_code
            }
            Err(e) => {
                eprintln!("Error: {}", e);
                
                let entry = CommandEntry {
                    cmd: cmd.to_string(),
                    exit_code: -1,
                    output: String::new(),
                    stderr: format!("Error: {}", e),
                    cwd,
                    timestamp,
                    duration_ms,
                };
                
                self.entries.push(entry);
                -1
            }
        }
    }
    
    async fn save_async(&self) -> io::Result<()> {
        let log_file = self.log_dir.join("commands.json");
        let log = CommandLog {
            entries: self.entries.clone(),
        };
        
        let json = serde_json::to_string_pretty(&log)?;
        fs::write(&log_file, json)?;
        
        println!("\nSession saved to: {}", log_file.display());
        
        // try to upload once; never block the repl earlier
        if let Err(e) = self.upload_session_to_cosmos().await {
            Self::log_cosmos_error("Cosmos upload failed", &e);
        }
        
        Ok(())
    }
    
    async fn interactive_shell(&mut self) -> io::Result<()> {
        println!("Recording session to: {}", self.log_dir.display());
        
        println!("Type 'exit' to quit\n");
        
        loop {
            // Show prompt
            let cwd = env::current_dir()
                .map(|p| p.to_string_lossy().to_string())
                .unwrap_or_else(|_| String::from("/"));
            
            print!("{} $ ", cwd);
            io::stdout().flush()?;
            
            // Read command
            let mut cmd = String::new();
            io::stdin().read_line(&mut cmd)?;
            let cmd = cmd.trim();
            
            if cmd.is_empty() {
                continue;
            }
            
            if cmd == "exit" || cmd == "quit" {
                break;
            }
            
            self.run_command(cmd).await;
        }
        
    self.save_async().await?;
        Ok(())
    }
}

/// Minimal Cosmos connectivity & schema check.
async fn cosmos_doctor() -> io::Result<()> {
    dotenv::dotenv().ok();

    let client = match CommandLogger::init_cosmos_client() {
        Some(c) => c,
        None => {
            eprintln!("! Cosmos client init failed. Check env vars:");
            eprintln!("  RECLI_AZURE__COSMOS__CONNSTR  or  (RECLI_AZURE__COSMOS__ACCOUNT + RECLI_AZURE__COSMOS__KEY)");
            return Ok(());
        }
    };
    let db = match std::env::var("RECLI_AZURE__COSMOS__DB") {
        Ok(v) => v,
        Err(_) => { eprintln!("! Missing RECLI_AZURE__COSMOS__DB"); return Ok(()); }
    };
    let container = match std::env::var("RECLI_AZURE__COSMOS__CONTAINER") {
        Ok(v) => v,
        Err(_) => { eprintln!("! Missing RECLI_AZURE__COSMOS__CONTAINER"); return Ok(()); }
    };

    let dbc = client.database_client(db.clone());
    let cc = dbc.collection_client(container.clone());

    eprintln!("→ Checking database '{}'", db);
    match dbc.get_database().into_future().await {
        Ok(_) => eprintln!("  ✓ database exists"),
        Err(e) => {
            CommandLogger::log_cosmos_error("get_database failed", &e);
            return Ok(());
        }
    }

    eprintln!("→ Checking container '{}'", container);
    match cc.get_collection().into_future().await {
        Ok(_) => {
            // try to extract partition key info if available
            eprintln!("  ✓ container exists");
            eprintln!("  Note: Verify container has partition key '/session_id'");
        }
        Err(e) => {
            CommandLogger::log_cosmos_error("get_container failed", &e);
            return Ok(());
        }
    }

    // Try a tiny ping doc in the right PK
    #[derive(Debug, Clone, Serialize, Deserialize)]
    struct PingDoc { id: String, session_id: String, kind: &'static str, ts: String }
    impl CosmosEntity for PingDoc { type Entity = String; fn partition_key(&self) -> Self::Entity { self.session_id.clone() } }
    let ping = PingDoc {
        id: "_recli_doctor_ping".into(),
        session_id: "doctor_pk".into(),
        kind: "doctor_ping",
        ts: chrono::Utc::now().to_rfc3339(),
    };
    eprintln!("→ Upserting ping doc…");
    match cc.create_document(ping).is_upsert(true).into_future().await {
        Ok(_) => eprintln!("  ✓ ping upsert ok"),
        Err(e) => {
            CommandLogger::log_cosmos_error("ping upsert failed", &e);
        }
    }
    Ok(())
}

#[tokio::main]
async fn main() -> io::Result<()> {
    let args: Vec<String> = env::args().collect();
    
    // handle start/end commands for compatibility
    if args.len() > 1 {
        match args[1].as_str() {
            "start" => {
                // interactive mode
                let mut logger = CommandLogger::new().await?;
                logger.interactive_shell().await?;
            }
            "end" => {
                println!("Session already ended (this version doesn't need 'end')");
            }
            "status" => {
                println!("No active session (this version doesn't track sessions)");
            }
            "cosmos_doctor" => {
                cosmos_doctor().await?;
            }
            _ => {
                // run as single command
                let mut logger = CommandLogger::new().await?;
                let cmd = args[1..].join(" ");
                let exit_code = logger.run_command(&cmd).await;
                logger.save_async().await?;
                std::process::exit(exit_code);
            }
        }
    } else {
        // default to interactive mode
        let mut logger = CommandLogger::new().await?;
        logger.interactive_shell().await?;
    }
    
    Ok(())
}