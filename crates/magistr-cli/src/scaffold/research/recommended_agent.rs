use serde::Deserialize;

/// An AI agent recommended for this stack.
#[derive(Debug, Clone, Deserialize)]
pub struct RecommendedAgent {
    /// Agent identifier (e.g. `"cursor"`, `"copilot"`).
    pub key: String,
    /// Why this agent is recommended for the stack.
    pub reason: String,
}
