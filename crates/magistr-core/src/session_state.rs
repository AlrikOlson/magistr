//! Session identity and tracking state (domain-only).
//!
//! Lightweight session data shared across frontends: active tool, thinking
//! state, session identity, model info, context window usage. No sidebar
//! widgets, no `Instant` timers, no rendering types.

use super::active_tool::ActiveTool;
use super::agent_types::AcpConfigOption;
use super::completed_tool::{CompletedTool, MAX_COMPLETED_TOOLS};

/// Domain-only session state.
///
/// Frontend implementations extend this with presentation-specific fields
/// (sidebar widgets, `Instant` timers, commit logs, session history).
#[derive(Debug, Clone, Default)]
pub struct SessionState {
    /// Currently executing tool (set on ToolUse, cleared on ToolResult).
    pub active_tool: Option<ActiveTool>,
    /// Whether the agent is currently in a thinking block.
    pub thinking_active: bool,
    /// Preview snippet of the current thinking content (truncated for display).
    pub thinking_preview: Option<String>,
    /// Buffer for accumulating streaming thinking chunks.
    pub thinking_buffer: String,
    /// Short session identifier for display and resume.
    pub session_id: Option<String>,
    /// Whether this session was resumed from a previous one.
    pub resumed: bool,
    /// Line index where resumed content ends and new content begins.
    pub resume_marker_line: Option<usize>,
    /// Model name currently in use.
    pub current_model: Option<String>,
    /// ACP config options (model, mode, thought level) from the agent.
    pub config_options: Vec<AcpConfigOption>,
    /// Context window tokens currently used.
    pub context_tokens_used: u64,
    /// Total context window size in tokens.
    pub context_window_size: u64,
    /// Completed tool calls retained for tool call inspector.
    pub completed_tools: Vec<CompletedTool>,
}

impl SessionState {
    /// Store a completed tool call, capping at [`MAX_COMPLETED_TOOLS`].
    pub fn store_completed_tool(&mut self, tool: CompletedTool) {
        if self.completed_tools.len() >= MAX_COMPLETED_TOOLS {
            self.completed_tools.remove(0);
        }
        self.completed_tools.push(tool);
    }

    /// Clear the thinking buffer and preview state.
    pub fn clear_thinking(&mut self) {
        self.thinking_active = false;
        self.thinking_preview = None;
        self.thinking_buffer.clear();
    }

    /// Accumulate thinking text into the buffer.
    pub fn accumulate_thinking(&mut self, text: &str) {
        self.thinking_active = true;
        self.thinking_buffer.push_str(text);
        // Keep a truncated preview (last 200 chars).
        let preview = if self.thinking_buffer.len() > 200 {
            &self.thinking_buffer[self.thinking_buffer.len() - 200..]
        } else {
            &self.thinking_buffer
        };
        self.thinking_preview = Some(preview.to_string());
    }
}

#[cfg(test)]
#[path = "session_state.tests.rs"]
mod tests;
