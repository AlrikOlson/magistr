//! Content blocks within agent messages.
//!
//! These are the atomic units of content in an assistant or user message:
//! text, tool calls, tool results, thinking traces, and images.

use crate::agent_types::ToolLocation;
use crate::tool_kind::ToolKind;

/// A content block within an assistant message.
#[derive(Debug, Clone)]
pub enum ContentBlock {
    /// Plain text output from the assistant.
    Text { text: String },

    /// Extended thinking / reasoning trace.
    Thinking { thinking: String, signature: String },

    /// The assistant wants to invoke a tool.
    ToolUse {
        id: String,
        /// Human-readable display title (e.g. "Write .magistr/current-plan.md").
        name: String,
        input: serde_json::Value,
        /// ACP tool kind (read, edit, execute, search, etc.) for icon display.
        kind: Option<ToolKind>,
        /// File locations affected by this tool call (for "follow the agent").
        locations: Vec<ToolLocation>,
        /// Programmatic tool name from `_meta.claudeCode.toolName` (e.g. "Write",
        /// "mcp__jig__magistr_set_phase"). Used for tool classification instead of
        /// the display title. `None` when not provided by the ACP agent.
        tool_name: Option<String>,
    },

    /// Result returned from a tool execution.
    ToolResult {
        tool_use_id: String,
        content: Option<String>,
        is_error: Option<bool>,
    },

    /// Image content from the agent (base64-encoded or URI reference).
    ///
    /// Rendered as an inline image when the terminal supports Kitty/Sixel/iTerm2,
    /// or as a text placeholder with dimensions and alt text otherwise.
    Image {
        /// MIME type (e.g. "image/png", "image/jpeg").
        mime_type: String,
        /// Optional URI for the image source.
        uri: Option<String>,
        /// Alt text extracted from context (filename or description).
        alt: String,
    },
}

/// Content of an assistant message.
#[derive(Debug, Clone)]
pub struct AssistantContent {
    /// Model that produced this message (e.g. "claude-sonnet-4-20250514").
    pub model: String,
    /// Ordered content blocks.
    pub content: Vec<ContentBlock>,
}

/// Content of a user message.
#[derive(Debug, Clone)]
pub struct UserContent {
    pub role: String,
    pub content: Option<UserBody>,
}

/// User content payload — either a simple string or structured blocks.
#[derive(Debug, Clone)]
pub enum UserBody {
    String(String),
    Blocks(Vec<ContentBlock>),
}

#[cfg(test)]
#[path = "content_block.tests.rs"]
mod tests;
