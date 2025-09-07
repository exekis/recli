use tracing_subscriber::{fmt, EnvFilter};

/// initialize global tracing subscriber from config level or env
pub fn init(level: &str) {
    let env_filter = std::env::var("RUST_LOG").unwrap_or_else(|_| map_level(level));
    let _ = tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::new(env_filter))
        .with_target(false)
        .try_init();
}

fn map_level(level: &str) -> String {
    match level.to_lowercase().as_str() {
        "error" => "error".into(),
        "warn" => "warn".into(),
        "debug" => "debug".into(),
        "trace" => "trace".into(),
        _ => "info".into(),
    }
}
