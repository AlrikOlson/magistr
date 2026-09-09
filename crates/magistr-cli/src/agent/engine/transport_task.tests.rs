use super::*;

use std::sync::Arc;
use std::sync::atomic::AtomicU32;
use std::time::Duration;

use magistr_protocol::jsonrpc::{Message, Notification, RequestId, Response};
use magistr_protocol::transport::TransportError;
use tokio::sync::mpsc;

// ── Mock Transport ──────────────────────────────────────────────────────────

struct MockTransport {
    connected: bool,
    outgoing_tx: mpsc::UnboundedSender<Message>,
    incoming_rx: mpsc::UnboundedReceiver<Message>,
    connect_count: Arc<AtomicU32>,
    disconnect_count: Arc<AtomicU32>,
}

struct MockTransportHandle {
    incoming_tx: mpsc::UnboundedSender<Message>,
    outgoing_rx: mpsc::UnboundedReceiver<Message>,
    connect_count: Arc<AtomicU32>,
    disconnect_count: Arc<AtomicU32>,
}

fn mock_transport() -> (MockTransport, MockTransportHandle) {
    let (outgoing_tx, outgoing_rx) = mpsc::unbounded_channel();
    let (incoming_tx, incoming_rx) = mpsc::unbounded_channel();
    let connect_count = Arc::new(AtomicU32::new(0));
    let disconnect_count = Arc::new(AtomicU32::new(0));

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

impl Transport for MockTransport {
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

// ── Tests ───────────────────────────────────────────────────────────────────

#[tokio::test]
async fn spawn_creates_disconnected_handle() {
    let (transport, _mock) = mock_transport();
    let handle = TransportHandle::spawn(transport);
    assert!(!handle.is_connected());
}

#[tokio::test]
async fn connect_via_command_channel() {
    let (transport, mock) = mock_transport();
    let handle = TransportHandle::spawn(transport);
    handle.connect().await.expect("connect should succeed");
    assert!(handle.is_connected());
    assert_eq!(mock.connect_count.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn disconnect_via_command_channel() {
    let (transport, mock) = mock_transport();
    let handle = TransportHandle::spawn(transport);
    handle.connect().await.expect("connect");
    handle
        .disconnect()
        .await
        .expect("disconnect should succeed");
    assert!(!handle.is_connected());
    assert_eq!(mock.disconnect_count.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn notifications_forwarded_to_channel() {
    let (transport, mock) = mock_transport();
    let mut handle = TransportHandle::spawn(transport);
    handle.connect().await.expect("connect");

    // Send a notification from the "server"
    mock.incoming_tx
        .send(Message::Notification(Notification::new(
            "session.event",
            Some(serde_json::json!({"session_id": "test", "data": "hello"})),
        )))
        .expect("send notification");

    let notification = handle
        .next_notification()
        .await
        .expect("should receive notification")
        .expect("should not be an error");

    assert_eq!(notification.method, "session.event");
}

#[tokio::test]
async fn responses_routed_by_request_id() {
    let (transport, mut mock) = mock_transport();
    let handle = TransportHandle::spawn(transport);
    handle.connect().await.expect("connect");

    let request_id = RequestId::Number(42);
    let request = Message::Request(magistr_protocol::jsonrpc::Request::new(
        42i64,
        "test.method",
        None,
    ));

    // Spawn a task to respond
    let respond = tokio::spawn(async move {
        let _msg = mock.outgoing_rx.recv().await.expect("should get request");
        mock.incoming_tx
            .send(Message::Response(Response::success(
                RequestId::Number(42),
                serde_json::json!({"ok": true}),
            )))
            .expect("send response");
    });

    let result = handle
        .send_request(request, request_id)
        .await
        .expect("request should succeed");

    assert_eq!(result, serde_json::json!({"ok": true}));
    respond.await.expect("responder task");
}

#[tokio::test]
async fn concurrent_requests_routed_correctly() {
    let (transport, mut mock) = mock_transport();
    let handle = Arc::new(TransportHandle::spawn(transport));
    handle.connect().await.expect("connect");

    // Spawn a responder that reads two requests and responds in reverse order
    let respond = tokio::spawn(async move {
        let msg1 = mock.outgoing_rx.recv().await.expect("request 1");
        let msg2 = mock.outgoing_rx.recv().await.expect("request 2");

        let id1 = match &msg1 {
            Message::Request(r) => r.id.clone(),
            _ => panic!("expected request"),
        };
        let id2 = match &msg2 {
            Message::Request(r) => r.id.clone(),
            _ => panic!("expected request"),
        };

        // Respond to request 2 first, then request 1
        mock.incoming_tx
            .send(Message::Response(Response::success(
                id2,
                serde_json::json!({"which": 2}),
            )))
            .expect("send response 2");
        mock.incoming_tx
            .send(Message::Response(Response::success(
                id1,
                serde_json::json!({"which": 1}),
            )))
            .expect("send response 1");
    });

    let handle1 = Arc::clone(&handle);
    let handle2 = Arc::clone(&handle);

    let req1 = tokio::spawn(async move {
        handle1
            .send_request(
                Message::Request(magistr_protocol::jsonrpc::Request::new(
                    1i64, "test.a", None,
                )),
                RequestId::Number(1),
            )
            .await
    });

    let req2 = tokio::spawn(async move {
        handle2
            .send_request(
                Message::Request(magistr_protocol::jsonrpc::Request::new(
                    2i64, "test.b", None,
                )),
                RequestId::Number(2),
            )
            .await
    });

    let result1 = req1.await.expect("join").expect("request 1");
    let result2 = req2.await.expect("join").expect("request 2");

    assert_eq!(result1, serde_json::json!({"which": 1}));
    assert_eq!(result2, serde_json::json!({"which": 2}));

    respond.await.expect("responder task");
}

#[tokio::test]
async fn notifications_not_lost_during_request() {
    let (transport, mut mock) = mock_transport();
    let mut handle = TransportHandle::spawn(transport);
    handle.connect().await.expect("connect");

    let request_id = RequestId::Number(1);
    let request = Message::Request(magistr_protocol::jsonrpc::Request::new(
        1i64,
        "test.method",
        None,
    ));

    // Spawn a responder that sends notification, then response, then another notification
    let respond = tokio::spawn(async move {
        let _msg = mock.outgoing_rx.recv().await.expect("should get request");

        // Notification before response
        mock.incoming_tx
            .send(Message::Notification(Notification::new(
                "event.before",
                Some(serde_json::json!({"seq": 1})),
            )))
            .expect("send notification 1");

        // The response
        mock.incoming_tx
            .send(Message::Response(Response::success(
                RequestId::Number(1),
                serde_json::json!({"ok": true}),
            )))
            .expect("send response");

        // Notification after response
        mock.incoming_tx
            .send(Message::Notification(Notification::new(
                "event.after",
                Some(serde_json::json!({"seq": 2})),
            )))
            .expect("send notification 2");
    });

    let result = handle
        .send_request(request, request_id)
        .await
        .expect("request should succeed");
    assert_eq!(result, serde_json::json!({"ok": true}));

    respond.await.expect("responder task");

    // Both notifications should be available
    let n1 = handle
        .next_notification()
        .await
        .expect("notification 1")
        .expect("not error");
    assert_eq!(n1.method, "event.before");

    let n2 = handle
        .next_notification()
        .await
        .expect("notification 2")
        .expect("not error");
    assert_eq!(n2.method, "event.after");
}

#[tokio::test]
async fn transport_error_fails_pending_requests() {
    let (transport, mut mock) = mock_transport();
    let handle = TransportHandle::spawn(transport);
    handle.connect().await.expect("connect");

    let request = Message::Request(magistr_protocol::jsonrpc::Request::new(
        1i64,
        "test.method",
        None,
    ));

    // Drop the mock incoming_tx to cause ConnectionClosed on recv
    let respond = tokio::spawn(async move {
        // Wait for the request to be sent, then drop the mock to close connection
        let _msg = mock.outgoing_rx.recv().await;
        drop(mock.incoming_tx);
    });

    // The recv on the transport side will get ConnectionClosed, failing this request
    let result = handle.send_request(request, RequestId::Number(1)).await;

    assert!(result.is_err());
    respond.await.expect("responder task");
}

#[tokio::test]
async fn transport_error_signals_notification_stream() {
    let (transport, mock) = mock_transport();
    let mut handle = TransportHandle::spawn(transport);
    handle.connect().await.expect("connect");

    // Drop the mock to close the transport connection
    drop(mock.incoming_tx);
    drop(mock.outgoing_rx);

    // The notification stream should eventually yield an error
    let result = handle.next_notification().await;
    match result {
        Some(Err(_)) => {} // Expected — transport error propagated
        None => {}         // Also acceptable — stream closed
        Some(Ok(n)) => panic!("expected error or None, got notification: {n:?}"),
    }
}

#[tokio::test]
async fn handle_drop_shuts_down_task() {
    let (transport, _mock) = mock_transport();
    let handle = TransportHandle::spawn(transport);
    let task_handle_ref = handle
        .task_handle
        .as_ref()
        .expect("should have task handle");
    let abort_handle = task_handle_ref.abort_handle();

    drop(handle);

    // Give the task a moment to shut down
    tokio::time::sleep(Duration::from_millis(50)).await;
    assert!(abort_handle.is_finished());
}
