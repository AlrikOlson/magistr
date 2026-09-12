//! Remote engine client — connects to a magistr-server via JSON-RPC transport.
//!
//! Implements `EngineClient` by delegating to a background `TransportHandle`
//! that exclusively owns the transport. Notifications and responses are demuxed
//! by the background task, eliminating mutex contention and notification loss.

use std::sync::atomic::{AtomicI64, Ordering};
use std::time::Duration;

use magistr_engine::SessionCommand;
use magistr_protocol::Transport;
use magistr_protocol::jsonrpc::{Message, Notification, Request};
use magistr_protocol::methods;
use magistr_protocol::notification::EventNotification;

use super::client::EngineClient;
use super::error::EngineClientError;
use super::reconnect::ReconnectionPolicy;
use super::transport_task::TransportHandle;
use crate::agent::message::{AgentMessage, AssistantContent, ContentBlock, SessionId};
use crate::agent::session::AcpSessionState;

/// Remote engine client that communicates with a magistr-server over a transport.
///
/// Uses a background `TransportHandle` to avoid mutex contention between
/// reading notifications and sending requests. The transport is exclusively
/// owned by the background task.
pub struct RemoteEngineClient {
    /// Handle to the background transport task.
    handle: TransportHandle,

    /// Session ID on the remote server.
    session_id: String,

    /// Monotonically increasing request ID counter.
    next_id: AtomicI64,

    /// Policy for reconnection on transport failures.
    reconnect_policy: ReconnectionPolicy,
}

impl RemoteEngineClient {
    /// Create a new remote engine client (initially disconnected).
    ///
    /// Spawns a background task that exclusively owns the transport.
    /// Call [`connect`] before sending commands.
    pub fn new(transport: impl Transport + 'static, session_id: String) -> Self {
        Self {
            handle: TransportHandle::spawn(transport),
            session_id,
            next_id: AtomicI64::new(1),
            reconnect_policy: ReconnectionPolicy::default(),
        }
    }

    /// Override the default reconnection policy.
    #[must_use]
    pub fn with_reconnection_policy(mut self, policy: ReconnectionPolicy) -> Self {
        self.reconnect_policy = policy;
        self
    }

    /// Get the current reconnection policy.
    #[must_use]
    pub fn reconnection_policy(&self) -> &ReconnectionPolicy {
        &self.reconnect_policy
    }

    /// Get the session ID.
    #[must_use]
    pub fn session_id(&self) -> &str {
        &self.session_id
    }

    /// Whether the connection is currently healthy.
    #[must_use]
    pub fn is_healthy(&self) -> bool {
        self.handle.is_connected()
    }

    /// Time elapsed since the last successful operation.
    ///
    /// Returns `Duration::MAX` if no successful operation has occurred.
    pub fn time_since_last_healthy(&self) -> Duration {
        self.handle.time_since_last_healthy()
    }

    /// Get the next monotonically increasing request ID.
    pub(crate) fn next_request_id(&self) -> i64 {
        self.next_id.fetch_add(1, Ordering::SeqCst)
    }

    /// Establish the transport connection.
    pub async fn connect(&mut self) -> Result<(), EngineClientError> {
        self.handle.connect().await
    }

    /// Disconnect the transport.
    pub async fn disconnect(&mut self) -> Result<(), EngineClientError> {
        self.handle.disconnect().await
    }

    /// Send a JSON-RPC request and wait for the matching response.
    async fn request(
        &self,
        method: &str,
        params: Option<serde_json::Value>,
    ) -> Result<serde_json::Value, EngineClientError> {
        let id = self.next_request_id();
        let request = Request::new(id, method, params);
        let request_id = request.id.clone();
        let message = Message::Request(request);

        self.handle.send_request(message, request_id).await
    }

    /// Put one [`SessionCommand`] to the session on the server and hand
    /// back what `session.command` answered.
    ///
    /// The verb behind [`EngineClient::send_command`] and
    /// [`EngineClient::send_config_change`], and the only way for a
    /// client outside this module — the cockpit's attach relay — to
    /// send a command the trait has no method for: `Cancel` is a
    /// *session* cancel on the server, not [`EngineClient::cancel`],
    /// which shuts this end's transport down.
    pub async fn session_command(
        &self,
        command: SessionCommand,
    ) -> Result<serde_json::Value, EngineClientError> {
        let params = serde_json::json!({
            "session_id": self.session_id,
            "command": command,
        });
        self.request(methods::method::SESSION_COMMAND, Some(params))
            .await
    }
}

impl EngineClient for RemoteEngineClient {
    async fn next_message(&mut self) -> Option<Result<AgentMessage, EngineClientError>> {
        loop {
            let notification = match self.handle.next_notification().await {
                Some(Ok(n)) => n,
                Some(Err(e)) => return Some(Err(e)),
                None => return None,
            };

            if let Some(agent_msg) = notification_to_agent_message(&notification) {
                return Some(Ok(agent_msg));
            }
            // Unknown notification type — skip and keep reading.
        }
    }

    async fn send_command(&self, text: &str) -> Result<(), EngineClientError> {
        let result = self
            .session_command(SessionCommand::SendFollowup {
                content: text.to_owned(),
            })
            .await?;

        // Check if the command was accepted.
        let accepted = result
            .get("accepted")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);

        if !accepted {
            return Err(EngineClientError::InvalidState {
                operation: "send_command".to_owned(),
                state: result
                    .get("status")
                    .and_then(|v| v.as_str())
                    .unwrap_or("unknown")
                    .to_owned(),
            });
        }

        Ok(())
    }

    async fn query_state(&self) -> Result<Option<AcpSessionState>, EngineClientError> {
        let params = serde_json::json!({
            "session_id": self.session_id,
        });
        let result = self
            .request(methods::method::SESSION_STATUS, Some(params))
            .await?;

        let status_str = result
            .get("status")
            .and_then(|v| v.as_str())
            .ok_or_else(|| EngineClientError::Protocol("missing status field".to_owned()))?;

        let status: magistr_engine::SessionStatus =
            serde_json::from_value(serde_json::Value::String(status_str.to_owned()))
                .map_err(|e| EngineClientError::Protocol(format!("invalid status: {e}")))?;

        Ok(Some(status_to_acp_state(status)))
    }

    fn cancel(&mut self) {
        self.handle.shutdown();
    }

    fn abort(&mut self) {
        self.handle.shutdown();
    }

    async fn send_config_change(
        &self,
        config_id: String,
        value_id: String,
    ) -> Result<(), EngineClientError> {
        self.session_command(SessionCommand::SetConfig {
            key: config_id,
            value: value_id,
        })
        .await?;
        Ok(())
    }
}

// ── Notification → AgentMessage mapping ─────────────────────────────────────

/// Convert a JSON-RPC notification from the server into an `AgentMessage`.
///
/// Returns `None` for unrecognized notification types.
pub(crate) fn notification_to_agent_message(notification: &Notification) -> Option<AgentMessage> {
    let params = notification.params.as_ref()?;

    // Try to deserialize as an EventNotification.
    let event_notif: EventNotification = serde_json::from_value(params.clone()).ok()?;
    let session_id = SessionId::new(&event_notif.session_id);

    match event_notif.event {
        magistr_engine::EngineEvent::Session(session_event) => {
            session_event_to_agent_message(session_event, session_id)
        }
        magistr_engine::EngineEvent::Workflow(workflow_event) => Some(AgentMessage::System {
            subtype: "workflow".to_owned(),
            data: serde_json::to_value(&workflow_event).ok()?,
        }),
        magistr_engine::EngineEvent::Cost(cost_event) => Some(AgentMessage::System {
            subtype: "cost".to_owned(),
            data: serde_json::to_value(&cost_event).ok()?,
        }),
        magistr_engine::EngineEvent::Roadmap(roadmap_event) => Some(AgentMessage::System {
            subtype: "roadmap".to_owned(),
            data: serde_json::to_value(&roadmap_event).ok()?,
        }),
    }
}

/// Map a `SessionEvent` to an `AgentMessage`.
fn session_event_to_agent_message(
    event: magistr_engine::session::SessionEvent,
    session_id: SessionId,
) -> Option<AgentMessage> {
    use magistr_engine::session::SessionEvent;

    match event {
        SessionEvent::MessageReceived { content } => Some(AgentMessage::Assistant {
            parent_tool_use_id: None,
            message: AssistantContent {
                model: String::new(),
                content: vec![ContentBlock::Text { text: content }],
            },
            session_id: Some(session_id),
        }),

        SessionEvent::SessionEnded { status, summary } => Some(AgentMessage::Result {
            subtype: "session_ended".to_owned(),
            duration_ms: 0,
            duration_api_ms: 0,
            is_error: status == magistr_engine::SessionStatus::Failed,
            num_turns: 0,
            session_id,
            total_cost_usd: None,
            usage: None,
            result: summary,
        }),

        SessionEvent::ToolStarted { tool_name, title } => Some(AgentMessage::System {
            subtype: "tool_started".to_owned(),
            data: serde_json::json!({
                "tool_name": tool_name,
                "title": title,
            }),
        }),

        SessionEvent::ToolCompleted { tool_name, success } => Some(AgentMessage::System {
            subtype: "tool_completed".to_owned(),
            data: serde_json::json!({
                "tool_name": tool_name,
                "success": success,
            }),
        }),

        SessionEvent::CostUpdated {
            total_cost_usd,
            turn_cost_usd: _,
        } => Some(AgentMessage::UsageUpdate {
            context_tokens_used: 0,
            context_window_size: 0,
            cost_usd: Some(total_cost_usd),
        }),

        SessionEvent::TurnCompleted { turn, duration_ms } => Some(AgentMessage::System {
            subtype: "turn_completed".to_owned(),
            data: serde_json::json!({
                "turn": turn,
                "duration_ms": duration_ms,
            }),
        }),

        SessionEvent::PermissionRequested {
            request_id,
            description,
            options,
        } => Some(AgentMessage::System {
            subtype: "permission_requested".to_owned(),
            data: serde_json::json!({
                "request_id": request_id,
                "description": description,
                "options": options,
            }),
        }),

        SessionEvent::PhaseChanged { from, to } => Some(AgentMessage::System {
            subtype: "phase_changed".to_owned(),
            data: serde_json::json!({
                "from": from,
                "to": to,
            }),
        }),
    }
}

// ── Status mapping ──────────────────────────────────────────────────────────

/// Map `SessionStatus` (engine domain) to `AcpSessionState` (agent session).
pub(crate) fn status_to_acp_state(status: magistr_engine::SessionStatus) -> AcpSessionState {
    match status {
        magistr_engine::SessionStatus::Created => AcpSessionState::Created,
        magistr_engine::SessionStatus::InProgress => AcpSessionState::InProgress,
        magistr_engine::SessionStatus::Awaiting => AcpSessionState::Awaiting,
        magistr_engine::SessionStatus::Completed => AcpSessionState::Completed,
        magistr_engine::SessionStatus::Failed => AcpSessionState::Failed,
        magistr_engine::SessionStatus::Cancelled => AcpSessionState::Cancelled,
    }
}

#[cfg(test)]
#[path = "remote.tests.rs"]
mod tests;
