use std::fmt;

#[derive(Debug)]
pub enum RecliError {
    /// IO-related errors
    Io(std::io::Error),
    /// PTY-related errors
    Pty(Box<dyn std::error::Error + Send + Sync>),
    /// terminal-related errors
    Terminal(String),
    /// shell process errors
    Shell(String),
}

impl fmt::Display for RecliError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            RecliError::Io(e) => write!(f, "IO error: {}", e),
            RecliError::Pty(e) => write!(f, "PTY error: {}", e),
            RecliError::Terminal(msg) => write!(f, "Terminal error: {}", msg),
            RecliError::Shell(msg) => write!(f, "Shell error: {}", msg),
        }
    }
}

impl std::error::Error for RecliError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            RecliError::Io(e) => Some(e),
            RecliError::Pty(e) => Some(e.as_ref()),
            RecliError::Terminal(_) => None,
            RecliError::Shell(_) => None,
        }
    }
}

impl From<std::io::Error> for RecliError {
    fn from(error: std::io::Error) -> Self {
        RecliError::Io(error)
    }
}

/// result type alias for Recli operations
pub type Result<T> = std::result::Result<T, RecliError>;
