//! Local engine client — wraps `AgentSession` in-process.
//!
//! Zero-overhead implementation of `EngineClient` that delegates directly
//! to `AgentSession`. This is the default for `magistr run` when no server
//! is needed — the agent subprocess runs as a child process of magistr itself.

use super::client::EngineClient;
use super::error::EngineClientError;
use crate::agent::message::AgentMessage;
use crate::agent::session::{AcpSessionState, AgentSession};

/// In-process engine client wrapping an `AgentSession` directly.
///
/// No serialization, no network — messages flow through in-process channels.
/// This is the current behavior of `magistr run`, extracted behind the
/// `EngineClient` trait for future polymorphism with `RemoteEngineClient`.
pub struct LocalEngineClient {
    session: AgentSession,
    /// Messages merged into the stream from outside the agent — the
    /// think-and-ship native feed (`tui::native_feed`) relays broadcast
    /// frames here so a single `magistr next` session sees them too.
    side: Option<tokio::sync::mpsc::UnboundedReceiver<crate::error::Result<AgentMessage>>>,
}

impl LocalEngineClient {
    /// Create a new local engine client wrapping the given session.
    #[must_use]
    pub fn new(session: AgentSession) -> Self {
        Self {
            session,
            side: None,
        }
    }

    /// Merge messages from `rx` into `next_message` alongside the agent's
    /// own stream. The agent's end-of-stream still ends the stream.
    #[must_use]
    pub fn with_side_channel(
        mut self,
        rx: tokio::sync::mpsc::UnboundedReceiver<crate::error::Result<AgentMessage>>,
    ) -> Self {
        self.side = Some(rx);
        self
    }

    /// Get a reference to the underlying `AgentSession`.
    ///
    /// Escape hatch for features not yet abstracted by `EngineClient`
    /// (e.g., `list_sessions`, `set_config_option`, permission handling).
    /// Will be removed as the trait surface grows.
    #[must_use]
    pub fn session(&self) -> &AgentSession {
        &self.session
    }

    /// Get a mutable reference to the underlying `AgentSession`.
    ///
    /// Escape hatch for features requiring mutable access (e.g.,
    /// `take_acp_permission_rx`). Will be removed as the trait grows.
    pub fn session_mut(&mut self) -> &mut AgentSession {
        &mut self.session
    }

    /// Consume the client, returning the inner `AgentSession`.
    #[must_use]
    pub fn into_session(self) -> AgentSession {
        self.session
    }
}

impl EngineClient for LocalEngineClient {
    async fn next_message(&mut self) -> Option<Result<AgentMessage, EngineClientError>> {
        let Some(side) = self.side.as_mut() else {
            return self
                .session
                .next_message()
                .await
                .map(|r| r.map_err(EngineClientError::from));
        };
        tokio::select! {
            msg = self.session.next_message() => msg.map(|r| r.map_err(EngineClientError::from)),
            // A closed side channel (feed ended) disables this arm; the
            // agent stream alone decides when the session is over.
            Some(msg) = side.recv() => Some(msg.map_err(EngineClientError::from)),
        }
    }

    async fn send_command(&self, text: &str) -> Result<(), EngineClientError> {
        self.session
            .send_followup(text)
            .map_err(EngineClientError::from)
    }

    async fn query_state(&self) -> Result<Option<AcpSessionState>, EngineClientError> {
        Ok(self.session.session_state())
    }

    fn cancel(&mut self) {
        self.session.cancel();
    }

    fn abort(&mut self) {
        self.session.abort();
    }

    async fn send_config_change(
        &self,
        config_id: String,
        value_id: String,
    ) -> Result<(), EngineClientError> {
        self.session
            .send_config_change(config_id, value_id)
            .map_err(EngineClientError::from)
    }
}

#[cfg(test)]
#[path = "local.tests.rs"]
mod tests;
