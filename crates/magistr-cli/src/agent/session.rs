//! Agent session — owns an `AcpBackend` and its message stream.
//!
//! `AgentSession` provides the interface for consuming agent messages.
//! With ACP as the only backend, there is no trait indirection — the
//! session owns the concrete `AcpBackend` directly.

use std::sync::{Arc, Mutex};
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
    /// The frozen initial model selection; None retains the runtime's model.
    resolved_model: Option<String>,
    /// The owned policy worker ends on cancel/drop; its grant lock orders cancellation.
    permission_worker: Option<tokio::task::JoinHandle<()>>,
    permission_stop: Option<tokio::sync::oneshot::Sender<()>>,
    permission_cancelled: Arc<Mutex<bool>>,
    /// Optional ACP permission request receiver (present only for ACP backends).
    acp_permission_rx: Option<mpsc::UnboundedReceiver<magistr_acp::AcpPermissionRequest>>,
}

impl AgentSession {
    /// Start a new agent session with the given config.
    #[instrument(skip_all, name = "agent_session_start")]
    pub async fn start(config: super::AgentConfig) -> Result<Self> {
        let spec = config.resolve()?;
        let (backend, message_rx, acp_permission_rx) =
            super::backend::AcpBackend::start_resolved(&spec).await?;
        let resolved_model = spec.model().map(str::to_owned);
        let (policy, handling) = spec.into_permission_policy();

        let mut session = Self {
            backend: SessionBackend::Acp(backend),
            message_rx,
            session_id: None,
            resolved_model,
            acp_permission_rx,
            permission_worker: None,
            permission_stop: None,
            permission_cancelled: Arc::new(Mutex::new(false)),
        };
        session.spawn_permission_worker(Box::new(policy), handling);
        Ok(session)
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
            resolved_model: None,
            acp_permission_rx: None,
            permission_worker: None,
            permission_stop: None,
            permission_cancelled: Arc::new(Mutex::new(false)),
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
            resolved_model: None,
            acp_permission_rx: None,
            permission_worker: None,
            permission_stop: None,
            permission_cancelled: Arc::new(Mutex::new(false)),
        };
        (session, msg_tx, follow_ups)
    }

    /// Take the ACP permission request receiver, if present.
    ///
    /// Real startup already transfers this receiver to the shared policy worker,
    /// so successfully started sessions return `None`. Before attachment, ownership
    /// can be taken only once; subsequent calls also return `None`.
    pub fn take_acp_permission_rx(
        &mut self,
    ) -> Option<mpsc::UnboundedReceiver<magistr_acp::AcpPermissionRequest>> {
        self.acp_permission_rx.take()
    }

    /// Attach the shared policy to the session's sole request receiver.
    fn spawn_permission_worker(
        &mut self,
        mut policy: Box<dyn magistr_cockpit::agent::SessionPolicy>,
        handling: crate::permissions::NoninteractivePolicy,
    ) -> bool {
        let Some(mut rx) = self.take_acp_permission_rx() else {
            return false;
        };
        let cancelled = Arc::clone(&self.permission_cancelled);
        let (stop_tx, mut stop_rx) = tokio::sync::oneshot::channel();
        self.permission_stop = Some(stop_tx);
        self.permission_worker = Some(tokio::spawn(async move {
            loop {
                let request = tokio::select! {
                    biased;
                    _ = &mut stop_rx => {
                        rx.close();
                        while let Ok(request) = rx.try_recv() {
                            let _ = request.response_tx.send(magistr_acp::AcpPermissionResponse::Cancelled);
                        }
                        return;
                    }
                    request = rx.recv() => request,
                };
                let Some(request) = request else {
                    break;
                };
                let response = super::noninteractive_response::noninteractive_response(
                    policy.as_mut(),
                    &request,
                    handling,
                );
                // Cancellation and answering serialize through this lock. Once cancel
                // returns no queued request can receive a new grant.
                let is_cancelled = cancelled
                    .lock()
                    .expect("permission cancellation lock poisoned");
                let response = if *is_cancelled {
                    magistr_acp::AcpPermissionResponse::Cancelled
                } else {
                    response
                };
                let _ = request.response_tx.send(response);
            }
        }));
        true
    }

    fn stop_permission_worker(&mut self) {
        *self
            .permission_cancelled
            .lock()
            .expect("permission cancellation lock poisoned") = true;
        if let Some(stop) = self.permission_stop.take() {
            let _ = stop.send(());
        }
        // The owned worker finishes its short cancellation drain asynchronously.
        self.permission_worker.take();
        if let Some(mut requests) = self.acp_permission_rx.take() {
            requests.close();
            while let Ok(request) = requests.try_recv() {
                let _ = request
                    .response_tx
                    .send(magistr_acp::AcpPermissionResponse::Cancelled);
            }
        }
    }

    /// Send a follow-up message in the current conversation.
    #[instrument(skip_all)]
    pub fn send_followup(&self, content: &str) -> Result<()> {
        if *self
            .permission_cancelled
            .lock()
            .expect("permission cancellation lock poisoned")
        {
            return Err(crate::error::MagistrError::Acp(
                "Cannot continue a cancelled or aborted session; start a new session".into(),
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
        self.stop_permission_worker();
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
        self.stop_permission_worker();
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
