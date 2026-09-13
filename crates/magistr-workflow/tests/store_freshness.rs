//! Each persisted family remembers the stamp of the store file it last
//! read or wrote and re-reads only when the stamp moved, so two engines on
//! one store answer one state without a reload and an unchanged store
//! costs a `stat`, never a parse. A write that raced inside the window
//! between one engine's read and its save resolves by the family's merge
//! rule on the record, and the loser observes the winner on its next
//! refresh — the declared outcome, not a refusal and not silent loss.

use std::thread::sleep;
use std::time::Duration;

use tempfile::TempDir;

use magistr_workflow::infra::{Domain, Persistence, PersistenceConfig};
use magistr_workflow::roadmap::RoadmapEngine;
use magistr_workflow::roadmap::domain::ChunkStatus;
use magistr_workflow::ship::domain::task::TaskType;
use magistr_workflow::ship::engine::ShipEngine;
use magistr_workflow::ship::persistence as ship_persistence;
use magistr_workflow::signal::SignalEngine;
use magistr_workflow::signal::domain::{SignalKind, SignalStatus};

fn infra_cfg(tmp: &TempDir) -> PersistenceConfig {
    PersistenceConfig::from_env()
        .with_data_dir(tmp.path().to_path_buf())
        .enabled(true)
}

fn roadmap_engine(tmp: &TempDir) -> RoadmapEngine {
    RoadmapEngine::new("proj".into())
        .with_persistence(Persistence::new(&infra_cfg(tmp), Domain::Roadmap))
}

fn signal_engine(tmp: &TempDir) -> SignalEngine {
    SignalEngine::new("proj".into())
        .with_persistence(Persistence::new(&infra_cfg(tmp), Domain::Signal))
}

fn ship_engine(tmp: &TempDir) -> ShipEngine {
    let cfg = ship_persistence::PersistenceConfig {
        enabled: true,
        data_dir: tmp.path().to_path_buf(),
    };
    ShipEngine::new("proj".into()).with_persistence(ship_persistence::Persistence::new(&cfg))
}

fn add_chunk(e: &mut RoadmapEngine, id: &str) {
    e.add_chunk(
        id.into(),
        format!("Chunk {id}"),
        ChunkStatus::Pending,
        10,
        String::new(),
        Vec::new(),
        Vec::new(),
        false,
    )
    .unwrap();
}

fn chunk_ids(e: &RoadmapEngine) -> Vec<String> {
    let mut ids: Vec<String> = e.roadmap().chunks.iter().map(|c| c.id.clone()).collect();
    ids.sort();
    ids
}

#[test]
fn a_roadmap_engine_re_reads_only_when_the_store_stamp_moved() {
    let tmp = TempDir::new().unwrap();
    let mut a = roadmap_engine(&tmp);
    let mut b = roadmap_engine(&tmp);
    assert_eq!(a.refresh_if_stale(), Ok(false), "nothing written yet");

    add_chunk(&mut b, "from-b");
    assert_eq!(a.refresh_if_stale(), Ok(true), "the stamp moved");
    assert_eq!(chunk_ids(&a), vec!["from-b".to_owned()]);
    assert_eq!(a.refresh_if_stale(), Ok(false), "unchanged: no re-read");

    // A's own save records the stamp of the file it wrote, so its next
    // refresh is a stat; B, which never saw it, reads once.
    add_chunk(&mut a, "from-a");
    assert_eq!(a.refresh_if_stale(), Ok(false));
    assert_eq!(b.refresh_if_stale(), Ok(true));
    assert_eq!(
        chunk_ids(&b),
        vec!["from-a".to_owned(), "from-b".to_owned()]
    );
    assert_eq!(b.refresh_if_stale(), Ok(false));
}

#[test]
fn a_writers_memory_is_the_merged_file_after_its_own_save() {
    let tmp = TempDir::new().unwrap();
    let mut a = roadmap_engine(&tmp);
    let mut b = roadmap_engine(&tmp);
    add_chunk(&mut b, "from-b");
    // A writes from memory that predates B's chunk: the locked merge keeps
    // B's chunk on disk, and A adopts what landed rather than its stale copy.
    add_chunk(&mut a, "from-a");
    assert_eq!(
        chunk_ids(&a),
        vec!["from-a".to_owned(), "from-b".to_owned()]
    );
    assert_eq!(a.refresh_if_stale(), Ok(false));
}

#[test]
fn a_conflicting_roadmap_transition_resolves_by_the_newest_record_and_the_loser_observes_it() {
    let tmp = TempDir::new().unwrap();
    let mut a = roadmap_engine(&tmp);
    add_chunk(&mut a, "c");
    let mut b = roadmap_engine(&tmp);
    assert_eq!(
        b.refresh_if_stale(),
        Ok(false),
        "opened on the current file"
    );

    // Both engines hold the current record. A starts the chunk; B, still
    // holding the pre-start copy, retitles it a moment later without
    // refreshing — the window between a read and a save.
    a.start_chunk("c").unwrap();
    sleep(Duration::from_millis(5));
    b.update_chunk("c", Some("B's title".into()), None, None, None, None, None)
        .unwrap();

    // Declared outcome: the copy with the newest `updated_at` wins whole.
    // B's write is newer, so B's record — pending, retitled — is the store,
    // and A's start is superseded rather than silently kept in A's memory.
    let on_disk = roadmap_engine(&tmp);
    let chunk = &on_disk.roadmap().chunks[0];
    assert_eq!(chunk.title, "B's title");
    assert_eq!(chunk.status, ChunkStatus::Pending);
    assert_eq!(a.refresh_if_stale(), Ok(true));
    let seen = &a.roadmap().chunks[0];
    assert_eq!(
        (seen.title.as_str(), seen.status),
        ("B's title", ChunkStatus::Pending)
    );
    // A's next transition validates against the record it now holds.
    a.start_chunk("c").expect("pending again, so it starts");
    assert_eq!(b.refresh_if_stale(), Ok(true));
    assert_eq!(b.roadmap().chunks[0].status, ChunkStatus::InProgress);
}

#[test]
fn a_conflicting_signal_write_resolves_by_lifecycle_rank_not_by_who_wrote_last() {
    let tmp = TempDir::new().unwrap();
    let mut a = signal_engine(&tmp);
    let id = a
        .capture(SignalKind::Idea, "alice".into(), "rank".into())
        .id
        .clone();
    let mut b = signal_engine(&tmp);
    assert_eq!(b.refresh_if_stale(), Ok(false));

    // A researches; B, from its pre-research copy, snoozes — a write that
    // carries the older lifecycle state.
    a.research(&id, "worth it".into(), 0.9, vec![], None)
        .unwrap();
    b.snooze(&id, 5).unwrap();

    // Declared outcome: the further lifecycle rank wins the record, so the
    // research survives B's later write, and B holds the merged record.
    let seen = b.get(&id).unwrap();
    assert_eq!(seen.status, SignalStatus::Researched);
    assert_eq!(seen.enrichment.len(), 1);
    assert_eq!(b.refresh_if_stale(), Ok(false));
    assert_eq!(a.refresh_if_stale(), Ok(true));
    assert_eq!(a.get(&id).unwrap().status, SignalStatus::Researched);
}

#[test]
fn a_ship_engine_re_reads_only_when_the_store_stamp_moved() {
    let tmp = TempDir::new().unwrap();
    let mut a = ship_engine(&tmp);
    let mut b = ship_engine(&tmp);
    assert_eq!(b.refresh_if_stale(), Ok(false));

    a.set_objective("cycle".into(), vec![], vec![], "chunk:c".into());
    a.add_task("t".into(), "task".into(), TaskType::Implement, None, None);
    assert_eq!(b.refresh_if_stale(), Ok(true));
    assert_eq!(b.tasks.len(), 1);
    assert_eq!(b.refresh_if_stale(), Ok(false));
    assert_eq!(a.refresh_if_stale(), Ok(false), "A wrote the file it holds");

    // B's write on the same cycle merges and B holds the merged state.
    b.start_task("t").unwrap();
    assert_eq!(b.refresh_if_stale(), Ok(false));
    assert_eq!(a.refresh_if_stale(), Ok(true));
    assert_eq!(a.active_task_id().as_deref(), Some("t"));
}

#[test]
fn engines_without_persistence_never_refresh() {
    let mut roadmap = RoadmapEngine::new("proj".into());
    let mut signal = SignalEngine::new("proj".into());
    let mut ship = ShipEngine::new("proj".into());
    assert_eq!(roadmap.refresh_if_stale(), Ok(false));
    assert_eq!(signal.refresh_if_stale(), Ok(false));
    assert_eq!(ship.refresh_if_stale(), Ok(false));
}
