use std::net::SocketAddr;
use std::time::Duration;

use super::*;
use crate::jsonrpc::{Notification, Request, RequestId, Response};

/// Helper to bind a listener on localhost with an OS-assigned port.
async fn test_listener() -> (TcpTransportListener, SocketAddr) {
    let listener = TcpTransportListener::bind("127.0.0.1:0")
        .await
        .expect("bind");
    let addr = listener.local_addr().expect("local_addr");
    (listener, addr)
}

#[tokio::test]
async fn roundtrip_request_over_tcp() {
    let (listener, addr) = test_listener().await;

    let client_handle = tokio::spawn(async move {
        let mut client = TcpTransport::connect_to(addr, Duration::from_secs(5))
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
    });

    let mut server_transport = listener.accept().await.expect("accept");
    let received = server_transport.recv_message().await.expect("recv");

    match &received {
        Message::Request(req) => {
            assert_eq!(req.method, "test/hello");
            assert_eq!(req.id, RequestId::Number(1));
        }
        other => panic!("expected Request, got {other:?}"),
    }

    let resp = Message::Response(Response {
        jsonrpc: "2.0".into(),
        id: RequestId::Number(1),
        result: serde_json::json!({"greeting": "hello world"}),
    });
    server_transport.send_message(&resp).await.expect("send");

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
    let (listener, addr) = test_listener().await;

    let client_handle = tokio::spawn(async move {
        let mut client = TcpTransport::connect_to(addr, Duration::from_secs(5))
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
    let (listener, addr) = test_listener().await;

    let _client_handle = tokio::spawn(async move {
        let _stream = tokio::net::TcpStream::connect(addr).await.expect("connect");
    });

    let transport = listener.accept().await.expect("accept");
    assert!(transport.is_connected());
}

#[tokio::test]
async fn send_after_disconnect_returns_not_connected() {
    let (listener, addr) = test_listener().await;

    let _client_handle = tokio::spawn(async move {
        let _stream = tokio::net::TcpStream::connect(addr).await.expect("connect");
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
    let (listener, addr) = test_listener().await;

    let _client_handle = tokio::spawn(async move {
        let _stream = tokio::net::TcpStream::connect(addr).await.expect("connect");
    });

    let mut transport = listener.accept().await.expect("accept");
    transport.disconnect().await.expect("disconnect");

    let err = transport.recv_message().await.unwrap_err();
    assert!(matches!(err, TransportError::NotConnected));
}

#[tokio::test]
async fn connect_to_refused_address_returns_io_error() {
    // Bind to get a port, then drop the listener so the port is refused.
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind");
    let addr = listener.local_addr().expect("local_addr");
    drop(listener);

    // Windows retries the SYN for a second or two before it reports the
    // refusal; the timeout has to sit past that.
    let err = TcpTransport::connect_to(addr, Duration::from_secs(10))
        .await
        .unwrap_err();
    assert!(matches!(err, TransportError::Io(_)), "{err:?}");
}

#[tokio::test]
async fn connect_to_timeout_returns_error() {
    // Use a non-routable address to trigger a timeout.
    // 192.0.2.1 is TEST-NET-1 (RFC 5737), should not be routable.
    let addr: SocketAddr = "192.0.2.1:9999".parse().expect("parse");
    let err = TcpTransport::connect_to(addr, Duration::from_millis(100))
        .await
        .unwrap_err();
    // Could be Io (timeout converted) or ConnectionTimeout
    assert!(
        matches!(
            err,
            TransportError::Io(_) | TransportError::ConnectionTimeout
        ),
        "expected Io or ConnectionTimeout, got {err:?}"
    );
}

#[tokio::test]
async fn local_addr_returns_bound_address() {
    let (listener, addr) = test_listener().await;
    assert_eq!(listener.local_addr().expect("local_addr"), addr);
    assert_ne!(addr.port(), 0); // OS should have assigned a real port
}

#[tokio::test]
async fn multiple_clients_accepted_sequentially() {
    let (listener, addr) = test_listener().await;

    for i in 0u32..3 {
        let client_handle = tokio::spawn(async move {
            let mut client = TcpTransport::connect_to(addr, Duration::from_secs(5))
                .await
                .expect("connect");
            let req = Message::Request(Request {
                jsonrpc: "2.0".into(),
                id: RequestId::Number(i64::from(i)),
                method: "test/seq".into(),
                params: None,
            });
            client.send_message(&req).await.expect("send");
            client.disconnect().await.expect("disconnect");
        });

        let mut server = listener.accept().await.expect("accept");
        let msg = server.recv_message().await.expect("recv");
        match msg {
            Message::Request(req) => {
                assert_eq!(req.id, RequestId::Number(i64::from(i)));
            }
            other => panic!("expected Request, got {other:?}"),
        }

        client_handle.await.expect("join");
    }
}
