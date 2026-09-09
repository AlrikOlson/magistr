//! Error types for engine client failures.
//!
//! `EngineClientError` covers the failure modes specific to engine client
//! communication: connection loss, timeouts, and protocol errors.
//! Separate from `MagistrError` to keep the domain boundary clean.

use std::fmt;

/// Error type for engine client failures.
///
/// Covers connection-level failures that can occur when communicating
/// with an agent engine, whether local (in-process) or remote (server).
#[derive(Debug)]
pub enum EngineClientError {
    /// The connection to the engine was lost unexpectedly.
    ConnectionLost(String),

    /// A request to the engine timed out.
    Timeout {
        /// What operation timed out.
        operation: String,
        /// How long we waited before giving up.
        duration: std::time::Duration,
    },

    /// A protocol-level error from the engine (malformed messages, version mismatch).
    Protocol(String),

    /// The engine session is not in a valid state for the requested operation.
    InvalidState {
        /// What was attempted.
        operation: String,
        /// What state the session is in.
        state: String,
    },

    /// Wraps an underlying `MagistrError` from the agent layer.
    Agent(crate::error::MagistrError),
}

impl fmt::Display for EngineClientError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ConnectionLost(msg) => write!(f, "connection lost: {msg}"),
            Self::Timeout {
                operation,
                duration,
            } => write!(f, "{operation} timed out after {duration:.1?}"),
            Self::Protocol(msg) => write!(f, "protocol error: {msg}"),
            Self::InvalidState { operation, state } => {
                write!(f, "invalid state for {operation}: session is {state}")
            }
            Self::Agent(err) => write!(f, "agent error: {err}"),
        }
    }
}

impl std::error::Error for EngineClientError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Agent(err) => Some(err),
            _ => None,
        }
    }
}

impl From<crate::error::MagistrError> for EngineClientError {
    fn from(err: crate::error::MagistrError) -> Self {
        Self::Agent(err)
    }
}

#[cfg(test)]
#[path = "error.tests.rs"]
mod tests;
