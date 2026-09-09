use std::time::Duration;

use magistr_engine::{ChunkStatus, SignalKind, TasConfig};
use tokio::sync::mpsc::UnboundedReceiver;

use super::*;

/// The interval the tests run the re-seed at — short enough to be quick,
/// long enough that a slow machine does not race the first tick.
const TICK: Duration = Duration::from_millis(40);

fn config(tmp: &tempfile::TempDir) -> TasConfig {
    std::fs::create_dir_all(tmp.path().join("proj")).expect("project root");
    TasConfig::from_lookup(tmp.path().join("proj"), |_| None).with_data_dir(tmp.path().join("data"))
}

fn engines(tmp: &tempfile::TempDir) -> NativeEngines {
    NativeEngines::open_for_project(&config(tmp), "reseeded".to_owned())
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
async fn next_seed(rx: &mut UnboundedReceiver<Msg>, within: Duration) -> Option<StoreSeed> {
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
    let task = spawn_reseed(engines.clone(), FeedAttached::new(), inbox, TICK);

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
    let ids: Vec<&str> = seed
        .chunks
        .iter()
        .filter_map(|chunk| chunk["id"].as_str())
        .collect();
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
    let attached = FeedAttached::new();
    attached.set(true);
    let task = spawn_reseed(engines.clone(), attached.clone(), inbox, TICK);
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
    let task = spawn_reseed(engines(&tmp), FeedAttached::new(), inbox, TICK);
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
    let task = spawn_reseed(engines.clone(), FeedAttached::new(), inbox, TICK);
    // Let the task take its opening stamp before the store moves under it.
    tokio::time::sleep(TICK).await;

    // A second host on the same store — what `magistr mcp-serve` is. Its
    // capture is invisible to this cockpit's in-memory signal engine, so
    // only a reload can put it in the seed.
    let server = NativeEngines::open_for_project(&config(&tmp), "reseeded".to_owned());
    let captured = server.signal_capture(SignalKind::Idea, "alice".to_owned(), "faster".to_owned());

    let seed = next_seed(&mut rx, Duration::from_secs(5))
        .await
        .expect("the capture reached the inbox");
    let ids: Vec<&str> = seed
        .signals
        .iter()
        .filter_map(|signal| signal["id"].as_str())
        .collect();
    assert_eq!(ids, vec![captured.id.as_str()]);
    assert_eq!(seed.signals_pending, 1);
    task.abort();
}
