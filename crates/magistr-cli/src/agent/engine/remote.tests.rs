use super::*;

use std::sync::Arc;
use std::time::Duration;

use magistr_protocol::jsonrpc::{Message, Notification, Response};
use magistr_protocol::transport::TransportError;
use tokio::sync::mpsc;

// ── Mock Transport ──────────────────────────────────────────────────────────

/// Channel-based mock transport for testing `RemoteEngineClient`.
///
/// Sends messages through an mpsc channel pair so tests can inject
/// server responses and notifications.
struct MockTransport {
    connected: bool,
    /// Outgoing messages (client → server). Tests read from `outgoing_rx`.
    outgoing_tx: mpsc::UnboundedSender<Message>,
    /// Incoming messages (server → client). Tests write to `incoming_tx`.
    incoming_rx: mpsc::UnboundedReceiver<Message>,
    /// Track connect/disconnect calls.
    connect_count: Arc<std::sync::atomic::AtomicU32>,
    disconnect_count: Arc<std::sync::atomic::AtomicU32>,
}

/// Test handle for controlling the mock transport from tests.
struct MockTransportHandle {
    /// Send messages to the client (simulates server responses).
    incoming_tx: mpsc::UnboundedSender<Message>,
    /// Read messages sent by the client (captures requests).
    outgoing_rx: mpsc::UnboundedReceiver<Message>,
    connect_count: Arc<std::sync::atomic::AtomicU32>,
    disconnect_count: Arc<std::sync::atomic::AtomicU32>,
}

fn mock_transport() -> (MockTransport, MockTransportHandle) {
    let (outgoing_tx, outgoing_rx) = mpsc::unbounded_channel();
    let (incoming_tx, incoming_rx) = mpsc::unbounded_channel();
    let connect_count = Arc::new(std::sync::atomic::AtomicU32::new(0));
    let disconnect_count = Arc::new(std::sync::atomic::AtomicU32::new(0));

    let transport = MockTransport {
        connected: false,
        outgoing_tx,
        incoming_rx,
        connect_count: Arc::clone(&connect_count),
        disconnect_count: Arc::clone(&disconnect_count),
    };

    let handle = MockTransportHandle {
        incoming_tx,
        outgoing_rx,
        connect_count,
        disconnect_count,
    };

    (transport, handle)
}

impl magistr_protocol::Transport for MockTransport {
    async fn connect(&mut self) -> Result<(), TransportError> {
        self.connected = true;
        self.connect_count
            .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        Ok(())
    }

    async fn disconnect(&mut self) -> Result<(), TransportError> {
        self.connected = false;
        self.disconnect_count
            .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        Ok(())
    }

    async fn send_message(&mut self, msg: &Message) -> Result<(), TransportError> {
        if !self.connected {
            return Err(TransportError::NotConnected);
        }
        self.outgoing_tx
            .send(msg.clone())
            .map_err(|_| TransportError::ConnectionClosed)
    }

    async fn recv_message(&mut self) -> Result<Message, TransportError> {
        if !self.connected {
            return Err(TransportError::NotConnected);
        }
        self.incoming_rx
            .recv()
            .await
            .ok_or(TransportError::ConnectionClosed)
    }

    fn is_connected(&self) -> bool {
        self.connected
    }
}

// ── Construction tests ──────────────────────────────────────────────────────

#[tokio::test]
async fn new_creates_disconnected_client() {
    let (transport, _handle) = mock_transport();
    let client = RemoteEngineClient::new(transport, "test-session".to_owned());
    assert!(!client.is_healthy());
}

#[tokio::test]
async fn connect_establishes_transport() {
    let (transport, handle) = mock_transport();
    let mut client = RemoteEngineClient::new(transport, "test-session".to_owned());
    client.connect().await.expect("connect should succeed");
    assert!(client.is_healthy());
    assert_eq!(
        handle
            .connect_count
            .load(std::sync::atomic::Ordering::SeqCst),
        1
    );
}

#[tokio::test]
async fn disconnect_shuts_down_transport() {
    let (transport, handle) = mock_transport();
    let mut client = RemoteEngineClient::new(transport, "test-session".to_owned());
    client.connect().await.expect("connect should succeed");
    client
        .disconnect()
        .await
        .expect("disconnect should succeed");
    assert!(!client.is_healthy());
    assert_eq!(
        handle
            .disconnect_count
            .load(std::sync::atomic::Ordering::SeqCst),
        1
    );
}

// ── JSON-RPC request tests ──────────────────────────────────────────────────

#[tokio::test]
async fn send_command_serializes_session_command_request() {
    let (transport, mut handle) = mock_transport();
    let mut client = RemoteEngineClient::new(transport, "test-session".to_owned());
    client.connect().await.expect("connect");

    // Spawn a task to respond to the request
    let respond = tokio::spawn(async move {
        // Read the outgoing request
        let msg = handle.outgoing_rx.recv().await.expect("should get request");
        let request = match msg {
            Message::Request(r) => r,
            other => panic!("expected Request, got {other:?}"),
        };

        // Verify it's a session.command request
        assert_eq!(request.method, "session.command");
        let params: serde_json::Value = request.params.expect("should have params");
        assert_eq!(params["session_id"], "test-session");
        assert_eq!(params["command"]["type"], "send_followup");
        assert_eq!(params["command"]["content"], "hello world");

        // Send back a success response
        handle
            .incoming_tx
            .send(Message::Response(Response::success(
                request.id,
                serde_json::json!({"accepted": true, "status": "in_progress"}),
            )))
            .expect("send response");
    });

    client
        .send_command("hello world")
        .await
        .expect("send_command should succeed");

    respond.await.expect("responder task");
}

#[tokio::test]
async fn query_state_serializes_session_status_request() {
    let (transport, mut handle) = mock_transport();
    let mut client = RemoteEngineClient::new(transport, "test-session".to_owned());
    client.connect().await.expect("connect");

    let respond = tokio::spawn(async move {
        let msg = handle.outgoing_rx.recv().await.expect("should get request");
        let request = match msg {
            Message::Request(r) => r,
            other => panic!("expected Request, got {other:?}"),
        };

        assert_eq!(request.method, "session.status");

        handle
            .incoming_tx
            .send(Message::Response(Response::success(
                request.id,
                serde_json::json!({
                    "session_id": "test-session",
                    "status": "in_progress",
                    "phase": "execute",
                    "cost": {"input_tokens": 0, "output_tokens": 0, "total_cost_usd": 0.0, "cache_creation_tokens": 0, "cache_read_tokens": 0}
                }),
            )))
            .expect("send response");
    });

    let state = client
        .query_state()
        .await
        .expect("query_state should succeed");
    assert_eq!(state, Some(AcpSessionState::InProgress));

    respond.await.expect("responder task");
}

// ── Notification → AgentMessage mapping tests ───────────────────────────────

#[tokio::test]
async fn notification_message_received_maps_to_agent_message() {
    let notification = Notification::new(
        "session.event",
        Some(serde_json::json!({
            "session_id": "test",
            "event": {
                "domain": "session",
                "type": "message_received",
                "content": "Hello from the agent"
            }
        })),
    );

    let msg = notification_to_agent_message(&notification);
    assert!(msg.is_some(), "should map to an AgentMessage");

    match msg.unwrap() {
        AgentMessage::Assistant { message, .. } => {
            assert_eq!(message.content.len(), 1);
            match &message.content[0] {
                ContentBlock::Text { text } => assert_eq!(text, "Hello from the agent"),
                other => panic!("expected Text block, got {other:?}"),
            }
        }
        other => panic!("expected Assistant message, got {other:?}"),
    }
}

#[tokio::test]
async fn notification_session_ended_maps_to_result_message() {
    let notification = Notification::new(
        "session.event",
        Some(serde_json::json!({
            "session_id": "test",
            "event": {
                "domain": "session",
                "type": "session_ended",
                "status": "completed",
                "summary": "All done"
            }
        })),
    );

    let msg = notification_to_agent_message(&notification);
    assert!(msg.is_some());

    match msg.unwrap() {
        AgentMessage::Result {
            is_error, result, ..
        } => {
            assert!(!is_error);
            assert_eq!(result.as_deref(), Some("All done"));
        }
        other => panic!("expected Result message, got {other:?}"),
    }
}

#[tokio::test]
async fn notification_tool_started_maps_to_stream_event() {
    let notification = Notification::new(
        "session.event",
        Some(serde_json::json!({
            "session_id": "test",
            "event": {
                "domain": "session",
                "type": "tool_started",
                "tool_name": "Edit",
                "title": "Editing main.rs"
            }
        })),
    );

    let msg = notification_to_agent_message(&notification);
    assert!(msg.is_some());

    match msg.unwrap() {
        AgentMessage::System { subtype, data } => {
            assert_eq!(subtype, "tool_started");
            assert_eq!(data["tool_name"], "Edit");
        }
        other => panic!("expected System message, got {other:?}"),
    }
}

#[tokio::test]
async fn notification_permission_requested_carries_the_request_id() {
    let notification = Notification::new(
        "session.event",
        Some(serde_json::json!({
            "session_id": "test",
            "event": {
                "domain": "session",
                "type": "permission_requested",
                "request_id": "r1",
                "description": "run cargo test",
                "options": ["allow", "deny"]
            }
        })),
    );
    match notification_to_agent_message(&notification).expect("a message") {
        AgentMessage::System { subtype, data } => {
            assert_eq!(subtype, "permission_requested");
            assert_eq!(data["request_id"], "r1");
            assert_eq!(data["description"], "run cargo test");
        }
        other => panic!("expected System message, got {other:?}"),
    }
}

#[tokio::test]
async fn a_permission_request_from_an_older_server_parses_without_an_id() {
    let notification = Notification::new(
        "session.event",
        Some(serde_json::json!({
            "session_id": "test",
            "event": {
                "domain": "session",
                "type": "permission_requested",
                "description": "run cargo test",
                "options": ["allow", "deny"]
            }
        })),
    );
    match notification_to_agent_message(&notification).expect("a message") {
        AgentMessage::System { data, .. } => assert!(data["request_id"].is_null()),
        other => panic!("expected System message, got {other:?}"),
    }
}

#[tokio::test]
async fn unknown_notification_returns_none() {
    let notification = Notification::new("unknown.method", None);
    let msg = notification_to_agent_message(&notification);
    assert!(msg.is_none());
}

// ── Health monitoring tests ─────────────────────────────────────────────────

#[tokio::test]
async fn health_tracks_last_successful_operation() {
    let (transport, _handle) = mock_transport();
    let mut client = RemoteEngineClient::new(transport, "test-session".to_owned());
    assert!(!client.is_healthy());

    client.connect().await.expect("connect");
    assert!(client.is_healthy());

    let elapsed = client.time_since_last_healthy();
    assert!(elapsed < Duration::from_secs(1));
}

// ── Request ID tests ────────────────────────────────────────────────────────

#[tokio::test]
async fn request_ids_increment() {
    let (transport, _handle) = mock_transport();
    let client = RemoteEngineClient::new(transport, "test-session".to_owned());
    let id1 = client.next_request_id();
    let id2 = client.next_request_id();
    let id3 = client.next_request_id();
    assert_eq!(id1, 1);
    assert_eq!(id2, 2);
    assert_eq!(id3, 3);
}

// ── Reconnection policy integration ─────────────────────────────────────────

#[tokio::test]
async fn default_reconnection_policy() {
    let (transport, _handle) = mock_transport();
    let client = RemoteEngineClient::new(transport, "test-session".to_owned());
    let policy = client.reconnection_policy();
    assert_eq!(policy.max_retries, 5);
    assert!(policy.should_retry(0));
    assert!(!policy.should_retry(5));
}

#[tokio::test]
async fn custom_reconnection_policy() {
    let (transport, _handle) = mock_transport();
    let client = RemoteEngineClient::new(transport, "test-session".to_owned())
        .with_reconnection_policy(ReconnectionPolicy::no_retry());
    assert_eq!(client.reconnection_policy().max_retries, 0);
    assert!(!client.reconnection_policy().should_retry(0));
}

// ── Status mapping tests ────────────────────────────────────────────────────

#[test]
fn session_status_to_acp_state_mapping() {
    use magistr_engine::SessionStatus;

    assert_eq!(
        status_to_acp_state(SessionStatus::Created),
        AcpSessionState::Created
    );
    assert_eq!(
        status_to_acp_state(SessionStatus::InProgress),
        AcpSessionState::InProgress
    );
    assert_eq!(
        status_to_acp_state(SessionStatus::Awaiting),
        AcpSessionState::Awaiting
    );
    assert_eq!(
        status_to_acp_state(SessionStatus::Completed),
        AcpSessionState::Completed
    );
    assert_eq!(
        status_to_acp_state(SessionStatus::Failed),
        AcpSessionState::Failed
    );
    assert_eq!(
        status_to_acp_state(SessionStatus::Cancelled),
        AcpSessionState::Cancelled
    );
}

// ── Notification preservation test ──────────────────────────────────────────

#[tokio::test]
async fn notifications_preserved_during_send_command() {
    let (transport, mut handle) = mock_transport();
    let mut client = RemoteEngineClient::new(transport, "test-session".to_owned());
    client.connect().await.expect("connect");

    // Spawn a responder that interleaves notifications with the response
    let respond = tokio::spawn(async move {
        let msg = handle.outgoing_rx.recv().await.expect("should get request");
        let request = match msg {
            Message::Request(r) => r,
            other => panic!("expected Request, got {other:?}"),
        };

        // Send a notification before the response
        handle
            .incoming_tx
            .send(Message::Notification(Notification::new(
                "session.event",
                Some(serde_json::json!({
                    "session_id": "test-session",
                    "event": {
                        "domain": "session",
                        "type": "message_received",
                        "content": "streaming token 1"
                    }
                })),
            )))
            .expect("send notification");

        // Send the response
        handle
            .incoming_tx
            .send(Message::Response(Response::success(
                request.id,
                serde_json::json!({"accepted": true, "status": "in_progress"}),
            )))
            .expect("send response");

        // Send another notification after the response
        handle
            .incoming_tx
            .send(Message::Notification(Notification::new(
                "session.event",
                Some(serde_json::json!({
                    "session_id": "test-session",
                    "event": {
                        "domain": "session",
                        "type": "message_received",
                        "content": "streaming token 2"
                    }
                })),
            )))
            .expect("send notification 2");
    });

    // Send command (which sends a request and waits for response)
    client
        .send_command("hello")
        .await
        .expect("send_command should succeed");

    respond.await.expect("responder task");

    // Both notifications should arrive via next_message()
    let msg1 = client
        .next_message()
        .await
        .expect("should get message 1")
        .expect("should not be error");
    match msg1 {
        AgentMessage::Assistant { message, .. } => match &message.content[0] {
            ContentBlock::Text { text } => assert_eq!(text, "streaming token 1"),
            other => panic!("expected Text, got {other:?}"),
        },
        other => panic!("expected Assistant, got {other:?}"),
    }

    let msg2 = client
        .next_message()
        .await
        .expect("should get message 2")
        .expect("should not be error");
    match msg2 {
        AgentMessage::Assistant { message, .. } => match &message.content[0] {
            ContentBlock::Text { text } => assert_eq!(text, "streaming token 2"),
            other => panic!("expected Text, got {other:?}"),
        },
        other => panic!("expected Assistant, got {other:?}"),
    }
}
