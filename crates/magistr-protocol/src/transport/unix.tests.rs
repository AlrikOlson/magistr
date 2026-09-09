use super::*;
use crate::jsonrpc::{Notification, Request, RequestId, Response};

/// Helper to create a test socket path in a temp directory.
fn test_socket_path(dir: &tempfile::TempDir, name: &str) -> PathBuf {
    dir.path().join(format!("{name}.sock"))
}

#[tokio::test]
async fn roundtrip_request_over_unix_socket() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let path = test_socket_path(&tmp, "roundtrip");

    let listener = UnixSocketListener::bind(&path).expect("bind");

    let client_handle = tokio::spawn({
        let path = path.clone();
        async move {
            let mut client = UnixSocketTransport::connect_to(&path)
                .await
                .expect("connect");

            let req = Message::Request(Request {
                jsonrpc: "2.0".into(),
                id: RequestId::Number(1),
                method: "test/hello".into(),
                params: Some(serde_json::json!({"name": "world"})),
            });
            client.send_message(&req).await.expect("send");

            let resp = client.recv_message().await.expect("recv");
            client.disconnect().await.expect("disconnect");
            resp
        }
    });

    let mut server_transport = listener.accept().await.expect("accept");
    let received = server_transport.recv_message().await.expect("recv");

    // Verify we got the request
    match &received {
        Message::Request(req) => {
            assert_eq!(req.method, "test/hello");
            assert_eq!(req.id, RequestId::Number(1));
        }
        other => panic!("expected Request, got {other:?}"),
    }

    // Send response back
    let resp = Message::Response(Response {
        jsonrpc: "2.0".into(),
        id: RequestId::Number(1),
        result: serde_json::json!({"greeting": "hello world"}),
    });
    server_transport.send_message(&resp).await.expect("send");

    // Verify client got the response
    let client_resp = client_handle.await.expect("join");
    match &client_resp {
        Message::Response(r) => {
            assert_eq!(r.id, RequestId::Number(1));
            assert_eq!(r.result, serde_json::json!({"greeting": "hello world"}));
        }
        other => panic!("expected Response, got {other:?}"),
    }
}

#[tokio::test]
async fn multiple_messages_in_sequence() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let path = test_socket_path(&tmp, "multi");

    let listener = UnixSocketListener::bind(&path).expect("bind");

    let client_handle = tokio::spawn({
        let path = path.clone();
        async move {
            let mut client = UnixSocketTransport::connect_to(&path)
                .await
                .expect("connect");

            for i in 0..5 {
                let notif = Message::Notification(Notification {
                    jsonrpc: "2.0".into(),
                    method: "test/ping".into(),
                    params: Some(serde_json::json!({"seq": i})),
                });
                client.send_message(&notif).await.expect("send");
            }
            client.disconnect().await.expect("disconnect");
        }
    });

    let mut server_transport = listener.accept().await.expect("accept");
    for i in 0..5 {
        let msg = server_transport.recv_message().await.expect("recv");
        match &msg {
            Message::Notification(n) => {
                assert_eq!(n.method, "test/ping");
                assert_eq!(
                    n.params.as_ref().and_then(|p| p.get("seq")),
                    Some(&serde_json::json!(i))
                );
            }
            other => panic!("expected Notification, got {other:?}"),
        }
    }

    client_handle.await.expect("join");
}

#[tokio::test]
async fn from_stream_is_immediately_connected() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let path = test_socket_path(&tmp, "from_stream");

    let listener = UnixSocketListener::bind(&path).expect("bind");

    let _client_handle = tokio::spawn({
        let path = path.clone();
        async move {
            let _stream = UnixStream::connect(&path).await.expect("connect");
        }
    });

    let transport = listener.accept().await.expect("accept");
    assert!(transport.is_connected());
}

#[tokio::test]
async fn send_after_disconnect_returns_not_connected() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let path = test_socket_path(&tmp, "disconnected");

    let listener = UnixSocketListener::bind(&path).expect("bind");

    let _client_handle = tokio::spawn({
        let path = path.clone();
        async move {
            let _stream = UnixStream::connect(&path).await.expect("connect");
        }
    });

    let mut transport = listener.accept().await.expect("accept");
    transport.disconnect().await.expect("disconnect");

    assert!(!transport.is_connected());

    let msg = Message::Notification(Notification {
        jsonrpc: "2.0".into(),
        method: "test/noop".into(),
        params: None,
    });
    let err = transport.send_message(&msg).await.unwrap_err();
    assert!(matches!(err, TransportError::NotConnected));
}

#[tokio::test]
async fn recv_after_disconnect_returns_not_connected() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let path = test_socket_path(&tmp, "recv_disc");

    let listener = UnixSocketListener::bind(&path).expect("bind");

    let _client_handle = tokio::spawn({
        let path = path.clone();
        async move {
            let _stream = UnixStream::connect(&path).await.expect("connect");
        }
    });

    let mut transport = listener.accept().await.expect("accept");
    transport.disconnect().await.expect("disconnect");

    let err = transport.recv_message().await.unwrap_err();
    assert!(matches!(err, TransportError::NotConnected));
}

#[tokio::test]
async fn listener_removes_stale_socket() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let path = test_socket_path(&tmp, "stale");

    // Create a stale file
    std::fs::write(&path, "stale").expect("write stale");
    assert!(path.exists());

    // Binding should succeed by removing the stale file
    let _listener = UnixSocketListener::bind(&path).expect("bind over stale");
    assert!(path.exists()); // Now it's the real socket
}

#[tokio::test]
async fn listener_cleans_up_on_drop() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let path = test_socket_path(&tmp, "cleanup");

    {
        let _listener = UnixSocketListener::bind(&path).expect("bind");
        assert!(path.exists());
    }
    // After drop, socket file should be removed
    assert!(!path.exists());
}

#[tokio::test]
async fn local_path_returns_bound_path() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let path = test_socket_path(&tmp, "local_path");

    let listener = UnixSocketListener::bind(&path).expect("bind");
    assert_eq!(listener.local_path(), path);
}

#[tokio::test]
async fn connect_to_nonexistent_path_returns_io_error() {
    let err = UnixSocketTransport::connect_to("/tmp/magistr-test-nonexistent-socket-12345.sock")
        .await
        .unwrap_err();
    assert!(matches!(err, TransportError::Io(_)));
}
