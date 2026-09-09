//! Transport-level error types.
//!
//! These errors cover I/O failures, framing issues, serialization problems,
//! and connection state violations that can occur in any transport implementation.

use thiserror::Error;

/// Errors that can occur during transport operations.
#[derive(Debug, Error)]
pub enum TransportError {
    /// An I/O error occurred during read or write.
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    /// JSON serialization or deserialization failed.
    #[error("serialization error: {0}")]
    Serialization(#[from] serde_json::Error),

    /// The remote end closed the connection.
    #[error("connection closed")]
    ConnectionClosed,

    /// Attempted to send or receive on a transport that is not connected.
    #[error("not connected")]
    NotConnected,

    /// A framing violation was detected (e.g., missing Content-Length header).
    #[error("invalid frame: {0}")]
    InvalidFrame(String),

    /// A connection attempt timed out.
    #[error("connection timed out")]
    ConnectionTimeout,

    /// A WebSocket protocol error occurred.
    #[error("WebSocket error: {0}")]
    WebSocket(String),
}

#[cfg(test)]
#[path = "error.tests.rs"]
mod tests;
