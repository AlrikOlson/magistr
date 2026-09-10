//! ACP (Agent Client Protocol) backend — the sole agent backend.
//!
//! Provides formal lifecycle management (graceful cancel, session state tracking)
//! via the Agent Client Protocol.
//!
//! ## Architecture
//!
//! ```text
//! ┌─────────────────────────────────┐
//! │         AcpBackend              │
//! │  state tracking, cancel, turns  │
//! └──────────┬──────────────────────┘
//!            │ owns
//! ┌──────────┴──────────────────────┐
//! │   AcpTransport (magistr-acp)    │
//! │  connect/prompt/cancel/close    │
//! │  one Send task: ACP 2.0 client  │
//! └──────────┬──────────────────────┘
//!            │ subprocess
//! ┌──────────┴──────────────────────┐
//! │ magistr acp-agent --provider claude │
//! └─────────────────────────────────┘
//! ```

use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, Mutex};

use magistr_acp::AcpEvent;
use tokio::sync::mpsc;
use tokio::task::JoinHandle;
use tracing::{debug, instrument};

use super::AgentConfig;
use super::message::SessionId;
use super::session::AcpSessionState;
use super::types;
use crate::agent::message::AgentMessage;
use crate::error::{MagistrError, Result};

/// Commands from the AcpBackend to the reader task.
enum BackendCommand {
    /// Send a follow-up prompt.
    Followup(String),
    /// Request graceful cancel.
    Cancel,
    /// List available sessions via ACP `session/list`.
    ListSessions {
        /// Response channel for the session list result.
        response_tx: tokio::sync::oneshot::Sender<
            Result<Vec<agent_client_protocol::schema::v1::SessionInfo>>,
        >,
    },
    /// Set a config option value via ACP `session/set_config_option`.
    SetConfigOption {
        /// Config option ID.
        config_id: String,
        /// New value to set.
        value: agent_client_protocol::schema::v1::SessionConfigOptionValue,
        /// Response channel with updated config options.
        response_tx: tokio::sync::oneshot::Sender<
            Result<Vec<agent_client_protocol::schema::v1::SessionConfigOption>>,
        >,
    },
}

/// ACP backend — communicates via Agent Client Protocol for formal lifecycle management.
pub struct AcpBackend {
    /// Channel to send commands (followup, cancel) to the reader task.
    cmd_tx: mpsc::UnboundedSender<BackendCommand>,
    /// Handle to the background reader task (aborted on drop).
    reader_handle: Option<JoinHandle<()>>,
    /// Current session lifecycle state (shared with reader task).
    state: Arc<Mutex<AcpSessionState>>,
    /// Number of completed turns in this session.
    turn_count: Arc<AtomicU32>,
    /// Agent capabilities negotiated during `initialize` (shared with reader task).
    agent_capabilities: Arc<Mutex<Option<agent_client_protocol::schema::v1::AgentCapabilities>>>,
    /// Process group ID of the child subprocess (for orphan prevention).
    #[cfg(unix)]
    child_pgid: i32,
}

impl AcpBackend {
    /// Start an ACP backend session.
    ///
    /// Connects to the ACP agent via `AcpTransport`, then spawns a reader task
    /// that converts ACP messages to `AgentMessage` values.
    ///
    /// Returns `(backend, message_rx, permission_rx)` — the caller (AgentSession)
    /// owns all three.
    #[instrument(skip_all, name = "acp_backend_start")]
    #[allow(clippy::type_complexity)] // Return tuple is consumed by AgentSession::start only
    pub async fn start(
        config: AgentConfig,
    ) -> Result<(
        Self,
        mpsc::UnboundedReceiver<Result<AgentMessage>>,
        Option<mpsc::UnboundedReceiver<magistr_acp::AcpPermissionRequest>>,
    )> {
        Self::start_resolved(&config.resolve()?).await
    }

    // A session and its permission worker consume the same resolved setup.
    #[allow(clippy::type_complexity)] // Same backend/message/permission ownership as start.
    pub(crate) async fn start_resolved(
        spec: &super::ResolvedSessionSpec,
    ) -> Result<(
        Self,
        mpsc::UnboundedReceiver<Result<AgentMessage>>,
        Option<mpsc::UnboundedReceiver<magistr_acp::AcpPermissionRequest>>,
    )> {
        // Spawn + handshake (+ the orphan guard) — shared with the cockpit.
        let transport = spec.connect_transport().await?;
        #[cfg(unix)]
        let child_pgid = transport.child_pgid();
        let result = Self::start_connected(spec.prompt(), transport).await;
        #[cfg(unix)]
        if result.is_err() && child_pgid > 0 {
            // Failed startup dropped the transport and killed its group. No
            // backend was constructed to unregister it through abort().
            crate::utils::child_guard::unregister_child_pgid(child_pgid);
        }
        result
    }

    // Continue startup on the connected transport before admitting model input.
    #[allow(clippy::type_complexity)] // Same backend/message/permission ownership as start.
    async fn start_connected(
        prompt: &str,
        mut transport: magistr_acp::AcpTransport,
    ) -> Result<(
        Self,
        mpsc::UnboundedReceiver<Result<AgentMessage>>,
        Option<mpsc::UnboundedReceiver<magistr_acp::AcpPermissionRequest>>,
    )> {
        #[cfg(unix)]
        let child_pgid = transport.child_pgid();

        // Take event/permission receivers from transport
        let event_rx = transport
            .read_events()
            .ok_or_else(|| MagistrError::Acp("ACP event channel not available".into()))?;
        let acp_perm_rx = transport.take_permission_rx();

        let state = Arc::new(Mutex::new(AcpSessionState::Created));
        let turn_count = Arc::new(AtomicU32::new(0));
        let agent_capabilities: Arc<
            Mutex<Option<agent_client_protocol::schema::v1::AgentCapabilities>>,
        > = Arc::new(Mutex::new(None));

        // Send initial prompt (AFTER model selection so the agent uses the right model)
        transport
            .prompt(prompt)
            .map_err(|e| MagistrError::Acp(format!("Failed to send initial prompt: {e}")))?;
        debug!("initial prompt sent");

        // Mark as in-progress
        *state.lock().expect("state lock poisoned") = AcpSessionState::InProgress;

        let (msg_tx, msg_rx_out) = mpsc::unbounded_channel();
        let (cmd_tx, cmd_rx) = mpsc::unbounded_channel::<BackendCommand>();

        // Reader owns the transport directly, avoiding a mutex across awaits.
        let reader_handle = spawn_reader(
            transport,
            event_rx,
            cmd_rx,
            msg_tx,
            Arc::clone(&state),
            Arc::clone(&turn_count),
            Arc::clone(&agent_capabilities),
        );

        let backend = AcpBackend {
            cmd_tx,
            reader_handle: Some(reader_handle),
            state,
            turn_count,
            agent_capabilities,
            #[cfg(unix)]
            child_pgid,
        };

        Ok((backend, msg_rx_out, acp_perm_rx))
    }

    /// Get the number of completed turns in this session.
    #[must_use]
    pub fn turn_count(&self) -> u32 {
        self.turn_count.load(Ordering::SeqCst)
    }
}

// Drain turns and service between-turn commands in a single transport owner.
fn spawn_reader(
    mut transport: magistr_acp::AcpTransport,
    mut event_rx: mpsc::UnboundedReceiver<magistr_acp::Result<AcpEvent>>,
    mut cmd_rx: mpsc::UnboundedReceiver<BackendCommand>,
    msg_tx: mpsc::UnboundedSender<Result<AgentMessage>>,
    reader_state: Arc<Mutex<AcpSessionState>>,
    reader_turn_count: Arc<AtomicU32>,
    reader_caps: Arc<Mutex<Option<agent_client_protocol::schema::v1::AgentCapabilities>>>,
) -> JoinHandle<()> {
    tokio::spawn(async move {
        // Session ID captured from Initialized event, needed for set_config_option
        let mut reader_session_id: Option<String> = None;
        loop {
            // Phase 1: Drain events from the current turn
            loop {
                tokio::select! {
                    biased;
                    // Check for cancel commands while draining
                    cmd = cmd_rx.recv() => {
                        match cmd {
                            Some(BackendCommand::Cancel) => {
                                let _ = transport.cancel();
                                // Don't return — wait for the cancelled result to arrive
                            }
                            Some(BackendCommand::Followup(_)) => {
                                // Can't start a new prompt while one is in-flight — ignore
                            }
                            Some(BackendCommand::ListSessions { response_tx }) => {
                                let result = transport.list_sessions(None).await
                                    .map_err(|e| MagistrError::Acp(format!("list_sessions failed: {e}")));
                                let _ = response_tx.send(result);
                            }
                            Some(BackendCommand::SetConfigOption { response_tx, .. }) => {
                                // Can't set config while a prompt is in-flight
                                let _ = response_tx.send(Err(MagistrError::Acp(
                                    "Cannot set config option while a prompt is in-flight".into(),
                                )));
                            }
                            None => return, // Backend dropped
                        }
                    }
                    event = event_rx.recv() => {
                        match event {
                            Some(Ok(acp_event)) => {
                                // Store agent capabilities and session ID on Initialized event
                                if let AcpEvent::Initialized {
                                    agent_capabilities: ref caps,
                                    ref session_id,
                                    ..
                                } = acp_event
                                {
                                    *reader_caps.lock().expect("caps lock poisoned") =
                                        Some((**caps).clone());
                                    reader_session_id = Some(session_id.clone());
                                }

                                let (messages, is_result, is_cancelled) =
                                    convert_acp_event(&acp_event);

                                for msg in messages {
                                    if msg_tx.send(Ok(msg)).is_err() {
                                        return;
                                    }
                                }

                                if is_result {
                                    reader_turn_count.fetch_add(1, Ordering::SeqCst);
                                    *reader_state.lock().expect("state lock poisoned") =
                                        AcpSessionState::Awaiting;
                                    break;
                                }

                                if is_cancelled {
                                    *reader_state.lock().expect("state lock poisoned") =
                                        AcpSessionState::Cancelled;
                                    return;
                                }
                            }
                            Some(Err(e)) => {
                                *reader_state.lock().expect("state lock poisoned") =
                                    AcpSessionState::Failed;
                                let _ = msg_tx.send(Err(MagistrError::Acp(e.to_string())));
                                return;
                            }
                            None => {
                                // Channel closed — transport ended
                                let mut s = reader_state.lock().expect("state lock poisoned");
                                if *s == AcpSessionState::InProgress {
                                    *s = AcpSessionState::Completed;
                                }
                                return;
                            }
                        }
                    }
                }
            }

            if !await_followup(
                &mut transport,
                &mut cmd_rx,
                &msg_tx,
                &reader_state,
                reader_session_id.as_deref(),
            )
            .await
            {
                return;
            }
        }
    })
}

// Stay idle through list/config requests until follow-up input or shutdown.
async fn await_followup(
    transport: &mut magistr_acp::AcpTransport,
    cmd_rx: &mut mpsc::UnboundedReceiver<BackendCommand>,
    msg_tx: &mpsc::UnboundedSender<Result<AgentMessage>>,
    reader_state: &Mutex<AcpSessionState>,
    reader_session_id: Option<&str>,
) -> bool {
    loop {
        match cmd_rx.recv().await {
            Some(BackendCommand::Followup(text)) => {
                if let Err(e) = transport.prompt(&text) {
                    let _ = msg_tx.send(Err(MagistrError::Acp(format!(
                        "Failed to send follow-up: {e}"
                    ))));
                    return false;
                }

                *reader_state.lock().expect("state lock poisoned") = AcpSessionState::InProgress;
                return true; // Begin draining the follow-up turn.
            }
            Some(BackendCommand::ListSessions { response_tx }) => {
                let result = transport
                    .list_sessions(None)
                    .await
                    .map_err(|e| MagistrError::Acp(format!("list_sessions failed: {e}")));
                let _ = response_tx.send(result);
                // Stay in Phase 2 — continue inner loop
            }
            Some(BackendCommand::SetConfigOption {
                config_id,
                value,
                response_tx,
            }) => {
                if let Some(sid) = reader_session_id {
                    let result = transport
                        .set_config_option(sid.to_owned(), config_id, value)
                        .await
                        .map_err(|e| MagistrError::Acp(format!("set_config_option failed: {e}")));
                    let _ = response_tx.send(result);
                } else {
                    let _ = response_tx.send(Err(MagistrError::Acp(
                        "Session not initialized — cannot set config option".into(),
                    )));
                }
                // Stay in Phase 2 — continue inner loop
            }
            Some(BackendCommand::Cancel) => {
                // Cancel while awaiting — just close
                *reader_state.lock().expect("state lock poisoned") = AcpSessionState::Cancelled;
                let _ = transport.close().await;
                return false;
            }
            None => return false, // Backend dropped
        }
    }
}

impl AcpBackend {
    /// Send a follow-up message to continue the conversation.
    pub fn send_followup(&self, content: &str) -> Result<()> {
        self.cmd_tx
            .send(BackendCommand::Followup(content.to_string()))
            .map_err(|_| MagistrError::Acp("ACP backend channel closed".into()))
    }

    /// Fire-and-forget config change — sends the request without waiting for response.
    ///
    /// The agent will process the change and emit a `config_updated` notification,
    /// which arrives through the normal message stream and updates the UI.
    pub fn send_config_change(&self, config_id: String, value_id: String) -> Result<()> {
        let (response_tx, _response_rx) = tokio::sync::oneshot::channel();
        self.cmd_tx
            .send(BackendCommand::SetConfigOption {
                config_id,
                value: agent_client_protocol::schema::v1::SessionConfigOptionValue::ValueId {
                    value: value_id.into(),
                },
                response_tx,
            })
            .map_err(|_| MagistrError::Acp("ACP backend channel closed".into()))
    }

    /// Abort the backend, killing the reader task.
    pub fn abort(&mut self) {
        debug!("aborting ACP backend");
        if let Some(handle) = self.reader_handle.take() {
            handle.abort();
        }
        *self.state.lock().expect("state lock poisoned") = AcpSessionState::Failed;

        // Unregister from the global child-process tracker.
        // The transport's Drop will killpg the process group directly,
        // but we also remove it from the registry to avoid stale entries.
        #[cfg(unix)]
        if self.child_pgid > 0 {
            crate::utils::child_guard::unregister_child_pgid(self.child_pgid);
        }
    }

    /// Gracefully cancel the current operation via `session/cancel`.
    pub fn cancel(&mut self) {
        debug!("cancelling ACP session");
        let _ = self.cmd_tx.send(BackendCommand::Cancel);
        *self.state.lock().expect("state lock poisoned") = AcpSessionState::Cancelled;
    }

    /// List available sessions via ACP `session/list`.
    ///
    /// Sends a request to the reader task which owns the transport. Returns the
    /// session list or an error if the agent doesn't support session listing.
    pub async fn list_sessions(
        &self,
    ) -> Result<Vec<agent_client_protocol::schema::v1::SessionInfo>> {
        let (response_tx, response_rx) = tokio::sync::oneshot::channel();
        self.cmd_tx
            .send(BackendCommand::ListSessions { response_tx })
            .map_err(|_| MagistrError::Acp("ACP backend channel closed".into()))?;
        response_rx
            .await
            .map_err(|_| MagistrError::Acp("list_sessions response channel dropped".into()))?
    }

    /// Set a config option value via ACP `session/set_config_option`.
    ///
    /// Sends the request to the reader task which forwards it to the transport.
    /// Returns the updated config options (full replacement — may include
    /// dependent changes from the agent).
    pub async fn set_config_option(
        &self,
        config_id: String,
        value: agent_client_protocol::schema::v1::SessionConfigOptionValue,
    ) -> Result<Vec<agent_client_protocol::schema::v1::SessionConfigOption>> {
        let (response_tx, response_rx) = tokio::sync::oneshot::channel();
        self.cmd_tx
            .send(BackendCommand::SetConfigOption {
                config_id,
                value,
                response_tx,
            })
            .map_err(|_| MagistrError::Acp("ACP backend channel closed".into()))?;
        response_rx
            .await
            .map_err(|_| MagistrError::Acp("set_config_option response channel dropped".into()))?
    }

    /// Get the ACP session lifecycle state.
    pub fn session_state(&self) -> Option<AcpSessionState> {
        Some(*self.state.lock().expect("state lock poisoned"))
    }

    /// Get the agent capabilities negotiated during `initialize`.
    ///
    /// Returns `None` until the `Initialized` event is received from the transport.
    pub fn agent_capabilities(
        &self,
    ) -> Option<agent_client_protocol::schema::v1::AgentCapabilities> {
        self.agent_capabilities
            .lock()
            .expect("caps lock poisoned")
            .clone()
    }
}

impl Drop for AcpBackend {
    fn drop(&mut self) {
        self.abort();
    }
}

/// Convert a typed `AcpEvent` into `AgentMessage` values.
///
/// Returns a tuple of (messages, is_result, is_cancelled) so the reader task
/// can track session lifecycle state.
fn convert_acp_event(event: &AcpEvent) -> (Vec<AgentMessage>, bool, bool) {
    let event_type = match event {
        AcpEvent::SessionUpdate { .. } => "session_update",
        AcpEvent::PromptResponse { .. } => "prompt_response",
        AcpEvent::Initialized { .. } => "initialized",
        AcpEvent::Terminal { .. } => "terminal",
        AcpEvent::System { .. } => "system",
    };
    debug!(event_type, "converting ACP event");
    match event {
        AcpEvent::SessionUpdate { update, session_id } => {
            let sid = SessionId::new(session_id);
            let messages = types::messages_from_session_update(update, &sid);
            (messages, false, false)
        }
        AcpEvent::PromptResponse {
            response,
            session_id,
        } => {
            let sid = SessionId::new(session_id);
            let msg = types::message_from_prompt_response(response, &sid, 0, 1, None);
            let is_result = matches!(&msg, AgentMessage::Result { .. });
            let is_cancelled = matches!(&msg, AgentMessage::Cancelled { .. });
            (vec![msg], is_result, is_cancelled)
        }
        AcpEvent::Initialized {
            agent_capabilities,
            session_id,
            config_options,
            ..
        } => {
            // Emit a system message so the TUI can display initialization info
            let sys_msg = AgentMessage::System {
                subtype: "initialized".to_string(),
                data: serde_json::json!({
                    "session_id": session_id,
                    "load_session": agent_capabilities.load_session,
                    "prompt_capabilities": {
                        "image": agent_capabilities.prompt_capabilities.image,
                        "audio": agent_capabilities.prompt_capabilities.audio,
                        "embedded_context": agent_capabilities.prompt_capabilities.embedded_context,
                    },
                    "session_list": agent_capabilities.session_capabilities.list.is_some(),
                }),
            };
            let mut msgs = vec![sys_msg];

            // If config options were returned from session/new or session/load,
            // emit a ConfigUpdate message so the TUI picks them up (C7.6)
            if let Some(opts) = config_options
                && !opts.is_empty()
            {
                let options = types::config_options_from_acp(opts);
                msgs.push(AgentMessage::ConfigUpdate { options });
            }

            (msgs, false, false)
        }
        // A terminal's raw bytes ride the wire whole, as a byte array, so
        // a remote cockpit's pane paints what a local one does.
        AcpEvent::Terminal { terminal_id, bytes } => {
            let msg = AgentMessage::System {
                subtype: "terminal_output".to_string(),
                data: serde_json::json!({ "terminal_id": terminal_id, "bytes": bytes }),
            };
            (vec![msg], false, false)
        }
        AcpEvent::System { subtype, data } => {
            let msg = AgentMessage::System {
                subtype: subtype.clone(),
                data: data.clone(),
            };
            (vec![msg], false, false)
        }
    }
}

#[cfg(test)]
#[path = "backend.tests.rs"]
mod tests;
