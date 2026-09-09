//! Completed tool call data for inspection and history.
//!
//! A `CompletedTool` captures the input/output of a tool call after it
//! finishes, retained for later inspection (tool call inspector, session
//! export, remote attach replay).

/// A completed tool call with stored input/output for later inspection.
///
/// Capped at [`MAX_COMPLETED_TOOLS`] to bound memory.
#[derive(Debug, Clone)]
pub struct CompletedTool {
    /// Tool use ID for correlation.
    pub tool_use_id: String,
    /// Tool name (e.g. "Bash", "Edit").
    pub name: String,
    /// Short detail (command or file path).
    pub detail: String,
    /// Tool input JSON.
    pub input: serde_json::Value,
    /// Tool output text (if any).
    pub output: Option<String>,
    /// Whether the tool call failed.
    pub is_error: bool,
    /// Icon for the tool kind (e.g. "⚡", "✎").
    pub kind_icon: Option<String>,
}

/// Maximum number of completed tool calls to retain for inspection.
pub const MAX_COMPLETED_TOOLS: usize = 100;

#[cfg(test)]
#[path = "completed_tool.tests.rs"]
mod tests;
