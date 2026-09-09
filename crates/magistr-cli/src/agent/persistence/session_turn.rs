//! A single conversation turn stored in SQLite for session replay.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::fmt;

/// Role of a conversation turn.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TurnRole {
    /// User message.
    User,
    /// Assistant text response.
    Assistant,
    /// Tool invocation by the assistant.
    ToolUse,
    /// Result returned from a tool.
    ToolResult,
}

impl TurnRole {
    /// Parse from a database string.
    pub fn parse_role(s: &str) -> Option<Self> {
        match s {
            "user" => Some(Self::User),
            "assistant" => Some(Self::Assistant),
            "tool_use" => Some(Self::ToolUse),
            "tool_result" => Some(Self::ToolResult),
            _ => None,
        }
    }

    /// Database string representation.
    #[must_use]
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::User => "user",
            Self::Assistant => "assistant",
            Self::ToolUse => "tool_use",
            Self::ToolResult => "tool_result",
        }
    }

    /// Human-readable label for display.
    #[must_use]
    pub fn label(&self) -> &'static str {
        match self {
            Self::User => "User",
            Self::Assistant => "Assistant",
            Self::ToolUse => "Tool Call",
            Self::ToolResult => "Tool Result",
        }
    }
}

impl fmt::Display for TurnRole {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.label())
    }
}

/// A single conversation turn for session replay.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SessionTurn {
    /// Position in the conversation (0-indexed).
    pub turn_index: u32,
    /// Role of this turn.
    pub role: TurnRole,
    /// Text content of the turn.
    pub content: String,
    /// Tool name (only for `ToolUse` and `ToolResult` roles).
    pub tool_name: Option<String>,
    /// Tool invocation ID (links `ToolUse` to its `ToolResult`).
    pub tool_id: Option<String>,
    /// When this turn occurred.
    pub timestamp: DateTime<Utc>,
}

impl SessionTurn {
    /// Whether this turn is a tool-related entry.
    #[must_use]
    pub fn is_tool(&self) -> bool {
        matches!(self.role, TurnRole::ToolUse | TurnRole::ToolResult)
    }

    /// Whether this turn's content exceeds the given line count.
    #[must_use]
    pub fn is_long(&self, max_lines: usize) -> bool {
        self.content.lines().count() > max_lines
    }
}

#[cfg(test)]
#[path = "session_turn.tests.rs"]
mod tests;
