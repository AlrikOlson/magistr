//! What a provider is told when a session opens.

use std::path::PathBuf;

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
}
