//! Error model shared by every command.
//!
//! Each error has a stable, machine-readable `code` and a documented process
//! exit code. Both are public interface: changing either is a breaking change.

use serde_json::{Value, json};

/// Process exit codes, documented in README.md and AGENTS.md.
pub mod exit {
    pub const OK: u8 = 0;
    pub const OTHER: u8 = 1;
    pub const BAD_INPUT: u8 = 2;
    pub const NOT_FOUND: u8 = 3;
    pub const UNAVAILABLE: u8 = 4;
    pub const RATE_LIMITED: u8 = 5;
}

#[derive(Debug, thiserror::Error)]
pub enum CliError {
    /// The request was rejected as invalid (bad arguments, or HTTP 400).
    #[error("{0}")]
    BadInput(String),

    #[error("{0}")]
    NotFound(String),

    /// Dogechain.com could not be reached or could not answer right now.
    #[error("{0}")]
    Unavailable(String),

    #[error("rate limited by dogechain.com; try again in {retry_after_secs} seconds")]
    RateLimited { retry_after_secs: u64 },

    #[error("{0}")]
    Other(String),

    /// Stdout was closed (e.g. `dogechain … | head`). Not an error for the
    /// user: the process exits 0 without a message.
    #[error("output closed")]
    OutputClosed,
}

impl From<std::io::Error> for CliError {
    fn from(e: std::io::Error) -> Self {
        if e.kind() == std::io::ErrorKind::BrokenPipe {
            CliError::OutputClosed
        } else {
            CliError::Other(format!("could not write output: {e}"))
        }
    }
}

impl CliError {
    /// Stable identifier for scripts and agents to branch on.
    pub fn code(&self) -> &'static str {
        match self {
            CliError::BadInput(_) => "BAD_INPUT",
            CliError::NotFound(_) => "NOT_FOUND",
            CliError::Unavailable(_) => "UNAVAILABLE",
            CliError::RateLimited { .. } => "RATE_LIMITED",
            CliError::Other(_) | CliError::OutputClosed => "OTHER",
        }
    }

    pub fn exit_code(&self) -> u8 {
        match self {
            CliError::BadInput(_) => exit::BAD_INPUT,
            CliError::NotFound(_) => exit::NOT_FOUND,
            CliError::Unavailable(_) => exit::UNAVAILABLE,
            CliError::RateLimited { .. } => exit::RATE_LIMITED,
            CliError::Other(_) => exit::OTHER,
            CliError::OutputClosed => exit::OK,
        }
    }

    /// Same envelope as the API's failures, plus the stable `code`.
    pub fn to_json(&self) -> Value {
        let mut data = json!({
            "error_message": self.to_string(),
            "code": self.code(),
        });
        if let CliError::RateLimited { retry_after_secs } = self {
            data["retry_after_secs"] = json!(retry_after_secs);
        }
        json!({ "status": "fail", "data": data })
    }
}

pub type Result<T> = std::result::Result<T, CliError>;
