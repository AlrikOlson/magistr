//! Active tool tracking — currently executing tool call.

use std::time::Instant;

use super::ToolKind;

/// A currently executing tool call (set on ToolUse, cleared on ToolResult).
#[derive(Debug, Clone)]
pub struct ActiveTool {
    /// Tool name (e.g., "Bash", "Edit").
    pub name: String,
    /// Short detail string (e.g., command or file path).
    pub detail: String,
    /// When this tool call started.
    pub started: Instant,
    /// Semantic kind of this tool call (drives status indicator icon).
    pub kind: Option<ToolKind>,
}

#[cfg(test)]
#[path = "active_tool.tests.rs"]
mod tests;
