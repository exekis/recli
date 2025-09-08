use super::log_event::LogEventV1;
use chrono::DateTime;

/// validate a LogEventV1 for required fields and formats
pub fn validate_event(event: &LogEventV1) -> Result<(), String> {
    // level validation
    match event.level.as_str() {
        "INFO" | "WARN" | "ERROR" => {}
        other => return Err(format!("invalid level: {}", other)),
    }

    // timestamp must be rfc3339
    if let Err(e) = DateTime::parse_from_rfc3339(&event.timestamp) {
        return Err(format!("invalid rfc3339 timestamp: {}", e));
    }

    if event.id.is_empty() {
        return Err("id is empty".to_string());
    }
    if event.session_id.is_empty() {
        return Err("session_id is empty".to_string());
    }
    if event.host.is_empty() {
        return Err("host is empty".to_string());
    }
    if event.app.is_empty() {
        return Err("app is empty".to_string());
    }
    if event.command.is_empty() && event.message.is_empty() {
        return Err("both command and message are empty".to_string());
    }

    Ok(())
}
