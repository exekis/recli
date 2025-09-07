use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

/// canonical log event v1 used for validation and future ingestion
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogEventV1 {
    pub id: String,
    pub schema_version: u8,
    pub timestamp: String, // rfc3339 utc
    pub host: String,
    pub app: String, // "recli"
    pub session_id: String,
    pub level: String, // "INFO" | "WARN" | "ERROR"
    pub command: String,
    pub exit_code: Option<i32>,
    pub error_type: Option<String>,
    pub message: String,
    pub tags: Vec<String>,
    pub raw: Option<serde_json::Value>,
}

impl LogEventV1 {
    /// build a deterministic id from fields
    pub fn make_id(
        host: &str,
        session_id: &str,
        timestamp: &str,
        command: &str,
        offset: &str,
    ) -> String {
        // offset can be file offset or sequence number to ensure uniqueness
        let input = format!(
            "{}|{}|{}|{}|{}",
            host, session_id, timestamp, command, offset
        );
        let mut hasher = Sha256::new();
        hasher.update(input.as_bytes());
        let hash = hasher.finalize();
        hex::encode(hash)
    }
}
