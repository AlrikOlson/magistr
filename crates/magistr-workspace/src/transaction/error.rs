use crate::{contracts::WorkspaceError, worker::WorkerError};

/// A transaction refusal or effect failure. Intent records never prove application.
#[derive(Debug, thiserror::Error)]
pub enum TransactionError {
    #[error("invalid transaction: {0}")]
    InvalidInput(String),
    #[error("workspace policy: {0}")]
    Policy(#[from] WorkspaceError),
    #[error("stale anchor: {0}")]
    StaleAnchor(String),
    #[error("confirmation mismatch: {0}")]
    Confirmation(String),
    #[error("parser worker: {0}")]
    Worker(#[from] WorkerError),
    #[error("parse rejected for {path}: {message}")]
    ParseRejected { path: String, message: String },
    #[error("filesystem effect: {0}")]
    Io(String),
    #[error("journal intent: {0}")]
    Journal(String),
}
