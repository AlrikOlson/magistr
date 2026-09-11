use serde::{Deserialize, Serialize};

/// Failures of the native worker boundary, independent of MCP envelopes.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, thiserror::Error)]
pub enum WorkerError {
    #[error("worker queue is full")]
    Busy,
    #[error("worker request cancelled")]
    Cancelled,
    #[error("worker request deadline exceeded")]
    Deadline,
    #[error("worker input quarantined: {0}")]
    Quarantined(String),
    #[error("worker supervisor stopped")]
    Stopped,
    #[error("worker unavailable: {0}")]
    Unavailable(String),
    #[error("invalid worker protocol: {0}")]
    Protocol(String),
    #[error("unsupported worker protocol version")]
    UnsupportedVersion,
    #[error("{code}: {message}")]
    Remote { code: String, message: String },
}
