//! What a provider is told when a session opens.

use std::path::PathBuf;

use crate::{AgentError, Provider};
use agent_client_protocol::schema::v1::{McpServer, SessionId};

/// A session the client is opening (`session/new`) or resuming
/// (`session/load`), as the provider sees it.
#[derive(Debug, Clone)]
pub struct SessionStart {
    /// The session's id — minted by the agent for a new session, named by the
    /// client for a resumed one.
    pub session_id: SessionId,
    /// The working directory the client opened the session in.
    pub cwd: PathBuf,
    /// The MCP servers the client wants the model to have.
    pub mcp_servers: Vec<McpServer>,
    /// `true` for `session/load`: the provider should restore the session
    /// and return its history for the agent to replay to the client.
    pub resume: bool,
    /// Explicit session model. None preserves the runtime model on load.
    pub model: Option<String>,
    /// Explicit reasoning effort, validated by the chosen provider.
    pub effort: Option<String>,
}

impl SessionStart {
    pub(crate) fn validate(&self, provider: &dyn Provider) -> Result<(), AgentError> {
        let capabilities = provider.capabilities();
        if !self.mcp_servers.is_empty() && !capabilities.tools {
            return Err(AgentError::provider(format!(
                "{} does not support MCP tools",
                provider.name()
            )));
        }
        if self.resume && !(capabilities.durable_load && capabilities.history_replay) {
            return Err(AgentError::provider(format!(
                "{} does not support durable session load; start a new session",
                provider.name()
            )));
        }
        if let Some(model) = &self.model
            && !provider.models().iter().any(|choice| &choice.id == model)
        {
            return Err(AgentError::UnknownModel(model.clone()));
        }
        if self.effort.is_some() && !capabilities.effort_selection {
            return Err(AgentError::provider(format!(
                "{} does not support explicit reasoning effort",
                provider.name()
            )));
        }
        Ok(())
    }
}
