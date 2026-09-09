//! Discrete state updates for the event consumer pattern.
//!
//! Instead of directly mutating frontend state, message handlers produce
//! `Vec<StateUpdate>` values. These are applied via the frontend's
//! `apply_update()` method, centralizing all state transitions in one place.
//! This decouples message parsing from state mutation, making both
//! independently testable.
//!
//! `UpdateBatch` wraps a vec of updates with ordering guarantees: phase
//! transitions are applied last, and consecutive stream flushes are coalesced.

use std::collections::{HashMap, HashSet};
use std::time::Instant;

use super::agent_types::{AcpConfigOption, AcpPlanEntry, AgentCommand, PendingTool, ToolLocation};
use super::completed_tool::CompletedTool;
use super::file_status::FileStatus;
use super::gate::GateResult;
use super::log::LogLevel;
use super::phase::Phase;
use super::select::SelectList;
use super::task::ActiveTask;
use super::tool_kind::ToolKind;
use super::workflow_phase::WorkflowPhase;

/// Classification of a tool result for display styling.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ToolResultLevel {
    /// Green checkmark — success with summary.
    Ok,
    /// Red cross — failure with message.
    Err,
    /// Dim dot — neutral info.
    Info,
    /// Indented inline lines — short output (e.g. Bash).
    Output,
    /// Nothing to show.
    Silent,
}

/// A discrete, self-contained update to application state.
///
/// Each variant carries all data needed for one logical state change.
/// Produced by message handlers, applied via the frontend's `apply_update()`.
#[derive(Debug, Clone)]
pub enum StateUpdate {
    // ── Stream content ──────────────────────────────────────────────────
    /// Append streaming markdown text (token-by-token from ACP).
    StreamMarkdown(String),

    /// Start a new output block (assistant turn boundary).
    StreamBeginBlock,

    /// Start a new output block with system role (tool calls, etc).
    StreamBeginSystemBlock,

    /// Push a tool call line to the stream panel.
    StreamToolCall {
        id: String,
        icon: String,
        name: String,
        detail: String,
    },

    /// Push tool input parameter lines as collapsed section body.
    StreamToolInputLines {
        tool_use_id: String,
        kind: Option<ToolKind>,
        input: serde_json::Value,
    },

    /// Push a thinking section to the stream panel.
    StreamThinking { text: String, elapsed_secs: u64 },

    /// Push a log message inline with the stream.
    StreamLog { level: LogLevel, message: String },

    /// Flush the streaming markdown buffer (end of assistant block).
    StreamFlush,

    /// Push a task tag to the stream (subagent spawn indicator).
    StreamTaskTag(String),

    /// Push an image placeholder to the stream.
    StreamImage {
        mime_type: String,
        uri: Option<String>,
        alt: String,
    },

    // ── Phase transitions ───────────────────────────────────────────────
    /// Transition the application phase (Running, AwaitingInput, Done, etc).
    SetPhase(Phase),

    /// Transition the workflow phase (Research, Plan, Execute, Verify).
    SetWorkflowPhase(WorkflowPhase),

    /// Force the workflow tracker to a specific phase (bypasses evidence).
    ForceTrackerPhase(WorkflowPhase),

    /// A think-and-ship broadcast frame (ship / think / roadmap / signal
    /// mutation) relayed from the project's broadcast socket.
    NativeFrame(magistr_engine::NativeFrame),

    /// The native feed's attachment to the broadcast socket changed
    /// (`native.feed`): waiting for it, connected (possibly a reconnect
    /// after frames were lost), or disconnected.
    NativeFeed(magistr_engine::FeedStatus),

    // ── Session state ───────────────────────────────────────────────────
    /// Set or clear the currently active tool indicator.
    SetActiveTool(Option<super::active_tool::ActiveTool>),

    /// Clear the thinking buffer and preview state.
    ClearThinking,

    /// Accumulate thinking text into the buffer.
    AccumulateThinking(String),

    /// Increment the tool call counter in session stats.
    TickToolCall,

    /// Register a pending tool call (awaiting result).
    RegisterPendingTool { id: String, tool: PendingTool },

    /// Record a session result (turn completion).
    SessionResult {
        is_error: bool,
        num_turns: u32,
        duration_ms: u64,
        total_cost_usd: Option<f64>,
        usage: Option<serde_json::Value>,
        result: Option<String>,
    },

    // ── Sidebar / files ─────────────────────────────────────────────────
    /// Add or update a file in the sidebar tree.
    AddFile { path: String, status: FileStatus },

    /// Set the currently active file (highlighted in sidebar).
    SetActiveFile(Option<String>),

    // ── Agent interaction ────────────────────────────────────────────────
    /// Set or clear agent option buttons (from AskUserQuestion or numbered list).
    SetAgentOptions(Option<SelectList>),

    /// Track a subagent task in the sidebar.
    TrackTask(ActiveTask),

    /// Mark a tracked task as done or failed by its tool_use_id.
    MarkTaskDone { tool_use_id: String, is_error: bool },

    // ── Plan / config / usage ───────────────────────────────────────────
    /// Full-replacement update of the agent plan entries.
    UpdatePlan(Vec<AcpPlanEntry>),

    /// Full-replacement update of config options.
    UpdateConfig(Vec<AcpConfigOption>),

    /// Full-replacement update of available commands.
    UpdateCommands(Vec<AgentCommand>),

    /// Update usage/cost tracking.
    UpdateUsage {
        context_tokens_used: u64,
        context_window_size: u64,
        cost_usd: Option<f64>,
    },

    // ── Misc ────────────────────────────────────────────────────────────
    /// Set or clear the in-assistant-block streaming flag.
    SetInAssistantBlock(bool),

    /// Process a session cancellation.
    Cancelled { reason: String },

    /// Feed a tool event to the workflow tracker for phase detection.
    FeedWorkflowTracker {
        kind: Option<ToolKind>,
        name: String,
        input: serde_json::Value,
        locations: Vec<ToolLocation>,
    },

    // ── Tool result display (G1.5) ──────────────────────────────────────
    /// Push a classified tool result to the stream.
    ///
    /// When `tool_use_id` is `Some`, the result is inserted beneath the
    /// corresponding tool call section. When `None`, appended at the end.
    StreamToolResult {
        tool_use_id: Option<String>,
        level: ToolResultLevel,
        text: String,
        /// For `Output` level — individual lines to display.
        lines: Option<Vec<String>>,
    },

    /// Insert a structured diff under a tool call section.
    StreamToolDiff {
        tool_use_id: String,
        diff_text: String,
    },

    /// Insert a hint line under a tool call section (e.g. "→ d to inspect").
    StreamToolHint { tool_use_id: String, hint: String },

    /// Update the display name on a tool call section header.
    StreamToolTitleUpdate { tool_use_id: String, name: String },

    /// Update the detail text on a tool call section header.
    StreamToolDetailUpdate { tool_use_id: String, detail: String },

    /// Record tool call duration on the section header.
    StreamToolDuration {
        tool_use_id: String,
        duration_ms: u64,
    },

    /// Mark that tool input lines have been pushed for a tool call.
    StreamMarkToolInputPushed(String),

    // ── Domain state (G1.5) ─────────────────────────────────────────────
    /// Store a completed tool call for later inspection.
    StoreCompletedTool(CompletedTool),

    /// Remove a pending tool by ID (tool result received).
    RemovePendingTool(String),

    /// Update the name of a pending tool (late-arriving title).
    UpdatePendingToolName { tool_use_id: String, name: String },

    /// Replace quality gate results for the current iteration.
    SetGateResults(Vec<GateResult>),

    /// Mark the roadmap as stale (needs reload after mutation tool).
    SetRoadmapStale,

    /// Show a toast notification in the UI.
    Toast { level: LogLevel, message: String },

    /// Show error detail panel with context.
    ShowErrorDetail {
        text: String,
        tool_name: Option<String>,
    },

    /// Mark the oldest running task as done or failed (system event fallback).
    MarkOldestRunningTask {
        failed: bool,
        error_msg: Option<String>,
    },
}

/// A batch of state updates with ordering guarantees.
///
/// When applied, the batch ensures:
/// 1. Phase transitions (`SetPhase`, `SetWorkflowPhase`) are applied last
/// 2. Consecutive `StreamFlush` updates are coalesced into one
/// 3. `SetInAssistantBlock(false)` before `StreamBeginBlock` is preserved
#[derive(Debug, Clone, Default)]
pub struct UpdateBatch {
    updates: Vec<StateUpdate>,
}

impl UpdateBatch {
    /// Create a new empty batch.
    #[must_use]
    pub fn new() -> Self {
        Self {
            updates: Vec::new(),
        }
    }

    /// Create a batch from existing updates.
    #[must_use]
    pub fn from_updates(updates: Vec<StateUpdate>) -> Self {
        Self { updates }
    }

    /// Add an update to the batch.
    pub fn push(&mut self, update: StateUpdate) {
        self.updates.push(update);
    }

    /// Number of updates in the batch.
    #[must_use]
    pub fn len(&self) -> usize {
        self.updates.len()
    }

    /// Whether the batch is empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.updates.is_empty()
    }

    /// Consume the batch, returning updates in application order.
    ///
    /// Ordering guarantees:
    /// - Non-phase updates come first, in original order
    /// - Phase transitions (`SetPhase`, `SetWorkflowPhase`) are moved to the end
    /// - Consecutive `StreamFlush` updates are coalesced into one
    #[must_use]
    pub fn into_ordered(self) -> Vec<StateUpdate> {
        let mut normal = Vec::new();
        let mut phase_updates = Vec::new();
        let mut last_was_flush = false;

        for update in self.updates {
            match &update {
                StateUpdate::SetPhase(_) | StateUpdate::SetWorkflowPhase(_) => {
                    last_was_flush = false;
                    phase_updates.push(update);
                }
                StateUpdate::StreamFlush => {
                    if !last_was_flush {
                        normal.push(update);
                        last_was_flush = true;
                    }
                    // else: coalesce consecutive flushes
                }
                _ => {
                    last_was_flush = false;
                    normal.push(update);
                }
            }
        }

        normal.extend(phase_updates);
        normal
    }
}

/// Read-only context extracted from app state before producing updates.
///
/// Allows `into_updates` functions to be pure — they read from this context
/// instead of borrowing the app, so they never need `&mut App`.
#[derive(Debug, Clone)]
pub struct HandlerContext {
    /// The project root directory (for path normalization).
    pub project_dir: String,
    /// Whether we're currently in an assistant text block.
    pub in_assistant_block: bool,
    /// The accumulated thinking buffer text.
    pub thinking_buffer: String,
    /// When thinking started (for elapsed calculation).
    pub thinking_since: Option<Instant>,
    /// Whether a workflow tracker is active.
    pub has_tracker: bool,
    /// Snapshot of pending tool calls (for result classification).
    pub pending_tools: HashMap<String, PendingTool>,
    /// Whether the detail panel is currently visible.
    pub detail_visible: bool,
    /// Which tool IDs have already had input lines pushed.
    pub tool_input_pushed: HashSet<String>,
    /// Map from tool_use_id to stream panel section ID.
    pub tool_section_ids: HashMap<String, usize>,
}

#[cfg(test)]
#[path = "state_update.tests.rs"]
mod tests;
