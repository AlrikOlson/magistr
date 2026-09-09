//! Agent message types — the protocol-agnostic message stream.
//!
//! The ACP backend emits `AgentMessage` values, decoupling the TUI
//! and GUI consumers from the `agent-client-protocol` crate.

use crate::agent_types::{AcpConfigOption, AcpPlanEntry, AgentCommand};
use crate::content_block::{AssistantContent, UserContent};
use crate::session_id::SessionId;

/// A single message from the agent stream.
///
/// The ACP backend emits this type, so downstream code (TUI handler,
/// no-TUI loop, roadmap generator) never imports a backend-specific SDK crate.
#[derive(Debug, Clone)]
pub enum AgentMessage {
    /// The user sent a message (echoed back for context).
    User {
        parent_tool_use_id: Option<String>,
        message: UserContent,
        session_id: Option<SessionId>,
    },

    /// The assistant produced content (text, tool calls, thinking).
    Assistant {
        parent_tool_use_id: Option<String>,
        message: AssistantContent,
        session_id: Option<SessionId>,
    },

    /// A system-level event (init, hooks, etc.).
    System {
        subtype: String,
        data: serde_json::Value,
    },

    /// The turn / session completed with summary metrics.
    Result {
        subtype: String,
        duration_ms: u64,
        duration_api_ms: u64,
        is_error: bool,
        num_turns: u32,
        session_id: SessionId,
        total_cost_usd: Option<f64>,
        usage: Option<serde_json::Value>,
        result: Option<String>,
    },

    /// Streaming partial event (tool call progress, status updates).
    StreamEvent {
        uuid: String,
        session_id: SessionId,
        event: serde_json::Value,
        parent_tool_use_id: Option<String>,
    },

    /// ACP agent plan update (full replacement — each update carries the complete plan).
    Plan {
        /// Ordered plan entries from the agent.
        entries: Vec<AcpPlanEntry>,
    },

    /// ACP config options update (full replacement — each update carries the complete set).
    ///
    /// Emitted when:
    /// - `session/new` or `session/load` returns `configOptions` (C7.6)
    /// - Agent sends `config_option_update` notification (C7.8)
    /// - Client calls `session/set_config_option` and gets response (C7.7)
    ConfigUpdate {
        /// Complete set of config options and their current values.
        options: Vec<AcpConfigOption>,
    },

    /// ACP agent slash commands update (full replacement — each update carries the complete set).
    ///
    /// Emitted when the agent sends `available_commands_update` notification (C7.13).
    /// Commands are dynamically updated — agent can add/remove based on context.
    CommandsUpdate {
        /// Complete set of available agent slash commands.
        commands: Vec<AgentCommand>,
    },

    /// ACP context window and cost update (C7.15).
    ///
    /// Emitted when the agent sends `SessionUpdate::UsageUpdate` notifications.
    /// Contains context window usage and optional cumulative cost.
    UsageUpdate {
        /// Tokens currently in the context window.
        context_tokens_used: u64,
        /// Total context window size in tokens.
        context_window_size: u64,
        /// Cumulative session cost in USD (converted from ACP `Cost`).
        cost_usd: Option<f64>,
    },

    /// The session was cancelled via a graceful cancel request (e.g. ACP `session/cancel`).
    ///
    /// Distinct from `Result { is_error: true }` — cancellation is an intentional
    /// user-initiated stop, not an error. The agent may have partial work completed.
    Cancelled {
        session_id: SessionId,
        /// Human-readable reason for cancellation.
        reason: String,
    },
}

#[cfg(test)]
#[path = "agent_message.tests.rs"]
mod tests;
