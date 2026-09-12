use std::time::Duration;

#[cfg(unix)]
use magistr_engine::{Delivery, NativeEvent, RoadmapFrame};
use tokio::sync::mpsc::UnboundedReceiver;

use super::*;
#[cfg(unix)]
use crate::harness::fixtures;

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

/// The socket line a publisher at `epoch` writes for a changed chunk as
/// its `seq`th frame — the bytes `Broadcaster::emit` produces.
#[cfg(unix)]
fn chunk_line(id: &str, epoch: u64, seq: u64) -> Vec<u8> {
    let mut payload = serde_json::to_value(RoadmapFrame::ChunkChanged {
        chunk: fixtures::chunk(id),
    })
    .expect("a frame serializes");
    let object = payload.as_object_mut().expect("an object");
    object.insert("family".to_owned(), serde_json::json!("roadmap"));
    object.insert("seq".to_owned(), serde_json::json!(seq));
    object.insert("epoch".to_owned(), serde_json::json!(epoch));
    let mut line = serde_json::to_string(&payload).expect("json");
    line.push('\n');
    line.into_bytes()
}

/// Whether a snapshot request is pending on `link` right now.
#[cfg(unix)]
async fn requested(link: &FeedLink) -> bool {
    tokio::time::timeout(Duration::from_millis(200), link.snapshot_requested())
        .await
        .is_ok()
}

#[cfg(unix)]
#[tokio::test]
async fn waits_for_the_socket_decodes_lines_and_reattaches_after_a_hangup() {
    use tokio::io::AsyncWriteExt;
    use tokio::net::UnixListener;

    let tmp = tempfile::tempdir().expect("tempdir");
    let socket = tmp.path().join("b.sock");
    let (inbox, mut rx) = Inbox::channel();
    let link = FeedLink::new();
    let task = spawn_feed(socket.clone(), inbox, link.clone());

    assert_eq!(next(&mut rx).await, EngineEvent::Feed(FeedStatus::Waiting));
    assert!(!link.is_attached(), "nothing to attach to yet");
    assert!(!requested(&link).await, "and nothing to re-read for");

    let listener = UnixListener::bind(&socket).expect("bind");
    let (mut server, _) = listener.accept().await.expect("accept");
    assert_eq!(
        next(&mut rx).await,
        EngineEvent::Feed(FeedStatus::Connected { reconnect: false })
    );
    assert!(link.is_attached(), "the socket is held");
    assert!(
        requested(&link).await,
        "attaching asks for a snapshot: frames before the registration are gone"
    );

    let mut bytes = b"not json\n".to_vec();
    bytes.extend(chunk_line("a", 1, 7));
    server.write_all(&bytes).await.expect("write");
    assert_eq!(
        next(&mut rx).await,
        EngineEvent::Event {
            event: NativeEvent::Roadmap(RoadmapFrame::ChunkChanged {
                chunk: fixtures::chunk("a"),
            }),
            delivery: Delivery::First,
        },
        "the junk line is dropped; the frame is decoded at the edge"
    );

    drop(server);
    assert_eq!(
        next(&mut rx).await,
        EngineEvent::Feed(FeedStatus::Disconnected)
    );
    assert!(!link.is_attached(), "and let go on the hangup");

    let (mut server, _) = listener.accept().await.expect("accept again");
    assert_eq!(
        next(&mut rx).await,
        EngineEvent::Feed(FeedStatus::Connected { reconnect: true })
    );
    assert!(requested(&link).await, "reattaching asks again");

    // The cursor survived the hangup: the next frame is in order only
    // if nothing was missed meanwhile — here two frames were.
    server
        .write_all(&chunk_line("b", 1, 10))
        .await
        .expect("write");
    assert_eq!(
        next(&mut rx).await,
        EngineEvent::Event {
            event: NativeEvent::Roadmap(RoadmapFrame::ChunkChanged {
                chunk: fixtures::chunk("b"),
            }),
            delivery: Delivery::Gap { missed: 2 },
        }
    );
    assert!(requested(&link).await, "a gap asks for a snapshot");
    task.abort();
}

#[cfg(unix)]
#[tokio::test]
async fn deliveries_follow_the_publishers_sequence_and_a_new_owner_is_noticed() {
    use tokio::io::AsyncWriteExt;
    use tokio::net::UnixListener;

    let tmp = tempfile::tempdir().expect("tempdir");
    let socket = tmp.path().join("b.sock");
    let (inbox, mut rx) = Inbox::channel();
    let link = FeedLink::new();
    let task = spawn_feed(socket.clone(), inbox, link.clone());
    assert_eq!(next(&mut rx).await, EngineEvent::Feed(FeedStatus::Waiting));

    let listener = UnixListener::bind(&socket).expect("bind");
    let (mut server, _) = listener.accept().await.expect("accept");
    assert_eq!(
        next(&mut rx).await,
        EngineEvent::Feed(FeedStatus::Connected { reconnect: false })
    );
    assert!(requested(&link).await, "the attachment's own request");

    let mut bytes = Vec::new();
    bytes.extend(chunk_line("a", 3, 1));
    bytes.extend(chunk_line("b", 3, 2));
    bytes.extend(chunk_line("b", 3, 2)); // the same frame again
    bytes.extend(chunk_line("c", 4, 1)); // a new owner's first frame
    bytes.extend(chunk_line("d", 4, 2));
    server.write_all(&bytes).await.expect("write");

    let mut deliveries = Vec::new();
    for _ in 0..5 {
        match next(&mut rx).await {
            EngineEvent::Event { delivery, .. } => deliveries.push(delivery),
            other => panic!("expected an event, got {other:?}"),
        }
    }
    assert_eq!(
        deliveries,
        vec![
            Delivery::First,
            Delivery::InOrder,
            Delivery::Duplicate,
            Delivery::NewEpoch,
            Delivery::InOrder,
        ]
    );
    assert!(requested(&link).await, "the new owner asked for a snapshot");
    assert!(!requested(&link).await, "and nothing else did");
    task.abort();
}

#[cfg(not(unix))]
#[tokio::test]
async fn without_unix_sockets_the_feed_waits_and_never_attaches() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let (inbox, mut rx) = Inbox::channel();
    let link = FeedLink::new();
    let task = spawn_feed(tmp.path().join("b.sock"), inbox, link.clone());

    assert_eq!(next(&mut rx).await, EngineEvent::Feed(FeedStatus::Waiting));
    tokio::time::sleep(Duration::from_millis(50)).await;
    assert!(!link.is_attached(), "there is no socket to attach to");
    assert!(rx.try_recv().is_err(), "and nothing more is posted");
    task.abort();
}

#[tokio::test]
async fn ends_once_the_loop_is_gone() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let (inbox, rx) = Inbox::channel();
    let task = spawn_feed(tmp.path().join("never.sock"), inbox, FeedLink::new());
    tokio::time::sleep(Duration::from_millis(20)).await;
    drop(rx);
    tokio::time::timeout(Duration::from_secs(2), task)
        .await
        .expect("the feed ended")
        .expect("without a panic");
}
