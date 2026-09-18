//! Agent session — client observation and compatibility access to the supervisor.
//!
//! Provider residency, turn admission and permission lifecycle belong to the
//! backend's reusable supervisor; this facade tracks the observed session ID.

#[cfg(test)]
use tokio::sync::mpsc;
use tracing::instrument;

use crate::agent::message::{AgentMessage, SessionId};
use crate::agent::observation::ObservationReceiver;
#[cfg(test)]
use crate::agent::observation::{ObservationCapacity, ObservationSender};
use crate::error::Result;

// Canonical definition lives in magistr-core. Re-export for backward compatibility.
pub use magistr_core::AcpSessionState;

// ─── Backend enum ───────────────────────────────────────────────────────────

/// The backend driving this session.
///
/// Real sessions use `AcpBackend`. Tests use `Channel` to inject messages
/// without a real ACP connection.
enum SessionBackend {
    /// Real ACP backend with lifecycle management.
    Acp(super::backend::AcpBackend),
    /// Channel-only backend for testing — no real agent connection.
    #[cfg(test)]
    Channel {
        /// Captured follow-up messages for test assertions.
        follow_ups: std::sync::Arc<std::sync::Mutex<Vec<String>>>,
        /// Sender for follow-ups (keeps the channel alive).
        _followup_tx: mpsc::UnboundedSender<String>,
    },
}

// ─── Session ────────────────────────────────────────────────────────────────

/// A running agent session that streams messages from the ACP backend.
///
/// Owns the backend directly (no trait object). The TUI and command handlers
/// consume this type to receive agent messages.
///
/// When dropped, the backend is aborted to ensure the agent process doesn't
/// outlive the TUI.
pub struct AgentSession {
    /// The active backend.
    backend: SessionBackend,
    /// Receiver for messages from the backend.
    message_rx: ObservationReceiver<Result<AgentMessage>>,
    /// The session ID returned from the most recent terminal response.
    session_id: Option<SessionId>,
    /// The frozen initial model selection; None retains the runtime's model.
    resolved_model: Option<String>,
    /// Compatibility test-channel lifecycle; real lifecycle belongs to the supervisor.
    #[cfg(test)]
    channel_closed: bool,
}

impl AgentSession {
    /// Start a new agent session with the given config; `source` is the
    /// route its authorized source writes take — the host's admitted
    /// workspace, or the refusal every host without one holds.
    #[instrument(skip_all, name = "agent_session_start")]
    pub async fn start(
        config: super::AgentConfig,
        source: magistr_application::effects::SourceRoute,
    ) -> Result<Self> {
        Self::start_checkpointed(config, source, || Ok(())).await
    }

    pub(crate) async fn start_checkpointed(
        config: super::AgentConfig,
        source: magistr_application::effects::SourceRoute,
        before_spawn: impl FnOnce() -> Result<()>,
    ) -> Result<Self> {
        let spec = config.resolve()?.with_source_route(source);
        let resolved_model = spec.model().map(str::to_owned);
        let (backend, message_rx) =
            super::backend::AcpBackend::start_checkpointed(spec, before_spawn).await?;
        Ok(Self {
            backend: SessionBackend::Acp(backend),
            message_rx,
            session_id: None,
            resolved_model,
            #[cfg(test)]
            channel_closed: false,
        })
    }

    /// Create a test session from a raw message channel.
    ///
    /// No real ACP connection — tests push messages through the returned sender.
    /// Returns `(session, message_sender)`.
    #[cfg(test)]
    pub fn from_channel() -> (Self, ObservationSender<Result<AgentMessage>>) {
        let (msg_tx, msg_rx) = crate::agent::observation::channel(ObservationCapacity::DEFAULT);
        let (followup_tx, _followup_rx) = mpsc::unbounded_channel::<String>();
        let session = Self {
            backend: SessionBackend::Channel {
                follow_ups: std::sync::Arc::new(std::sync::Mutex::new(Vec::new())),
                _followup_tx: followup_tx,
            },
            message_rx: msg_rx,
            session_id: None,
            resolved_model: None,
            channel_closed: false,
        };
        (session, msg_tx)
    }

    /// Create a test session that also captures follow-up messages.
    ///
    /// Returns `(session, message_sender, follow_ups)`.
    #[cfg(test)]
    #[allow(clippy::type_complexity)]
    pub fn from_channel_with_followups() -> (
        Self,
        ObservationSender<Result<AgentMessage>>,
        std::sync::Arc<std::sync::Mutex<Vec<String>>>,
    ) {
        let (msg_tx, msg_rx) = crate::agent::observation::channel(ObservationCapacity::DEFAULT);
        let (followup_tx, _followup_rx) = mpsc::unbounded_channel::<String>();
        let follow_ups = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
        let session = Self {
            backend: SessionBackend::Channel {
                follow_ups: std::sync::Arc::clone(&follow_ups),
                _followup_tx: followup_tx,
            },
            message_rx: msg_rx,
            session_id: None,
            resolved_model: None,
            channel_closed: false,
        };
        (session, msg_tx, follow_ups)
    }

    /// Send a follow-up message in the current conversation.
    #[instrument(skip_all)]
    pub fn send_followup(&self, content: &str) -> Result<()> {
        #[cfg(test)]
        if self.channel_closed {
            return Err(crate::error::MagistrError::Acp(
                "Test session is closed".into(),
            ));
        }
        match &self.backend {
            SessionBackend::Acp(backend) => backend.send_followup(content),
            #[cfg(test)]
            SessionBackend::Channel { follow_ups, .. } => {
                follow_ups
                    .lock()
                    .expect("follow_ups lock poisoned")
                    .push(content.to_string());
                Ok(())
            }
        }
    }

    /// Fire-and-forget config change — sends the request without waiting.
    ///
    /// The agent processes the change and emits a `config_updated` notification
    /// through the normal message stream. Returns an error for test channels.
    pub fn send_config_change(&self, config_id: String, value_id: String) -> Result<()> {
        match &self.backend {
            SessionBackend::Acp(backend) => backend.send_config_change(config_id, value_id),
            #[cfg(test)]
            SessionBackend::Channel { .. } => Ok(()), // silently succeed in tests
        }
    }

    /// Get the next message from the agent stream.
    ///
    /// Returns `None` when observation ends. Only a terminal message or the
    /// supervisor snapshot establishes the provider's actual turn outcome.
    pub async fn next_message(&mut self) -> Option<Result<AgentMessage>> {
        let result = self.message_rx.recv().await;

        // Cancellation is a terminal response on the same resumable session.
        if let Some(Ok(
            AgentMessage::Result { ref session_id, .. }
            | AgentMessage::Cancelled { ref session_id, .. },
        )) = result
        {
            self.session_id = Some(session_id.clone());
        }

        result
    }

    /// Abort the backend, killing the agent process.
    ///
    /// Called automatically on drop, but can be called explicitly for
    /// immediate cleanup.
    #[instrument(skip_all)]
    pub fn abort(&mut self) {
        #[cfg(test)]
        {
            self.channel_closed = true;
        }
        match &mut self.backend {
            SessionBackend::Acp(backend) => backend.abort(),
            #[cfg(test)]
            SessionBackend::Channel { .. } => {}
        }
    }

    /// Gracefully cancel the current operation.
    ///
    /// Sends `session/cancel` and ends this session's usable stream.
    /// Follow-up input requires a new session after cancellation.
    #[instrument(skip_all)]
    pub fn cancel(&mut self) {
        #[cfg(test)]
        {
            self.channel_closed = true;
        }
        match &mut self.backend {
            SessionBackend::Acp(backend) => backend.cancel(),
            #[cfg(test)]
            SessionBackend::Channel { .. } => {}
        }
    }

    /// Request cancellation while retaining the session for another turn after acknowledgment.
    pub fn cancel_turn(&self) -> Result<()> {
        match &self.backend {
            SessionBackend::Acp(backend) => backend.cancel_turn(),
            #[cfg(test)]
            SessionBackend::Channel { .. } => Ok(()),
        }
    }

    /// Wind the session down the way an interrupt does: no further turn,
    /// the active one cancelled, the transport closed after its terminal.
    pub fn interrupt(&self) -> Result<()> {
        match &self.backend {
            SessionBackend::Acp(backend) => backend.interrupt(),
            #[cfg(test)]
            SessionBackend::Channel { .. } => Ok(()),
        }
    }

    /// End residency and await the owned transport's cleanup.
    pub async fn terminate_session(&mut self) -> Result<()> {
        #[cfg(test)]
        {
            self.channel_closed = true;
        }
        match &mut self.backend {
            SessionBackend::Acp(backend) => backend.terminate_session().await,
            #[cfg(test)]
            SessionBackend::Channel { .. } => Ok(()),
        }
    }

    /// Remove this client's observation without cancelling provider work.
    pub fn detach_client(&mut self) {
        match &mut self.backend {
            SessionBackend::Acp(backend) => backend.detach_client(),
            #[cfg(test)]
            SessionBackend::Channel { .. } => {}
        }
        self.message_rx.close();
    }

    /// Precise cancellation, outcome and residency facts for real sessions.
    pub fn supervisor_snapshot(
        &self,
    ) -> Option<super::session_supervisor::SessionSupervisorSnapshot> {
        match &self.backend {
            SessionBackend::Acp(backend) => Some(backend.supervisor_snapshot()),
            #[cfg(test)]
            SessionBackend::Channel { .. } => None,
        }
    }

    /// The provider process this session's supervisor registered — the
    /// runtime's own handle for a dispatch record; `None` for test channels
    /// and in-process agents.
    #[must_use]
    pub fn provider_pid(&self) -> Option<u32> {
        match &self.backend {
            SessionBackend::Acp(backend) => backend.provider_pid(),
            #[cfg(test)]
            SessionBackend::Channel { .. } => None,
        }
    }

    /// Get the ACP session lifecycle state.
    ///
    /// Returns `Some(state)` for real ACP sessions, `None` for test channels.
    pub fn session_state(&self) -> Option<AcpSessionState> {
        match &self.backend {
            SessionBackend::Acp(backend) => backend.session_state(),
            #[cfg(test)]
            SessionBackend::Channel { .. } => None,
        }
    }

    /// Get the session ID from the most recent terminal response.
    ///
    /// Available after a result or cancellation has been received. Can be used
    /// to resume this session later via `--resume`.
    pub fn session_id(&self) -> Option<&SessionId> {
        self.session_id.as_ref()
    }

    /// Initial model after defaults and governance; None preserves runtime state.
    #[must_use]
    pub fn resolved_model(&self) -> Option<&str> {
        self.resolved_model.as_deref()
    }

    /// Get the agent capabilities negotiated during `initialize`.
    ///
    /// Returns `None` for test channels or before the `Initialized` event is received.
    pub fn agent_capabilities(
        &self,
    ) -> Option<agent_client_protocol::schema::v1::AgentCapabilities> {
        match &self.backend {
            SessionBackend::Acp(backend) => backend.agent_capabilities(),
            #[cfg(test)]
            SessionBackend::Channel { .. } => None,
        }
    }

    /// List available sessions via ACP `session/list`.
    ///
    /// Delegates to the ACP backend. Returns an empty vec for test channels.
    pub async fn list_sessions(
        &self,
    ) -> crate::error::Result<Vec<agent_client_protocol::schema::v1::SessionInfo>> {
        match &self.backend {
            SessionBackend::Acp(backend) => backend.list_sessions().await,
            #[cfg(test)]
            SessionBackend::Channel { .. } => Ok(vec![]),
        }
    }

    /// Set a config option value via ACP `session/set_config_option`.
    ///
    /// Delegates to the ACP backend. Returns the full updated config options.
    /// Returns an error for test channels.
    pub async fn set_config_option(
        &self,
        config_id: String,
        value: agent_client_protocol::schema::v1::SessionConfigOptionValue,
    ) -> crate::error::Result<Vec<agent_client_protocol::schema::v1::SessionConfigOption>> {
        match &self.backend {
            SessionBackend::Acp(backend) => backend.set_config_option(config_id, value).await,
            #[cfg(test)]
            SessionBackend::Channel { .. } => Err(crate::error::MagistrError::Acp(
                "set_config_option not available for test channels".into(),
            )),
        }
    }
}

impl Drop for AgentSession {
    fn drop(&mut self) {
        self.abort();
    }
}

#[cfg(test)]
#[path = "session.tests.rs"]
mod tests;
