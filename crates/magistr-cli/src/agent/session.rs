//! Agent session — owns an `AcpBackend` and its message stream.
//!
//! `AgentSession` provides the interface for consuming agent messages.
//! With ACP as the only backend, there is no trait indirection — the
//! session owns the concrete `AcpBackend` directly.

use tokio::sync::mpsc;
use tracing::instrument;

use crate::agent::message::{AgentMessage, SessionId};
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
    message_rx: mpsc::UnboundedReceiver<Result<AgentMessage>>,
    /// The session ID returned from the most recent Result message.
    session_id: Option<SessionId>,
    /// Optional ACP permission request receiver (present only for ACP backends).
    acp_permission_rx: Option<mpsc::UnboundedReceiver<magistr_acp::AcpPermissionRequest>>,
}

impl AgentSession {
    /// Start a new agent session with the given config.
    #[instrument(skip_all, name = "agent_session_start")]
    pub async fn start(config: super::AgentConfig) -> Result<Self> {
        let config = config.apply_user_defaults();
        let (backend, message_rx, acp_permission_rx) =
            super::backend::AcpBackend::start(config).await?;

        Ok(Self {
            backend: SessionBackend::Acp(backend),
            message_rx,
            session_id: None,
            acp_permission_rx,
        })
    }

    /// Create a test session from a raw message channel.
    ///
    /// No real ACP connection — tests push messages through the returned sender.
    /// Returns `(session, message_sender)`.
    #[cfg(test)]
    pub fn from_channel() -> (Self, mpsc::UnboundedSender<Result<AgentMessage>>) {
        let (msg_tx, msg_rx) = mpsc::unbounded_channel();
        let (followup_tx, _followup_rx) = mpsc::unbounded_channel::<String>();
        let session = Self {
            backend: SessionBackend::Channel {
                follow_ups: std::sync::Arc::new(std::sync::Mutex::new(Vec::new())),
                _followup_tx: followup_tx,
            },
            message_rx: msg_rx,
            session_id: None,
            acp_permission_rx: None,
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
        mpsc::UnboundedSender<Result<AgentMessage>>,
        std::sync::Arc<std::sync::Mutex<Vec<String>>>,
    ) {
        let (msg_tx, msg_rx) = mpsc::unbounded_channel();
        let (followup_tx, _followup_rx) = mpsc::unbounded_channel::<String>();
        let follow_ups = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
        let session = Self {
            backend: SessionBackend::Channel {
                follow_ups: std::sync::Arc::clone(&follow_ups),
                _followup_tx: followup_tx,
            },
            message_rx: msg_rx,
            session_id: None,
            acp_permission_rx: None,
        };
        (session, msg_tx, follow_ups)
    }

    /// Take the ACP permission request receiver, if present.
    ///
    /// Returns `Some` for ACP backends, `None` for test channels.
    /// Can only be called once — subsequent calls return `None`.
    pub fn take_acp_permission_rx(
        &mut self,
    ) -> Option<mpsc::UnboundedReceiver<magistr_acp::AcpPermissionRequest>> {
        self.acp_permission_rx.take()
    }

    /// Spawn a background task that auto-approves all permission requests.
    ///
    /// Use for non-interactive agent sessions (roadmap generation, background
    /// tasks) where there's no TUI modal to prompt the user. Selects
    /// `AllowOnce` when available, falls back to the first option.
    ///
    /// Returns `Some(JoinHandle)` for ACP backends, `None` for test channels.
    /// The handle is fire-and-forget — it exits when the session ends.
    pub fn spawn_auto_approve_permissions(&mut self) -> Option<tokio::task::JoinHandle<()>> {
        use magistr_acp::{AcpPermissionOptionKind, AcpPermissionResponse};

        self.take_acp_permission_rx().map(|mut rx| {
            tokio::spawn(async move {
                while let Some(req) = rx.recv().await {
                    let response = req
                        .options
                        .iter()
                        .find(|o| o.kind == AcpPermissionOptionKind::AllowOnce)
                        .or(req.options.first())
                        .map_or(AcpPermissionResponse::Cancelled, |opt| {
                            AcpPermissionResponse::Selected(opt.option_id.clone())
                        });
                    let _ = req.response_tx.send(response);
                }
            })
        })
    }

    /// Send a follow-up message in the current conversation.
    #[instrument(skip_all)]
    pub fn send_followup(&self, content: &str) -> Result<()> {
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
    /// Returns `None` when the stream is exhausted (session complete).
    pub async fn next_message(&mut self) -> Option<Result<AgentMessage>> {
        let result = self.message_rx.recv().await;

        // Capture session_id from Result messages
        if let Some(Ok(AgentMessage::Result { ref session_id, .. })) = result {
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
        match &mut self.backend {
            SessionBackend::Acp(backend) => backend.abort(),
            #[cfg(test)]
            SessionBackend::Channel { .. } => {}
        }
    }

    /// Gracefully cancel the current operation.
    ///
    /// Sends `session/cancel` so the agent stops after its current tool call.
    #[instrument(skip_all)]
    pub fn cancel(&mut self) {
        match &mut self.backend {
            SessionBackend::Acp(backend) => backend.cancel(),
            #[cfg(test)]
            SessionBackend::Channel { .. } => {}
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

    /// Get the session ID from the most recent completed turn.
    ///
    /// Available after a `AgentMessage::Result` has been received. Can be used
    /// to resume this session later via `--resume`.
    pub fn session_id(&self) -> Option<&SessionId> {
        self.session_id.as_ref()
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
