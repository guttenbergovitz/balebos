use std::io;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum BalebosError {
    #[error("Probe error: {0}")]
    Probe(#[from] ProbeError),

    #[error("Storage error: {0}")]
    Storage(#[from] StorageError),

    #[error("Validation error: {0}")]
    Validation(String),

    #[error("IO error: {0}")]
    Io(#[from] io::Error),

    #[error("Missing required field: {0}")]
    MissingField(String),

    #[error("Invalid ID format: {0}")]
    InvalidId(String),
}

#[derive(Debug, Error)]
pub enum ProbeError {
    #[error("{tool} not found in PATH")]
    ToolNotFound { tool: String },

    #[error("{tool} failed: {stderr}")]
    ToolFailed { tool: String, stderr: String },

    #[error("Failed to parse JSON: {0}")]
    ParseError(#[from] serde_json::Error),

    #[error("Tool execution error: {0}")]
    Execution(#[from] io::Error),

    #[error("RDAP lookup failed")]
    RdapFailed,

    #[error("Invalid output from {tool}: {reason}")]
    InvalidOutput { tool: String, reason: String },
}

#[derive(Debug, Error)]
pub enum StorageError {
    #[error("IO error: {0}")]
    Io(#[from] io::Error),

    #[error("Serialization failed: {0}")]
    Serialization(#[from] serde_json::Error),

    #[error("SQLite error: {0}")]
    Sqlite(String),
}

pub type Result<T> = std::result::Result<T, BalebosError>;
