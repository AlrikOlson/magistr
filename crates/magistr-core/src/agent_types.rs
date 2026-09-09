//! Agent message types shared between frontends.
//!
//! These types are extracted from the main crate's `agent::message` module
//! so that `StateUpdate` and other core types can reference them without
//! depending on TUI or backend-specific code.

use super::ToolKind;

// ─── Tool location ──────────────────────────────────────────────────────────

/// A file location affected by a tool call (mirrors ACP `ToolCallLocation`).
#[derive(Debug, Clone)]
pub struct ToolLocation {
    /// Relative or absolute file path.
    pub path: String,
    /// Optional line number (1-based).
    pub line: Option<u32>,
}

// ─── ACP Plan types ─────────────────────────────────────────────────────────

/// Priority level for a plan entry (mirrors ACP `PlanEntryPriority`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlanEntryPriority {
    High,
    Medium,
    Low,
}

impl PlanEntryPriority {
    /// Single-character indicator for display.
    pub fn indicator(self) -> &'static str {
        match self {
            Self::High => "!",
            Self::Medium => "-",
            Self::Low => "·",
        }
    }
}

/// Execution status of a plan entry (mirrors ACP `PlanEntryStatus`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlanEntryStatus {
    Pending,
    InProgress,
    Completed,
}

impl PlanEntryStatus {
    /// Icon for display.
    pub fn icon(self) -> &'static str {
        match self {
            Self::Pending => "○",
            Self::InProgress => "◐",
            Self::Completed => "✓",
        }
    }
}

/// A single entry in an ACP agent plan.
#[derive(Debug, Clone)]
pub struct AcpPlanEntry {
    /// Description of this plan step.
    pub content: String,
    /// Priority level.
    pub priority: PlanEntryPriority,
    /// Current execution status.
    pub status: PlanEntryStatus,
}

// ─── ACP Config Option types ────────────────────────────────────────────────

/// Category of a config option (mirrors ACP `SessionConfigOptionCategory`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConfigOptionCategory {
    /// Session mode selector (e.g., plan, code, architect).
    Mode,
    /// Model selector (e.g., claude-sonnet-4, claude-opus-4).
    Model,
    /// Thought/reasoning level selector.
    ThoughtLevel,
    /// Unknown or extension category.
    Other(String),
}

/// A selectable value within a config option.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConfigSelectValue {
    /// Unique ID for this value (sent back in set_config_option).
    pub value_id: String,
    /// Human-readable display name.
    pub name: String,
}

/// A config option advertised by the ACP agent.
///
/// Simplified from `agent-client-protocol::SessionConfigOption` for rendering.
/// Full-replacement semantics: each update replaces the entire config options vec.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AcpConfigOption {
    /// Unique identifier (e.g., "model", "mode", "thinking").
    pub id: String,
    /// Human-readable label (e.g., "Model", "Mode").
    pub name: String,
    /// Optional description.
    pub description: Option<String>,
    /// Semantic category for grouping/display.
    pub category: Option<ConfigOptionCategory>,
    /// Currently selected value ID.
    pub current_value: String,
    /// Available values to choose from.
    pub options: Vec<ConfigSelectValue>,
}

impl AcpConfigOption {
    /// Get the display name of the currently selected value.
    ///
    /// Falls back to the raw `current_value` ID if no matching option is found.
    #[must_use]
    pub fn current_value_name(&self) -> &str {
        self.options
            .iter()
            .find(|o| o.value_id == self.current_value)
            .map(|o| o.name.as_str())
            .unwrap_or(&self.current_value)
    }
}

// ─── ACP Agent Command types ────────────────────────────────────────────────

/// A slash command advertised by the ACP agent.
///
/// Agents dynamically advertise available commands via `available_commands_update`
/// notifications. Commands are sent as regular text in `session/prompt`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AgentCommand {
    /// Command name (e.g., `compact`, `review`, `web`).
    pub name: String,
    /// Human-readable description of what the command does.
    pub description: String,
    /// Optional input hint (e.g., "search query").
    pub hint: Option<String>,
}

// ─── Pending tool ───────────────────────────────────────────────────────────

/// A pending tool call awaiting its result.
///
/// Stored in `pending_tools` to preserve the typed `ToolKind` across the
/// tool_use → tool_result boundary, eliminating heuristic name matching.
#[derive(Debug, Clone)]
pub struct PendingTool {
    /// Display name / title from ACP (may be freeform like "Reading file").
    pub name: String,
    /// Typed tool kind from ACP — the authoritative dispatch key.
    pub kind: Option<ToolKind>,
    /// Tool input JSON (stored for tool call inspector).
    pub input: serde_json::Value,
    /// Short detail string (e.g., command or file path).
    pub detail: String,
    /// When the tool call was initiated (for duration badge).
    pub started_at: std::time::Instant,
}

#[cfg(test)]
#[path = "agent_types.tests.rs"]
mod tests;
