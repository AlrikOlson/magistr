use serde::Deserialize;

fn default_command() -> String {
    "npx".to_string()
}

/// An MCP server discovered through AI research.
#[derive(Debug, Clone, Deserialize)]
pub struct DiscoveredServer {
    /// Config key (e.g. `"context7"`) — used in `.mcp.json`.
    pub key: String,
    /// Human-readable display name.
    pub name: String,
    /// One-line description of what this server does.
    pub description: String,
    /// Executable command (defaults to `"npx"`).
    #[serde(default = "default_command")]
    pub command: String,
    /// Command arguments.
    #[serde(default)]
    pub args: Vec<String>,
    /// Use-case category (e.g. `"docs"`, `"testing"`).
    #[serde(default)]
    pub category: String,
}
