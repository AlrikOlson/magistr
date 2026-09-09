//! Error type for tool handlers and the MCP server.

use thiserror::Error;

/// What a tool handler or the server can fail with.
#[derive(Error, Debug)]
pub enum ToolError {
    /// The tool itself failed — a bad parameter, a failed command, a missing
    /// file. Surfaces to the agent as a JSON-RPC internal error.
    #[error("MCP error: {0}")]
    Mcp(String),

    /// A tool's configuration (project config, schema) could not be used.
    #[error("Invalid configuration: {0}")]
    InvalidConfig(String),

    /// I/O while running a tool.
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    /// A tool result or argument that is not the JSON it claims to be.
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),
}

/// Result alias for tool handlers.
pub type Result<T> = std::result::Result<T, ToolError>;

impl ToolError {
    /// Build an [`ToolError::Mcp`] from any message.
    pub fn mcp(msg: impl Into<String>) -> Self {
        Self::Mcp(msg.into())
    }
}
