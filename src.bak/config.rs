use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;

/// top-level config for recli
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Config {
    pub logging: LoggingConfig,
    pub azure: Option<AzureConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoggingConfig {
    pub level: String, // error|warn|info|debug|trace
}

impl Default for LoggingConfig {
    fn default() -> Self {
        Self { level: "info".to_string() }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct AzureConfig {
    pub cosmos: Option<CosmosConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct CosmosConfig {
    pub account: Option<String>,
    pub database: Option<String>,
    pub container: Option<String>,
    pub connection_string: Option<String>,
}

impl Config {
    /// load config from a toml file, then overlay with env vars (RECLI_*)
    pub fn load(path: Option<&str>) -> Self {
        let mut cfg = if let Some(p) = path {
            Self::from_file(p).unwrap_or_default()
        } else {
            // try default ~/.recli/recli.toml
            let home = std::env::var("HOME").unwrap_or_else(|_| "/tmp".into());
            let default_path = format!("{}/.recli/recli.toml", home);
            Self::from_file(&default_path).unwrap_or_default()
        };

        // overlay env variables
        if let Ok(level) = std::env::var("RECLI_LOG_LEVEL") {
            cfg.logging.level = level;
        }

        let mut cosmos = cfg.azure.and_then(|a| a.cosmos).unwrap_or_default();
        if let Ok(v) = std::env::var("RECLI_AZURE__COSMOS__ACCOUNT") { cosmos.account = Some(v); }
        if let Ok(v) = std::env::var("RECLI_AZURE__COSMOS__DB") { cosmos.database = Some(v); }
        if let Ok(v) = std::env::var("RECLI_AZURE__COSMOS__CONTAINER") { cosmos.container = Some(v); }
        if let Ok(v) = std::env::var("RECLI_AZURE__COSMOS__CONNSTR") { cosmos.connection_string = Some(v); }

        cfg.azure = Some(AzureConfig { cosmos: Some(cosmos) });
        cfg
    }

    fn from_file(path: &str) -> Option<Self> {
        let p = Path::new(path);
        if !p.exists() { return None; }
        let text = fs::read_to_string(p).ok()?;
        toml::from_str(&text).ok()
    }
}
