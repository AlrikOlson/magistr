use super::*;
use crate::jsonrpc::{Message, RequestId, Response};
use crate::transport::TransportError;
use std::collections::VecDeque;
use std::sync::Mutex;

/// A mock transport that returns pre-programmed responses.
struct MockTransport {
    responses: Mutex<VecDeque<Message>>,
    sent: Mutex<Vec<Message>>,
    connected: bool,
}

impl MockTransport {
    fn new() -> Self {
        Self {
            responses: Mutex::new(VecDeque::new()),
            sent: Mutex::new(Vec::new()),
            connected: false,
        }
    }

    fn queue_response(&self, result: serde_json::Value) {
        let resp = Response::success(RequestId::Number(1), result);
        self.responses
            .lock()
            .unwrap()
            .push_back(Message::Response(resp));
    }
}

impl crate::transport::Transport for MockTransport {
    async fn connect(&mut self) -> Result<(), TransportError> {
        self.connected = true;
        Ok(())
    }

    async fn disconnect(&mut self) -> Result<(), TransportError> {
        self.connected = false;
        Ok(())
    }

    async fn send_message(&mut self, msg: &Message) -> Result<(), TransportError> {
        if !self.connected {
            return Err(TransportError::NotConnected);
        }
        self.sent.lock().unwrap().push(msg.clone());
        Ok(())
    }

    async fn recv_message(&mut self) -> Result<Message, TransportError> {
        if !self.connected {
            return Err(TransportError::NotConnected);
        }
        self.responses
            .lock()
            .unwrap()
            .pop_front()
            .ok_or(TransportError::ConnectionClosed)
    }

    fn is_connected(&self) -> bool {
        self.connected
    }
}

// ── MagistrClient ───────────────────────────────────────────────────────

#[tokio::test]
async fn connect_and_disconnect() {
    let transport = MockTransport::new();
    let mut client = MagistrClient::new(transport);

    client.connect().await.unwrap();
    assert!(client.is_connected());

    client.disconnect().await.unwrap();
    assert!(!client.is_connected());
}

#[tokio::test]
async fn system_version() {
    let transport = MockTransport::new();
    transport.queue_response(serde_json::json!({
        "protocol_version": "0.1.0",
        "server_version": "0.2.0"
    }));

    let mut client = MagistrClient::new(transport);
    client.connect().await.unwrap();

    let result = client.system_version().await.unwrap();
    assert_eq!(result.protocol_version, "0.1.0");
    assert_eq!(result.server_version, "0.2.0");
}

#[tokio::test]
async fn system_health() {
    let transport = MockTransport::new();
    transport.queue_response(serde_json::json!({
        "status": "ok",
        "uptime_secs": 42,
        "client_count": 1,
        "session_id": "test",
        "version": "0.1.0"
    }));

    let mut client = MagistrClient::new(transport);
    client.connect().await.unwrap();

    let result = client.system_health().await.unwrap();
    assert_eq!(result.status, "ok");
    assert_eq!(result.uptime_secs, 42);
}

#[tokio::test]
async fn system_shutdown() {
    let transport = MockTransport::new();
    transport.queue_response(serde_json::json!({
        "shutting_down": true
    }));

    let mut client = MagistrClient::new(transport);
    client.connect().await.unwrap();

    let result = client.system_shutdown().await.unwrap();
    assert!(result.shutting_down);
}

#[tokio::test]
async fn session_status() {
    let transport = MockTransport::new();
    transport.queue_response(serde_json::json!({
        "session_id": "test-1",
        "status": "in_progress",
        "phase": "executing",
        "cost": {
            "total_usd": 0.05,
            "input_tokens": 100,
            "output_tokens": 50,
            "turns": 2
        }
    }));

    let mut client = MagistrClient::new(transport);
    client.connect().await.unwrap();

    let result = client.session_status().await.unwrap();
    assert_eq!(result.session_id, "test-1");
}

#[tokio::test]
async fn call_before_connect_fails() {
    let transport = MockTransport::new();
    let mut client = MagistrClient::new(transport);

    let result = client.system_version().await;
    assert!(result.is_err());
}

#[tokio::test]
async fn request_ids_increment() {
    let transport = MockTransport::new();
    // Queue two responses
    transport.queue_response(serde_json::json!({
        "protocol_version": "0.1.0",
        "server_version": "0.1.0"
    }));
    transport.queue_response(serde_json::json!({
        "shutting_down": false
    }));

    let mut client = MagistrClient::new(transport);
    client.connect().await.unwrap();

    let _ = client.system_version().await;
    let _ = client.system_shutdown().await;

    let sent = client.transport.sent.lock().unwrap();
    // Extract request IDs from sent messages
    let ids: Vec<_> = sent
        .iter()
        .filter_map(|m| {
            if let Message::Request(r) = m
                && let RequestId::Number(n) = &r.id
            {
                return Some(*n);
            }
            None
        })
        .collect();
    assert_eq!(ids, vec![1, 2]);
}
