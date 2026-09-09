//! What the agent side can fail with.

use thiserror::Error;

/// A failure inside the agent: a provider that could not do its part, a
/// session the client named that the registry does not hold, or the wire
/// itself.
#[derive(Error, Debug)]
pub enum AgentError {
    /// The provider could not start a session or run a turn.
    #[error("provider error: {0}")]
    Provider(String),

    /// The client named a session this agent has not opened.
    #[error("unknown session: {0}")]
    UnknownSession(String),

    /// `session/load` (or `session/new`) named a session that is already open.
    #[error("session already open: {0}")]
    SessionExists(String),

    /// The client asked for a model the provider does not offer.
    #[error("unknown model: {0}")]
    UnknownModel(String),

    /// The connection refused a notification or request.
    #[error("protocol error: {0}")]
    Protocol(#[from] agent_client_protocol::Error),
}

impl AgentError {
    /// An [`AgentError::Provider`] from any message.
    pub fn provider(msg: impl Into<String>) -> Self {
        Self::Provider(msg.into())
    }
}

/// The JSON-RPC error the client sees for each failure.
impl From<AgentError> for agent_client_protocol::Error {
    fn from(error: AgentError) -> Self {
        match error {
            AgentError::Protocol(e) => e,
            AgentError::Provider(msg) => Self::internal_error().data(serde_json::json!(msg)),
            other => Self::invalid_params().data(serde_json::json!(other.to_string())),
        }
    }
}

#[cfg(test)]
#[path = "error.tests.rs"]
mod tests;
