use serde::{Deserialize, Serialize};

use crate::workflow::WorkflowPhase;

use super::session_status::SessionStatus;

/// Events emitted by the engine session.
///
/// These are the domain-level events that the interface (or any other consumer)
/// can subscribe to. They are produced by the infrastructure adapter
/// translating protocol-specific messages into domain events.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum SessionEvent {
    /// An assistant message was received.
    MessageReceived {
        /// The message content (text, tool use, etc.).
        content: String,
    },

    /// The workflow phase changed.
    PhaseChanged {
        from: WorkflowPhase,
        to: WorkflowPhase,
    },

    /// Cost was updated after a turn.
    CostUpdated {
        /// Accumulated cost in USD.
        total_cost_usd: f64,
        /// Cost for this turn only.
        turn_cost_usd: f64,
    },

    /// A permission request from the agent.
    PermissionRequested {
        /// The id the server matches an answer against
        /// (`SessionCommand::ApprovePermission { request_id }`). Absent
        /// on the wire from a server that predates it — a client then
        /// shows the request and cannot answer it.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        request_id: Option<String>,
        /// Description of what the agent wants to do.
        description: String,
        /// Available response options.
        options: Vec<String>,
    },

    /// A tool execution started.
    ToolStarted {
        /// Tool name (e.g., "Edit", "Bash", "Read").
        tool_name: String,
        /// Brief description of the tool invocation.
        title: Option<String>,
    },

    /// A tool execution completed.
    ToolCompleted {
        /// Tool name.
        tool_name: String,
        /// Whether the tool succeeded.
        success: bool,
    },

    /// An agent turn completed.
    TurnCompleted {
        /// Turn number (1-based).
        turn: u32,
        /// Duration of the turn in milliseconds.
        duration_ms: u64,
    },

    /// The session ended.
    SessionEnded {
        /// Final status.
        status: SessionStatus,
        /// Optional summary message.
        summary: Option<String>,
    },
}
