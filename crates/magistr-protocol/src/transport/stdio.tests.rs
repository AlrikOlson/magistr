use super::*;

use crate::jsonrpc::{Message, Notification, Request};

#[tokio::test]
async fn connect_and_disconnect() {
    let reader: &[u8] = &[];
    let writer = Vec::new();
    let mut transport = StdioTransport::new(reader, writer);

    assert!(!transport.is_connected());

    transport.connect().await.unwrap();
    assert!(transport.is_connected());

    transport.disconnect().await.unwrap();
    assert!(!transport.is_connected());
}

#[tokio::test]
async fn send_before_connect_returns_not_connected() {
    let reader: &[u8] = &[];
    let writer = Vec::new();
    let mut transport = StdioTransport::new(reader, writer);

    let msg = Message::Notification(Notification::new("test", None));
    let result = transport.send_message(&msg).await;
    assert!(matches!(result, Err(TransportError::NotConnected)));
}

#[tokio::test]
async fn recv_before_connect_returns_not_connected() {
    let reader: &[u8] = &[];
    let writer = Vec::new();
    let mut transport = StdioTransport::new(reader, writer);

    let result = transport.recv_message().await;
    assert!(matches!(result, Err(TransportError::NotConnected)));
}

#[tokio::test]
async fn send_and_recv_request_roundtrip() {
    let request = Request::new(1, "session.create", Some(serde_json::json!({"key": "val"})));
    let msg = Message::Request(request);

    // Serialize via one transport, deserialize via another.
    let writer = Vec::new();
    let reader: &[u8] = &[];
    let mut sender = StdioTransport::new(reader, writer);
    sender.connect().await.unwrap();
    sender.send_message(&msg).await.unwrap();

    let written = sender.into_writer();
    let mut receiver = StdioTransport::new(written.as_slice(), Vec::new());
    receiver.connect().await.unwrap();
    let received = receiver.recv_message().await.unwrap();

    match (&msg, &received) {
        (Message::Request(sent), Message::Request(got)) => {
            assert_eq!(sent.id, got.id);
            assert_eq!(sent.method, got.method);
            assert_eq!(sent.params, got.params);
        }
        _ => panic!("expected Request, got {received:?}"),
    }
}

#[tokio::test]
async fn send_and_recv_notification_roundtrip() {
    let notification =
        Notification::new("workflow.event", Some(serde_json::json!({"phase": "plan"})));
    let msg = Message::Notification(notification);

    let writer = Vec::new();
    let reader: &[u8] = &[];
    let mut sender = StdioTransport::new(reader, writer);
    sender.connect().await.unwrap();
    sender.send_message(&msg).await.unwrap();

    let written = sender.into_writer();
    let mut receiver = StdioTransport::new(written.as_slice(), Vec::new());
    receiver.connect().await.unwrap();
    let received = receiver.recv_message().await.unwrap();

    assert!(matches!(received, Message::Notification(_)));
}

#[tokio::test]
async fn recv_on_eof_returns_connection_closed() {
    let reader: &[u8] = &[];
    let mut transport = StdioTransport::new(reader, Vec::new());
    transport.connect().await.unwrap();

    let result = transport.recv_message().await;
    assert!(matches!(result, Err(TransportError::ConnectionClosed)));
}

#[tokio::test]
async fn multiple_messages_sequential() {
    let msgs = vec![
        Message::Notification(Notification::new("a", None)),
        Message::Notification(Notification::new("b", None)),
        Message::Notification(Notification::new("c", None)),
    ];

    let reader: &[u8] = &[];
    let writer = Vec::new();
    let mut sender = StdioTransport::new(reader, writer);
    sender.connect().await.unwrap();
    for msg in &msgs {
        sender.send_message(msg).await.unwrap();
    }

    let written = sender.into_writer();
    let mut receiver = StdioTransport::new(written.as_slice(), Vec::new());
    receiver.connect().await.unwrap();

    for expected in &msgs {
        let received = receiver.recv_message().await.unwrap();
        match (expected, &received) {
            (Message::Notification(e), Message::Notification(r)) => {
                assert_eq!(e.method, r.method);
            }
            _ => panic!("mismatch"),
        }
    }
}
