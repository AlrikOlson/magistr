//! Durability of the ship/roadmap/signal stores under concurrent server
//! processes — the family-stores extension of `think_trace_durability.rs`.
//!
//! Residue of the 2026-06-09 think incident: these three engines still did
//! load-once-at-startup + whole-file-overwrite of their per-project
//! `<project_id>.json`, so two live engines in the SAME project could clobber
//! each other's acked mutations. The `*_clobber*` tests reproduce that loss
//! mechanism and MUST fail on the pre-fix unlocked overwrite persistence.

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

fn roadmap_engine(tmp: &TempDir, project: &str) -> RoadmapEngine {
    RoadmapEngine::new(project.into())
        .with_persistence(Persistence::new(&infra_cfg(tmp), Domain::Roadmap))
}

fn signal_engine(tmp: &TempDir, project: &str) -> SignalEngine {
    SignalEngine::new(project.into())
        .with_persistence(Persistence::new(&infra_cfg(tmp), Domain::Signal))
}

fn ship_engine(tmp: &TempDir, project: &str) -> ShipEngine {
    let cfg = ship_persistence::PersistenceConfig {
        enabled: true,
        data_dir: tmp.path().to_path_buf(),
    };
    ShipEngine::new(project.into()).with_persistence(ship_persistence::Persistence::new(&cfg))
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

// ── roadmap ────────────────────────────────────────────────────────────────

/// THE INCIDENT SHAPE, on the roadmap store: two live engines share one data
/// dir and one project. B adds a chunk (acked + persisted). A — which loaded
/// before B's write and knows nothing of it — then adds its own chunk. A's
/// save must not erase B's acked chunk from disk.
#[test]
fn concurrent_roadmap_writer_does_not_clobber_acked_chunks() {
    let tmp = TempDir::new().unwrap();
    let mut a = roadmap_engine(&tmp, "proj");
    let mut b = roadmap_engine(&tmp, "proj");

    add_chunk(&mut b, "from-b");
    add_chunk(&mut a, "from-a");

    let reloaded = roadmap_engine(&tmp, "proj");
    let ids: Vec<&str> = reloaded
        .roadmap()
        .chunks
        .iter()
        .map(|c| c.id.as_str())
        .collect();
    assert!(
        ids.contains(&"from-b"),
        "chunk from-b was acked to B but a stale concurrent writer erased it; survived: {ids:?}"
    );
    assert!(
        ids.contains(&"from-a"),
        "chunk from-a lost; survived: {ids:?}"
    );
}

/// Per-chunk conflicts resolve by recency: a stale writer that still holds an
/// old copy of chunk X must not roll back the status another process advanced.
#[test]
fn stale_roadmap_writer_does_not_roll_back_chunk_status() {
    let tmp = TempDir::new().unwrap();
    {
        let mut seed = roadmap_engine(&tmp, "proj");
        add_chunk(&mut seed, "x");
    }

    let mut a = roadmap_engine(&tmp, "proj"); // both load x@Pending
    let mut b = roadmap_engine(&tmp, "proj");

    b.set_status("x", ChunkStatus::InProgress).unwrap(); // B advances x
    add_chunk(&mut a, "y"); // A persists with a STALE x@Pending

    let reloaded = roadmap_engine(&tmp, "proj");
    let x = reloaded
        .roadmap()
        .chunks
        .iter()
        .find(|c| c.id == "x")
        .expect("x must survive");
    assert_eq!(
        x.status,
        ChunkStatus::InProgress,
        "stale writer rolled back x's acked status transition"
    );
    assert!(reloaded.roadmap().chunks.iter().any(|c| c.id == "y"));
}

// ── signal ─────────────────────────────────────────────────────────────────

/// Same mechanism on the signal store: a signal captured (and acked) by B must
/// survive a save from stale A.
#[test]
fn concurrent_signal_writer_does_not_clobber_acked_signals() {
    let tmp = TempDir::new().unwrap();
    let mut a = signal_engine(&tmp, "proj");
    let mut b = signal_engine(&tmp, "proj");

    let b_id = b
        .capture(SignalKind::Bug, "b@example.com".into(), "from b".into())
        .as_ref()
        .unwrap()
        .value
        .id
        .clone();
    let a_id = a
        .capture(SignalKind::Idea, "a@example.com".into(), "from a".into())
        .as_ref()
        .unwrap()
        .value
        .id
        .clone();

    let reloaded = signal_engine(&tmp, "proj");
    let ids: Vec<&str> = reloaded
        .signals()
        .signals
        .iter()
        .map(|s| s.id.as_str())
        .collect();
    assert!(
        ids.contains(&b_id.as_str()),
        "signal from B was acked but a stale concurrent writer erased it; survived: {ids:?}"
    );
    assert!(ids.contains(&a_id.as_str()), "signal from A lost: {ids:?}");
}

/// The signal lifecycle is forward-only, so a stale writer must never roll a
/// signal's status backwards on disk.
#[test]
fn stale_signal_writer_does_not_roll_back_lifecycle_progress() {
    let tmp = TempDir::new().unwrap();
    let seeded_id = {
        let mut seed = signal_engine(&tmp, "proj");
        seed.capture(SignalKind::Question, "q@example.com".into(), "seed".into())
            .as_ref()
            .unwrap()
            .value
            .id
            .clone()
    };

    let mut a = signal_engine(&tmp, "proj"); // both load seeded@New
    let mut b = signal_engine(&tmp, "proj");

    b.set_status(&seeded_id, SignalStatus::Triaged).unwrap(); // B advances
    a.capture(SignalKind::Feedback, "a@example.com".into(), "new".into())
        .unwrap(); // A persists stale copy

    let reloaded = signal_engine(&tmp, "proj");
    let seeded = reloaded
        .signals()
        .signals
        .iter()
        .find(|s| s.id == seeded_id)
        .expect("seeded signal must survive");
    assert_eq!(
        seeded.status,
        SignalStatus::Triaged,
        "stale writer rolled back an acked lifecycle transition"
    );
    assert_eq!(reloaded.signals().signals.len(), 2);
}

// ── ship ───────────────────────────────────────────────────────────────────

/// Two engines working the SAME cycle (B loaded the objective A persisted):
/// a task acked to either process must survive the other's saves.
#[test]
fn concurrent_ship_writers_union_tasks_within_one_cycle() {
    let tmp = TempDir::new().unwrap();
    let mut a = ship_engine(&tmp, "proj");
    a.set_objective("Shared cycle".into(), vec![], vec![], String::new())
        .unwrap();

    // B loads the persisted objective — same cycle as A.
    let mut b = ship_engine(&tmp, "proj");
    assert!(b.objective.is_some(), "B must join A's cycle");

    a.add_task(
        "a-task".into(),
        "A's task".into(),
        TaskType::Implement,
        None,
        None,
    )
    .unwrap();
    // B never saw a-task; its save must not erase it.
    b.add_task(
        "b-task".into(),
        "B's task".into(),
        TaskType::Test,
        None,
        None,
    )
    .unwrap();

    let reloaded = ship_engine(&tmp, "proj");
    let ids: Vec<&str> = reloaded.tasks.iter().map(|t| t.id.as_str()).collect();
    assert!(
        ids.contains(&"a-task"),
        "task acked to A erased by stale writer B; survived: {ids:?}"
    );
    assert!(ids.contains(&"b-task"), "task from B lost: {ids:?}");
}

/// A stale process still holding a PRIOR cycle must not clobber the new cycle
/// another process started after a reset.
#[test]
fn stale_ship_cycle_does_not_clobber_a_newer_cycle() {
    let tmp = TempDir::new().unwrap();
    let mut a = ship_engine(&tmp, "proj");
    a.set_objective("Old cycle".into(), vec![], vec![], String::new())
        .unwrap();

    let mut b = ship_engine(&tmp, "proj"); // B holds the old cycle
    assert_eq!(b.objective.as_ref().unwrap().description, "Old cycle");

    a.reset().unwrap();
    a.set_objective("New cycle".into(), vec![], vec![], String::new())
        .unwrap();

    // B (stale, old cycle) saves — it must not resurrect the old cycle.
    b.add_task(
        "zombie".into(),
        "stale".into(),
        TaskType::Implement,
        None,
        None,
    )
    .unwrap_err();

    let reloaded = ship_engine(&tmp, "proj");
    assert_eq!(
        reloaded.objective.as_ref().map(|o| o.description.as_str()),
        Some("New cycle"),
        "a stale writer resurrected a reset-away cycle"
    );
    assert!(
        reloaded.tasks.iter().all(|t| t.id != "zombie"),
        "a stale cycle's task leaked into the new cycle"
    );
}

/// Reset stays a reset for the resetting process itself: its own next cycle
/// must not re-merge the pre-reset tasks. (Guards the merge-on-save change —
/// this already passes pre-fix and must keep passing.)
#[test]
fn ship_reset_is_not_resurrected_by_later_saves() {
    let tmp = TempDir::new().unwrap();
    let mut a = ship_engine(&tmp, "proj");
    a.set_objective("Cycle 1".into(), vec![], vec![], String::new())
        .unwrap();
    a.add_task(
        "old".into(),
        "pre-reset".into(),
        TaskType::Implement,
        None,
        None,
    )
    .unwrap();

    a.reset().unwrap();
    a.set_objective("Cycle 2".into(), vec![], vec![], String::new())
        .unwrap();
    a.add_task(
        "new".into(),
        "post-reset".into(),
        TaskType::Implement,
        None,
        None,
    )
    .unwrap();

    let reloaded = ship_engine(&tmp, "proj");
    assert_eq!(
        reloaded.objective.as_ref().map(|o| o.description.as_str()),
        Some("Cycle 2")
    );
    let ids: Vec<&str> = reloaded.tasks.iter().map(|t| t.id.as_str()).collect();
    assert_eq!(ids, vec!["new"], "reset-away task resurrected: {ids:?}");
}

#[test]
fn rejected_roadmap_and_signal_writes_preserve_bytes_and_retry_after_repair() {
    use magistr_workflow::roadmap::engine::RoadmapMutationError;
    use magistr_workflow::signal::engine::SignalMutationError;
    use magistr_workflow_contracts::{ReadErrorKind, ReadSource, WriteErrorKind};
    let tmp = TempDir::new().unwrap();
    let mut roadmap = roadmap_engine(&tmp, "proj");
    let mut signal = signal_engine(&tmp, "proj");
    add_chunk(&mut roadmap, "saved");
    let captured = signal
        .capture(SignalKind::Idea, "author".into(), "saved".into())
        .unwrap();
    let roadmap_path = Persistence::new(&infra_cfg(&tmp), Domain::Roadmap).path_for("proj");
    let signal_path = Persistence::new(&infra_cfg(&tmp), Domain::Signal).path_for("proj");
    let roadmap_bytes = std::fs::read(&roadmap_path).unwrap();
    let signal_bytes = std::fs::read(&signal_path).unwrap();
    for (bad, kind) in [
        (b"{".to_vec(), ReadErrorKind::Decode),
        (b"{}".to_vec(), ReadErrorKind::Decode),
    ] {
        std::fs::write(&roadmap_path, &bad).unwrap();
        std::fs::write(&signal_path, &bad).unwrap();
        let RoadmapMutationError::Store(error) =
            roadmap.link_chunk("saved", "think:1").unwrap_err()
        else {
            panic!("expected store refusal")
        };
        assert_eq!(error.kind, WriteErrorKind::Read(kind));
        assert_eq!(error.effect(), "none");
        let SignalMutationError::Store(error) =
            signal.link(&captured.value.id, "think:1").unwrap_err()
        else {
            panic!("expected store refusal")
        };
        assert_eq!(error.kind, WriteErrorKind::Read(kind));
        assert_eq!(error.effect(), "none");
        assert_eq!(std::fs::read(&roadmap_path).unwrap(), bad);
        assert_eq!(std::fs::read(&signal_path).unwrap(), bad);
        assert!(roadmap.roadmap().chunks[0].cross_refs.is_empty());
        assert!(signal.signals().signals[0].cross_refs.is_empty());
    }
    std::fs::write(&roadmap_path, roadmap_bytes).unwrap();
    std::fs::write(&signal_path, signal_bytes).unwrap();
    let saved = roadmap.link_chunk("saved", "think:1").unwrap();
    assert!(matches!(saved.source, ReadSource::File(_)));
    assert_eq!(saved.source, roadmap.observe().unwrap().source);
    let saved = signal.link(&captured.value.id, "think:1").unwrap();
    assert_eq!(saved.source, signal.observe().unwrap().source);
    assert_eq!(saved.value.cross_refs, ["think:1"]);
}

#[test]
fn simultaneous_owner_transactions_retain_every_acknowledged_record() {
    let tmp = TempDir::new().unwrap();
    let barrier = std::sync::Barrier::new(8);
    std::thread::scope(|scope| {
        for i in 0..8 {
            let tmp = &tmp;
            let barrier = &barrier;
            scope.spawn(move || {
                let mut roadmap = roadmap_engine(tmp, "proj");
                let mut signals = signal_engine(tmp, "proj");
                barrier.wait();
                add_chunk(&mut roadmap, &format!("work-{i}"));
                signals
                    .capture(SignalKind::Idea, "author".into(), format!("idea {i}"))
                    .unwrap();
            });
        }
    });
    assert_eq!(
        roadmap_engine(&tmp, "proj")
            .observe()
            .unwrap()
            .value
            .chunks
            .len(),
        8
    );
    assert_eq!(
        signal_engine(&tmp, "proj")
            .observe()
            .unwrap()
            .value
            .signals
            .len(),
        8
    );
}

#[tokio::test]
async fn signal_mcp_refuses_failed_saves_and_acknowledges_the_repaired_owner() {
    use magistr_workflow::signal::{SignalService, mcp::handlers::CaptureArgs};
    use rmcp::handler::server::wrapper::Parameters;
    let tmp = TempDir::new().unwrap();
    let signal = signal_engine(&tmp, "proj");
    let path = Persistence::new(&infra_cfg(&tmp), Domain::Signal).path_for("proj");
    std::fs::write(&path, "{").unwrap();
    let service = SignalService::new(signal);
    let args = || {
        Parameters(CaptureArgs {
            kind: "idea".into(),
            from: "author".into(),
            body: "Keep my work".into(),
            content: None,
        })
    };
    let refused = service.signal_capture(args()).await.unwrap();
    assert_eq!(refused.is_error, Some(true));
    let body = refused.structured_content.unwrap();
    assert_eq!(body["effect"], "none");
    assert_eq!(body["error_kind"], "store_unavailable");
    assert_eq!(std::fs::read(&path).unwrap(), b"{");
    std::fs::remove_file(&path).unwrap();
    let saved = service.signal_capture(args()).await.unwrap();
    assert_ne!(saved.is_error, Some(true));
    let body = saved.structured_content.unwrap();
    assert_eq!(body["source"]["kind"], "file");
    assert_eq!(body["value"]["body"], "Keep my work");
    assert_eq!(
        service
            .engine()
            .lock()
            .unwrap()
            .observe()
            .unwrap()
            .value
            .signals
            .len(),
        1
    );
}
