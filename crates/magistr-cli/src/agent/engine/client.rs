//! `EngineClient` trait — the abstract interface for agent engine communication.
//!
//! This trait decouples the TUI and command handlers from the concrete agent
//! backend. Two implementations are planned:
//!
//! - `LocalEngineClient` — wraps `AgentSession` in-process (zero overhead)
//! - `RemoteEngineClient` — connects to a magistr-server via JSON-RPC (E4a.2)

use super::error::EngineClientError;
use crate::agent::message::AgentMessage;
use crate::agent::session::AcpSessionState;

/// Abstract interface for communicating with an agent engine.
///
/// Implementations handle the details of message transport (in-process channel
/// vs. network connection) while exposing a uniform async API.
pub trait EngineClient: Send {
    /// Receive the next message from the agent stream.
    ///
    /// Returns `None` when the stream is exhausted (session complete).
    /// Errors indicate transport-level failures, not agent-level errors
    /// (those arrive as `AgentMessage::Result { is_error: true, .. }`).
    fn next_message(
        &mut self,
    ) -> impl std::future::Future<Output = Option<Result<AgentMessage, EngineClientError>>> + Send;

    /// Send a command or follow-up message to the agent.
    ///
    /// For multi-turn conversations, this sends the next user message.
    /// Returns an error if the session is not in a state that accepts input.
    fn send_command(
        &self,
        text: &str,
    ) -> impl std::future::Future<Output = Result<(), EngineClientError>> + Send;

    /// Query the current session state.
    ///
    /// Returns `None` if state tracking is not available (e.g., test channels).
    fn query_state(
        &self,
    ) -> impl std::future::Future<Output = Result<Option<AcpSessionState>, EngineClientError>> + Send;

    /// Request graceful cancellation of the current operation.
    ///
    /// The agent will finish its current tool call and then stop.
    /// A `AgentMessage::Cancelled` will arrive through `next_message()`.
    fn cancel(&mut self);

    /// Abort the session immediately, killing the agent process.
    fn abort(&mut self);

    /// Send a configuration change to the agent.
    ///
    /// Fire-and-forget: the agent processes the change and emits a
    /// `ConfigUpdate` message through the normal stream. Returns an error
    /// if the session doesn't support config changes.
    fn send_config_change(
        &self,
        config_id: String,
        value_id: String,
    ) -> impl std::future::Future<Output = Result<(), EngineClientError>> + Send;
}

#[cfg(test)]
#[path = "client.tests.rs"]
mod tests;
