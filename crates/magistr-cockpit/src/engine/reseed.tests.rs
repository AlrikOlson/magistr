use std::time::Duration;

use magistr_engine::native::SignalStatus;
use magistr_engine::{
    ChunkStatus, NativeConfig, NativeEngines, NativeSnapshot, ProjectAuthority, SignalKind,
    WriterRole,
};
use tokio::sync::mpsc::UnboundedReceiver;

use super::*;

/// The interval the tests run the re-seed at — short enough to be quick,
/// long enough that a slow machine does not race the first tick.
const TICK: Duration = Duration::from_millis(40);

fn config(tmp: &tempfile::TempDir) -> NativeConfig {
    std::fs::create_dir_all(tmp.path().join("proj")).expect("project root");
    NativeConfig::from_lookup(tmp.path().join("proj"), |_| None)
        .with_data_dir(tmp.path().join("data"))
}

fn engines(tmp: &tempfile::TempDir) -> NativeEngines {
    ProjectAuthority::admit_for_project(&config(tmp), "reseeded".to_owned(), WriterRole::Host)
        .expect("host admitted")
        .open()
}

fn add_chunk(engines: &NativeEngines, id: &str) {
    engines
        .roadmap_add_chunk(
            id.to_owned(),
            format!("Chunk {id}"),
            id.to_owned(),
            ChunkStatus::Pending,
            100,
            String::new(),
            vec![],
            vec![],
            None,
            None,
        )
        .expect("add chunk");
}

/// The next seed, or `None` when nothing arrived within `within`.
async fn next_seed(rx: &mut UnboundedReceiver<Msg>, within: Duration) -> Option<NativeSnapshot> {
    match tokio::time::timeout(within, rx.recv()).await {
        Ok(Some(Msg::Engine(EngineEvent::Seed(seed)))) => Some(seed),
        Ok(other) => panic!("not a seed: {other:?}"),
        Err(_) => None,
    }
}

#[tokio::test]
async fn a_write_by_another_process_reaches_the_inbox_as_a_seed() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let engines = engines(&tmp);
    add_chunk(&engines, "base");

    let (inbox, mut rx) = Inbox::channel();
    let task = spawn_reseed(engines.reader(), FeedLink::new(), inbox, TICK);

    // Nothing changed: nothing is posted, and the cockpit stays idle.
    assert!(
        next_seed(&mut rx, TICK * 4).await.is_none(),
        "an unchanged store is not re-read"
    );

    // A second host on the same store — what `magistr mcp-serve` is.
    add_chunk(&engines, "next");
    let seed = next_seed(&mut rx, Duration::from_secs(5))
        .await
        .expect("the write reached the inbox");
    let ids: Vec<&str> = seed.chunks.iter().map(|chunk| chunk.id.as_str()).collect();
    assert_eq!(ids, vec!["base", "next"]);

    // And having caught up, it goes quiet again.
    assert!(
        next_seed(&mut rx, TICK * 4).await.is_none(),
        "one write, one seed"
    );
    task.abort();
}

#[tokio::test]
async fn an_attached_feed_stands_the_re_seed_down() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let engines = engines(&tmp);
    add_chunk(&engines, "base");

    let (inbox, mut rx) = Inbox::channel();
    let attached = FeedLink::new();
    attached.set(true);
    let task = spawn_reseed(engines.reader(), attached.clone(), inbox, TICK);
    // Let the task take its opening stamp before the store moves under it.
    tokio::time::sleep(TICK).await;

    add_chunk(&engines, "next");
    assert!(
        next_seed(&mut rx, TICK * 6).await.is_none(),
        "the frames are already arriving on the socket"
    );

    // The socket goes away and the safety net takes over.
    attached.set(false);
    assert!(
        next_seed(&mut rx, Duration::from_secs(5)).await.is_some(),
        "the change is picked up once the feed is gone"
    );
    task.abort();
}

#[tokio::test]
async fn it_ends_once_the_loop_is_gone() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let (inbox, rx) = Inbox::channel();
    let task = spawn_reseed(engines(&tmp).reader(), FeedLink::new(), inbox, TICK);
    tokio::time::sleep(TICK / 2).await;
    drop(rx);
    tokio::time::timeout(Duration::from_secs(2), task)
        .await
        .expect("the re-seed ended")
        .expect("without a panic");
}

#[tokio::test]
async fn a_signal_captured_by_another_process_reaches_the_seed() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let engines = engines(&tmp);
    add_chunk(&engines, "base");

    let (inbox, mut rx) = Inbox::channel();
    let task = spawn_reseed(engines.reader(), FeedLink::new(), inbox, TICK);
    // Let the task take its opening stamp before the store moves under it.
    tokio::time::sleep(TICK).await;

    // A second host on the same store — what `magistr mcp-serve` is. Its
    // capture is invisible to this cockpit's in-memory signal engine, so
    // only a reload can put it in the seed.
    let server = ProjectAuthority::admit_for_project(
        &config(&tmp),
        "reseeded".to_owned(),
        WriterRole::Service,
    )
    .expect("service admitted")
    .open();
    let captured = server
        .signal_capture(SignalKind::Idea, "alice".to_owned(), "faster".to_owned())
        .expect("capture");

    let seed = next_seed(&mut rx, Duration::from_secs(5))
        .await
        .expect("the capture reached the inbox");
    let ids: Vec<&str> = seed
        .signals
        .iter()
        .map(|signal| signal.id.as_str())
        .collect();
    assert_eq!(ids, vec![captured.id.as_str()]);
    assert_eq!(seed.signals[0].status, SignalStatus::New);
    task.abort();
}

/// The feed's request is the other way in: a snapshot is read at once,
/// attached or not, when the feed says frames may have been missed — and
/// only when the store moved, so a request over an unchanged store posts
/// nothing.
#[tokio::test]
async fn a_snapshot_request_from_the_feed_reads_the_store_even_while_attached() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let engines = engines(&tmp);
    add_chunk(&engines, "base");

    let (inbox, mut rx) = Inbox::channel();
    let link = FeedLink::new();
    link.set(true);
    // A long period: the timer never fires inside this test, so every
    // seed that arrives was the request's.
    let task = spawn_reseed(
        engines.reader(),
        link.clone(),
        inbox,
        Duration::from_secs(60),
    );
    tokio::time::sleep(TICK).await;

    link.request_snapshot();
    assert!(
        next_seed(&mut rx, TICK * 6).await.is_none(),
        "the store did not move: nothing to re-read"
    );

    add_chunk(&engines, "missed");
    link.request_snapshot();
    let seed = next_seed(&mut rx, Duration::from_secs(5))
        .await
        .expect("the request was served while attached");
    let ids: Vec<&str> = seed.chunks.iter().map(|chunk| chunk.id.as_str()).collect();
    assert_eq!(ids, vec!["base", "missed"]);
    assert_eq!(
        seed.cursor,
        engines.snapshot_cursor(),
        "the snapshot names where it was read"
    );
    task.abort();
}
