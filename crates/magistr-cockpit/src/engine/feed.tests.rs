use std::time::Duration;

use tokio::sync::mpsc::UnboundedReceiver;

use super::*;

async fn next(rx: &mut UnboundedReceiver<Msg>) -> EngineEvent {
    let msg = tokio::time::timeout(Duration::from_secs(5), rx.recv())
        .await
        .expect("an event within 5 s")
        .expect("the inbox is open");
    match msg {
        Msg::Engine(event) => event,
        other => panic!("not an engine event: {other:?}"),
    }
}

#[cfg(unix)]
#[tokio::test]
async fn waits_for_the_socket_relays_frames_and_reattaches_after_a_hangup() {
    use tokio::io::AsyncWriteExt;
    use tokio::net::UnixListener;

    let tmp = tempfile::tempdir().expect("tempdir");
    let socket = tmp.path().join("b.sock");
    let (inbox, mut rx) = Inbox::channel();
    let attached = FeedAttached::new();
    let task = spawn_feed(socket.clone(), inbox, attached.clone());

    assert_eq!(next(&mut rx).await, EngineEvent::Feed(FeedStatus::Waiting));
    assert!(!attached.is_attached(), "nothing to attach to yet");

    let listener = UnixListener::bind(&socket).expect("bind");
    let (mut server, _) = listener.accept().await.expect("accept");
    assert_eq!(
        next(&mut rx).await,
        EngineEvent::Feed(FeedStatus::Connected { reconnect: false })
    );
    assert!(attached.is_attached(), "the socket is held");

    server
        .write_all(b"not json\n{\"family\":\"roadmap\",\"type\":\"chunk_changed\",\"chunk\":{\"id\":\"a\"}}\n")
        .await
        .expect("write");
    match next(&mut rx).await {
        EngineEvent::Frame(frame) => {
            assert_eq!(frame.family, "roadmap");
            assert_eq!(frame.kind, "chunk_changed");
            assert_eq!(frame.payload["chunk"]["id"], "a");
        }
        other => panic!("expected a frame, got {other:?}"),
    }

    drop(server);
    assert_eq!(
        next(&mut rx).await,
        EngineEvent::Feed(FeedStatus::Disconnected)
    );
    assert!(!attached.is_attached(), "and let go on the hangup");

    let (_server, _) = listener.accept().await.expect("accept again");
    assert_eq!(
        next(&mut rx).await,
        EngineEvent::Feed(FeedStatus::Connected { reconnect: true })
    );
    task.abort();
}

#[cfg(not(unix))]
#[tokio::test]
async fn without_unix_sockets_the_feed_waits_and_never_attaches() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let (inbox, mut rx) = Inbox::channel();
    let attached = FeedAttached::new();
    let task = spawn_feed(tmp.path().join("b.sock"), inbox, attached.clone());

    assert_eq!(next(&mut rx).await, EngineEvent::Feed(FeedStatus::Waiting));
    tokio::time::sleep(Duration::from_millis(50)).await;
    assert!(!attached.is_attached(), "there is no socket to attach to");
    assert!(rx.try_recv().is_err(), "and nothing more is posted");
    task.abort();
}

#[tokio::test]
async fn ends_once_the_loop_is_gone() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let (inbox, rx) = Inbox::channel();
    let task = spawn_feed(tmp.path().join("never.sock"), inbox, FeedAttached::new());
    tokio::time::sleep(Duration::from_millis(20)).await;
    drop(rx);
    tokio::time::timeout(Duration::from_secs(2), task)
        .await
        .expect("the feed ended")
        .expect("without a panic");
}
