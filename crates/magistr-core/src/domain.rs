//! Domain state — the frontend-agnostic core of application state.
//!
//! `DomainState` holds all agent/engine-related state that any frontend
//! (desktop, CLI, remote attach) needs: session identity, workflow tracking,
//! budget, agent plan/tools/tasks, file tree, and messages.
//!
//! Presentation-only state (layout, effects, overlays, input, scroll positions,
//! `Instant` timers, sidebar widgets) belongs to the frontend implementation.

use std::collections::HashMap;

use super::agent_types::{AcpPlanEntry, AgentCommand, PendingTool};
use super::budget_state::BudgetState;
use super::file_entry::FileEntry;
use super::log::LogMessage;
use super::select::SelectList;
use super::session_state::SessionState;
use super::task::ActiveTask;
use super::workflow_state::WorkflowState;

/// Domain state: everything the engine owns, frontend-agnostic.
///
/// Frontend implementations compose this struct and add presentation-specific
/// fields. For example, the interface adds `FileTree` (ratatui rendering),
/// `GitStatusProvider` (process management), and `Instant`-based timers.
#[derive(Debug, Clone, Default)]
pub struct DomainState {
    /// Session sub-state: active tool, thinking, identity, model, context usage.
    pub session: SessionState,
    /// Workflow sub-state: phase tracking, gate results, loop progress.
    pub workflow: WorkflowState,
    /// Budget sub-state: spending limits and thresholds.
    pub budget: BudgetState,
    /// File entries tracked during the session (generated/modified files).
    pub files: Vec<FileEntry>,
    /// ACP agent plan entries (full-replacement on each update).
    pub agent_plan: Vec<AcpPlanEntry>,
    /// ACP agent slash commands (full-replacement on each update).
    pub agent_commands: Vec<AgentCommand>,
    /// Map of tool_use_id → pending tool info for correlating ToolResult with its ToolUse.
    pub pending_tools: HashMap<String, PendingTool>,
    /// Active subagent tasks (description → status).
    pub active_tasks: Vec<ActiveTask>,
    /// Currently active file the agent is working on.
    pub active_file: Option<String>,
    /// Agent-presented options for selection.
    pub agent_options: Option<SelectList>,
    /// Log messages displayed in the main area.
    pub messages: Vec<LogMessage>,
    /// Roadmap sync status for the footer indicator.
    pub sync_status: String,
    /// Flag set when a roadmap-mutating MCP tool completes.
    pub roadmap_stale: bool,
}

#[cfg(test)]
#[path = "domain.tests.rs"]
mod tests;
