//! What the transport can fail with.

use thiserror::Error;

/// Transport-level failure: the agent could not be found, started, spoken to,
/// or did not answer in time.
#[derive(Error, Debug)]
pub enum AcpError {
    /// No ACP agent binary on PATH or in the usual install locations.
    #[error("ACP binary not found: {0}")]
    CliNotFound(String),

    /// Spawning the agent or completing its handshake failed.
    #[error("Connection error: {0}")]
    Connection(String),

    /// The connection was up and then broke: the agent exited, closed its
    /// stdout, or refused a request.
    #[error("Transport error: {0}")]
    Transport(String),

    /// The agent did not answer within the allotted time.
    #[error("Timeout: {0}")]
    Timeout(String),

    /// I/O on the agent's pipes or on a file the agent asked for.
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    /// A payload that was not the JSON it claimed to be.
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),
}

impl AcpError {
    /// A [`AcpError::Connection`] from any message.
    pub fn connection(msg: impl Into<String>) -> Self {
        Self::Connection(msg.into())
    }

    /// A [`AcpError::Transport`] from any message.
    pub fn transport(msg: impl Into<String>) -> Self {
        Self::Transport(msg.into())
    }

    /// A [`AcpError::Timeout`] from any message.
    pub fn timeout(msg: impl Into<String>) -> Self {
        Self::Timeout(msg.into())
    }
}
