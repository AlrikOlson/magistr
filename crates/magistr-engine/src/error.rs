//! Engine-layer error types.
//!
//! Domain errors for session lifecycle and workflow orchestration.
//! These are backend-agnostic — no ACP, no IO, no TUI concerns.

use std::fmt;

/// Errors produced by the engine domain layer.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum EngineError {
    /// A command was rejected because the session is in an incompatible state.
    #[error("invalid command {command} in state {state}: {reason}")]
    InvalidCommand {
        command: String,
        state: String,
        reason: String,
    },

    /// A workflow phase transition was rejected.
    #[error("invalid phase transition from {from} to {to}")]
    InvalidPhaseTransition { from: String, to: String },

    /// A required field or precondition was missing.
    #[error("precondition failed: {0}")]
    Precondition(String),
}

impl EngineError {
    /// Create an invalid command error.
    pub fn invalid_command(
        command: impl fmt::Display,
        state: impl fmt::Display,
        reason: impl Into<String>,
    ) -> Self {
        Self::InvalidCommand {
            command: command.to_string(),
            state: state.to_string(),
            reason: reason.into(),
        }
    }

    /// Create an invalid phase transition error.
    pub fn invalid_phase_transition(from: impl fmt::Display, to: impl fmt::Display) -> Self {
        Self::InvalidPhaseTransition {
            from: from.to_string(),
            to: to.to_string(),
        }
    }
}

/// Engine result type alias.
pub(crate) type Result<T> = std::result::Result<T, EngineError>;

#[cfg(test)]
#[path = "error.tests.rs"]
mod tests;
