use std::time::Duration;

use magistr_cockpit::{cockpit::Msg, engine::EngineEvent};
use magistr_engine::{
    ChunkStatus, NativeConfig, NativeEngines, NativeSnapshot, ProjectAuthority, ReadState,
    SignalKind, WriterRole,
};
use magistr_workflow::infra::{Domain, Persistence};
use tokio::sync::mpsc::UnboundedReceiver;

use super::*;

const TICK: Duration = Duration::from_millis(40);

fn config(tmp: &tempfile::TempDir) -> NativeConfig {
    std::fs::create_dir_all(tmp.path().join("proj")).unwrap();
    NativeConfig::from_lookup(tmp.path().join("proj"), |_| None)
        .with_data_dir(tmp.path().join("data"))
}

fn engines(tmp: &tempfile::TempDir) -> NativeEngines {
    ProjectAuthority::admit_for_project(&config(tmp), "reseeded".into(), WriterRole::Host)
        .unwrap()
        .open()
}

fn add_chunk(engines: &NativeEngines, id: &str) {
    engines
        .roadmap_add_chunk(
            id.into(),
            format!("Chunk {id}"),
            id.into(),
            ChunkStatus::Pending,
            100,
            String::new(),
            vec![],
            vec![],
            None,
            None,
        )
        .unwrap();
}

async fn seeded(engines: &NativeEngines) -> SnapshotReads {
    let reads = SnapshotReads::new(engines.reader());
    reads.read(None, true).await;
    reads
}

async fn next_seed(rx: &mut UnboundedReceiver<Msg>, within: Duration) -> Option<NativeSnapshot> {
    match tokio::time::timeout(within, rx.recv()).await {
        Ok(Some(Msg::Engine(EngineEvent::Seed(seed)))) => Some(seed),
        Ok(other) => panic!("not a snapshot: {other:?}"),
        Err(_) => None,
    }
}

#[tokio::test]
async fn changes_are_published_and_validated_unchanged_timer_reads_stay_quiet() {
    let tmp = tempfile::tempdir().unwrap();
    let engines = engines(&tmp);
    add_chunk(&engines, "base");
    let reads = seeded(&engines).await;
    let (inbox, mut rx) = Inbox::channel();
    let task = spawn_reseed(reads, FeedLink::new(), inbox, TICK);

    assert!(next_seed(&mut rx, TICK * 4).await.is_none());
    add_chunk(&engines, "next");
    let seed = next_seed(&mut rx, Duration::from_secs(5)).await.unwrap();
    let ids: Vec<_> = seed
        .roadmap
        .current()
        .unwrap()
        .value
        .chunks
        .iter()
        .map(|chunk| chunk.id.as_str())
        .collect();
    assert_eq!(ids, ["base", "next"]);
    assert!(next_seed(&mut rx, TICK * 4).await.is_none());
    task.abort();
}

#[cfg(unix)]
#[tokio::test]
async fn timer_retries_failure_and_unchanged_stamp_repair_while_feed_stays_attached() {
    use magistr_engine::FeedStatus;
    use tokio::net::UnixListener;

    let tmp = tempfile::tempdir().unwrap();
    let engines = engines(&tmp);
    add_chunk(&engines, "retained");
    let reads = seeded(&engines).await;
    let link = FeedLink::new();
    let socket = tmp.path().join("events.sock");
    let listener = UnixListener::bind(&socket).unwrap();
    let (feed_inbox, mut events) = Inbox::channel();
    let feed = super::super::feed::spawn_feed(socket, "selected".into(), feed_inbox, link.clone());
    let (_connection, _) = listener.accept().await.unwrap();
    assert!(matches!(
        events.recv().await,
        Some(Msg::Engine(EngineEvent::Feed(FeedStatus::Waiting)))
    ));
    assert!(matches!(
        events.recv().await,
        Some(Msg::Engine(EngineEvent::Feed(FeedStatus::Connected { .. })))
    ));
    let (inbox, mut rx) = Inbox::channel();
    let task = spawn_reseed(reads, link, inbox, TICK);
    next_seed(&mut rx, Duration::from_secs(5)).await.unwrap();

    let path = Persistence::new(&engines.config().persistence_config(), Domain::Roadmap)
        .path_for("reseeded");
    let original = std::fs::read(&path).unwrap();
    std::fs::write(&path, vec![b'!'; original.len()]).unwrap();
    let failed_stamp = std::fs::metadata(&path).unwrap().modified().unwrap();
    let failed = next_seed(&mut rx, Duration::from_secs(5)).await.unwrap();
    assert!(matches!(failed.roadmap, ReadState::Stale { .. }));
    assert_eq!(
        failed.roadmap.last_successful().unwrap().value.chunks[0].id,
        "retained"
    );
    assert!(failed.ship.current().is_ok());
    assert!(next_seed(&mut rx, TICK * 3).await.is_none());

    std::fs::write(&path, original).unwrap();
    std::fs::File::options()
        .write(true)
        .open(&path)
        .unwrap()
        .set_times(std::fs::FileTimes::new().set_modified(failed_stamp))
        .unwrap();
    let recovered = next_seed(&mut rx, Duration::from_secs(5)).await.unwrap();
    assert_eq!(
        recovered.roadmap.current().unwrap().value.chunks[0].id,
        "retained"
    );
    assert!(
        events.try_recv().is_err(),
        "the feed stayed attached throughout"
    );
    task.abort();
    feed.abort();
}

#[tokio::test]
async fn scoped_ship_only_and_signal_changes_reach_the_snapshot() {
    let tmp = tempfile::tempdir().unwrap();
    let root = engines(&tmp);
    let engines = root
        .for_working_copy(root.config().project_root(), "selected")
        .unwrap();
    let reads = seeded(&engines).await;
    let (inbox, mut rx) = Inbox::channel();
    let task = spawn_reseed(reads, FeedLink::new(), inbox, TICK);
    engines
        .ship_set_objective(
            "Only this execution".into(),
            vec![],
            vec![],
            "chunk:work".into(),
        )
        .unwrap();
    let ship = next_seed(&mut rx, Duration::from_secs(5)).await.unwrap();
    assert_eq!(
        ship.ship
            .current()
            .unwrap()
            .value
            .objective
            .as_ref()
            .unwrap()
            .description,
        "Only this execution"
    );
    let captured = engines
        .signal_capture(SignalKind::Idea, "alice".into(), "faster".into())
        .unwrap();
    let seed = next_seed(&mut rx, Duration::from_secs(5)).await.unwrap();
    assert_eq!(seed.signals.current().unwrap().value[0].id, captured.id);
    task.abort();
}

#[tokio::test]
async fn requests_always_validate_and_publish_including_unchanged_records() {
    let tmp = tempfile::tempdir().unwrap();
    let engines = engines(&tmp);
    add_chunk(&engines, "base");
    let reads = seeded(&engines).await;
    let expected = engines.snapshot();
    let (inbox, mut rx) = Inbox::channel();
    let link = FeedLink::new();
    let task = spawn_reseed(reads, link.clone(), inbox, Duration::from_secs(60));
    link.request_snapshot();
    assert_eq!(
        next_seed(&mut rx, Duration::from_secs(5)).await.unwrap(),
        expected
    );
    link.request_snapshot();
    assert_eq!(
        next_seed(&mut rx, Duration::from_secs(5)).await.unwrap(),
        expected
    );
    task.abort();
}

#[tokio::test]
async fn a_write_between_initial_seed_and_worker_start_is_not_missed() {
    let tmp = tempfile::tempdir().unwrap();
    let engines = engines(&tmp);
    add_chunk(&engines, "base");
    let reads = seeded(&engines).await;
    add_chunk(&engines, "between");
    let (inbox, mut rx) = Inbox::channel();
    let task = spawn_reseed(reads, FeedLink::new(), inbox, TICK);
    let seed = next_seed(&mut rx, Duration::from_secs(5)).await.unwrap();
    assert_eq!(seed.roadmap.current().unwrap().value.chunks.len(), 2);
    task.abort();
}

#[tokio::test]
async fn it_ends_when_the_observation_destination_closes() {
    let tmp = tempfile::tempdir().unwrap();
    let engines = engines(&tmp);
    let (inbox, rx) = Inbox::channel();
    let task = spawn_reseed(seeded(&engines).await, FeedLink::new(), inbox, TICK);
    drop(rx);
    tokio::time::timeout(Duration::from_secs(2), task)
        .await
        .unwrap()
        .unwrap();
}
