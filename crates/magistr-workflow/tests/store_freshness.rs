//! Validated family observations distinguish durable data, absence and read failure.

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
fn a_roadmap_refresh_validates_the_store_and_reports_changes() {
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
fn roadmap_changes_apply_to_fresh_state_without_overwriting_other_fields() {
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

    // The title edit applies to the current committed chunk and retains its status.
    let on_disk = roadmap_engine(&tmp);
    let chunk = &on_disk.roadmap().chunks[0];
    assert_eq!(chunk.title, "B's title");
    assert_eq!(chunk.status, ChunkStatus::InProgress);
    assert_eq!(a.refresh_if_stale(), Ok(true));
    let seen = &a.roadmap().chunks[0];
    assert_eq!(
        (seen.title.as_str(), seen.status),
        ("B's title", ChunkStatus::InProgress)
    );
    // A's next transition validates against the record it now holds.
    assert!(
        a.start_chunk("c").is_err(),
        "the started chunk cannot start twice"
    );
    assert_eq!(
        b.refresh_if_stale(),
        Ok(false),
        "a refused write changes no file"
    );
    assert_eq!(b.roadmap().chunks[0].status, ChunkStatus::InProgress);
}

#[test]
fn a_conflicting_signal_write_resolves_by_lifecycle_rank_not_by_who_wrote_last() {
    let tmp = TempDir::new().unwrap();
    let mut a = signal_engine(&tmp);
    let id = a
        .capture(SignalKind::Idea, "alice".into(), "rank".into())
        .as_ref()
        .unwrap()
        .value
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
fn a_ship_refresh_validates_the_store_and_reports_changes() {
    let tmp = TempDir::new().unwrap();
    let mut a = ship_engine(&tmp);
    let mut b = ship_engine(&tmp);
    assert_eq!(b.refresh_if_stale(), Ok(false));

    a.set_objective("cycle".into(), vec![], vec![], "chunk:c".into())
        .unwrap();
    a.add_task("t".into(), "task".into(), TaskType::Implement, None, None)
        .unwrap();
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

#[test]
fn corrupt_cold_ship_reads_never_confirm_empty_and_recover_without_reopening() {
    use magistr_workflow_contracts::ReadErrorKind;
    let tmp = TempDir::new().unwrap();
    let mut writer = ship_engine(&tmp);
    writer
        .set_objective("kept".into(), vec![], vec![], "chunk:c".into())
        .unwrap();
    let persistence = ship_persistence::Persistence::new(&ship_persistence::PersistenceConfig {
        enabled: true,
        data_dir: tmp.path().to_owned(),
    });
    let path = persistence.path_for("proj");
    let good = std::fs::read(&path).unwrap();
    std::fs::write(&path, "{").unwrap();
    let mut cold = ship_engine(&tmp);
    for _ in 0..2 {
        assert_eq!(cold.observe().unwrap_err().kind, ReadErrorKind::Decode);
        assert!(cold.refresh_if_stale().is_err());
        assert_eq!(std::fs::read(&path).unwrap(), b"{");
    }
    std::fs::write(&path, &good).unwrap();
    assert_eq!(
        cold.observe().unwrap().value.objective.unwrap().description,
        "kept"
    );
    assert!(cold.refresh_if_stale().unwrap());
    assert_eq!(cold.objective.as_ref().unwrap().description, "kept");
}

#[test]
fn each_family_retries_refused_reads_and_observes_deletion() {
    use magistr_workflow_contracts::{ReadErrorKind, ReadSource};
    let tmp = TempDir::new().unwrap();
    let mut roadmap = roadmap_engine(&tmp);
    add_chunk(&mut roadmap, "kept");
    let mut signal = signal_engine(&tmp);
    signal
        .capture(SignalKind::Idea, "author".into(), "kept".into())
        .unwrap();
    let mut ship = ship_engine(&tmp);
    ship.set_objective("kept".into(), vec![], vec![], "chunk:kept".into())
        .unwrap();
    let paths = [
        Persistence::new(&infra_cfg(&tmp), Domain::Roadmap).path_for("proj"),
        Persistence::new(&infra_cfg(&tmp), Domain::Signal).path_for("proj"),
        ship_persistence::Persistence::new(&ship_persistence::PersistenceConfig {
            enabled: true,
            data_dir: tmp.path().to_owned(),
        })
        .path_for("proj"),
    ];
    let good: Vec<_> = paths
        .iter()
        .map(|path| std::fs::read(path).unwrap())
        .collect();
    for path in &paths {
        std::fs::write(path, "{").unwrap();
    }
    for _ in 0..2 {
        assert_eq!(roadmap.observe().unwrap_err().kind, ReadErrorKind::Decode);
        assert_eq!(signal.observe().unwrap_err().kind, ReadErrorKind::Decode);
        assert_eq!(ship.observe().unwrap_err().kind, ReadErrorKind::Decode);
        assert!(roadmap.refresh_if_stale().is_err());
        assert!(signal.refresh_if_stale().is_err());
        assert!(ship.refresh_if_stale().is_err());
    }
    assert_eq!(roadmap.roadmap().chunks.len(), 1);
    assert_eq!(signal.signals().signals.len(), 1);
    assert!(ship.objective.is_some());
    for (path, bytes) in paths.iter().zip(good) {
        std::fs::write(path, bytes).unwrap();
    }
    assert_eq!(roadmap.observe().unwrap().value.chunks.len(), 1);
    assert_eq!(signal.observe().unwrap().value.signals.len(), 1);
    assert!(ship.observe().unwrap().value.objective.is_some());
    for path in &paths {
        std::fs::remove_file(path).unwrap();
    }
    assert_eq!(roadmap.observe().unwrap().source, ReadSource::Missing);
    assert!(roadmap.observe().unwrap().value.chunks.is_empty());
    assert_eq!(signal.observe().unwrap().source, ReadSource::Missing);
    assert!(signal.observe().unwrap().value.signals.is_empty());
    assert_eq!(ship.observe().unwrap().source, ReadSource::Missing);
    assert!(ship.observe().unwrap().value.objective.is_none());
}

#[test]
fn ship_observes_the_opened_execution_and_refuses_an_unsavable_mutation() {
    use magistr_workflow_contracts::ReadSource;
    let tmp = TempDir::new().unwrap();
    let config = ship_persistence::PersistenceConfig {
        enabled: true,
        data_dir: tmp.path().to_owned(),
    };
    let a_store = ship_persistence::Persistence::new(&config).for_execution("a");
    let b_store = ship_persistence::Persistence::new(&config).for_execution("b");
    let a_path = a_store.path_for("proj");
    let mut a = ShipEngine::new("proj".into()).with_persistence(a_store.clone());
    let a_reader = ShipEngine::new("proj".into()).with_persistence(a_store);
    let mut b = ShipEngine::new("proj".into()).with_persistence(b_store);
    a.set_objective("execution a".into(), vec![], vec![], "chunk:a".into())
        .unwrap();
    b.set_objective("execution b".into(), vec![], vec![], "chunk:b".into())
        .unwrap();
    assert_eq!(
        a_reader
            .observe()
            .unwrap()
            .value
            .objective
            .unwrap()
            .description,
        "execution a"
    );
    assert_eq!(
        b.observe().unwrap().value.objective.unwrap().description,
        "execution b"
    );
    let lock = a_path.with_extension("json.lock");
    std::fs::remove_file(&lock).unwrap();
    std::fs::create_dir(&lock).unwrap();
    let refused = a
        .add_task(
            "unsaved".into(),
            "Refuse the unsavable task".into(),
            TaskType::Implement,
            None,
            None,
        )
        .unwrap_err();
    assert_eq!(refused.effect(), "none");
    assert!(a.tasks.is_empty());
    assert!(a.observe().unwrap().value.tasks.is_empty());
    assert!(a.tasks.is_empty());
    std::fs::remove_file(&a_path).unwrap();
    assert_eq!(a_reader.observe().unwrap().source, ReadSource::Missing);
    assert!(b.observe().unwrap().value.objective.is_some());
}

#[test]
fn signal_failure_keeps_the_last_committed_memory_and_observation() {
    let tmp = TempDir::new().unwrap();
    let mut signal = signal_engine(&tmp);
    signal
        .capture(SignalKind::Idea, "author".into(), "saved".into())
        .unwrap();
    let path = Persistence::new(&infra_cfg(&tmp), Domain::Signal).path_for("proj");
    let lock = path.with_extension("json.lock");
    std::fs::remove_file(&lock).unwrap();
    std::fs::create_dir(&lock).unwrap();
    let refused = signal
        .capture(SignalKind::Idea, "author".into(), "unsaved".into())
        .unwrap_err();
    assert!(
        matches!(refused, magistr_workflow::signal::engine::SignalMutationError::Store(ref error) if error.effect() == "none")
    );
    assert_eq!(signal.signals().signals.len(), 1);
    let read = signal.observe().unwrap();
    assert_eq!(read.value.signals.len(), 1);
    assert_eq!(read.value.signals[0].body, "saved");
    assert_eq!(signal.signals().signals.len(), 1);
}

#[test]
fn reasoning_reads_preserve_authored_ids_numbers_and_file_bytes() {
    use magistr_workflow::think::{
        config::ThinkConfig, domain::ThinkStep, engine::ReasoningServer,
    };
    use magistr_workflow_contracts::ReadErrorKind;
    let tmp = TempDir::new().unwrap();
    let mut config = ThinkConfig {
        persistence: infra_cfg(&tmp),
        ..ThinkConfig::default()
    };
    config.display.color_output = false;
    let mut writer = ReasoningServer::new_for_project(config.clone(), "proj".into());
    for number in [7, 9] {
        let step: ThinkStep = serde_json::from_value(serde_json::json!({
            "step_number": number, "estimated_total": 10, "purpose": "check",
            "context": "read integrity", "thought": format!("step {number}"),
            "outcome": "observed", "next_action": "continue", "rationale": "preserve identity"
        }))
        .unwrap();
        assert!(writer.process_step_native(step).is_ok());
    }
    let persistence =
        magistr_workflow::think::persistence::Persistence::for_project(&config.persistence, "proj");
    let path = persistence.store_path().unwrap();
    let original = std::fs::read(&path).unwrap();
    let before = writer.observe_archive().unwrap();
    let reader = ReasoningServer::new_for_project(config, "proj".into());
    let after = reader.observe_archive().unwrap();
    assert_eq!(
        after
            .value
            .steps
            .iter()
            .map(|s| s.step_number)
            .collect::<Vec<_>>(),
        vec![7, 9]
    );
    assert_eq!(
        after
            .value
            .steps
            .iter()
            .map(|s| &s.record_id)
            .collect::<Vec<_>>(),
        before
            .value
            .steps
            .iter()
            .map(|s| &s.record_id)
            .collect::<Vec<_>>()
    );
    assert_eq!(std::fs::read(&path).unwrap(), original);
    let mut invalid: serde_json::Value = serde_json::from_slice(&original).unwrap();
    invalid["histories"]["proj"]["steps"][1]["step_number"] = serde_json::json!(7);
    let bytes = serde_json::to_vec(&invalid).unwrap();
    std::fs::write(&path, &bytes).unwrap();
    assert_eq!(
        reader.observe_archive().unwrap_err().kind,
        ReadErrorKind::Identity
    );
    assert_eq!(std::fs::read(&path).unwrap(), bytes);
}

#[test]
fn reasoning_observes_selected_history_from_one_validated_project() {
    use magistr_workflow::think::{
        config::ThinkConfig, domain::ThinkStep, engine::ReasoningServer,
        persistence::Persistence as ThinkPersistence,
    };
    use magistr_workflow_contracts::{ReadErrorKind, ReadSource};
    let tmp = TempDir::new().unwrap();
    let mut config = ThinkConfig {
        persistence: infra_cfg(&tmp),
        ..ThinkConfig::default()
    };
    config.display.color_output = false;
    let step = |number, session: Option<&str>| -> ThinkStep {
        serde_json::from_value(serde_json::json!({
            "step_number": number, "estimated_total": 10, "purpose": "analysis",
            "context": "session scope", "thought": format!("saved step {number}"),
            "outcome": "observed", "next_action": "continue", "rationale": "preserve scope",
            "session_id": session,
        }))
        .unwrap()
    };
    let mut default = ReasoningServer::new_for_project(config.clone(), "proj".into());
    default.process_step_native(step(1, None)).unwrap();
    let store = ThinkPersistence::for_project(&config.persistence, "proj");
    let default_history = default.observe_archive().unwrap().value;
    config.features.enable_sessions = true;
    config.features.default_session_id = Some("chosen".into());
    let mut writer = ReasoningServer::new_for_project(config.clone(), "proj".into());
    let missing = writer.observe_archive().unwrap();
    assert!(matches!(missing.source, ReadSource::File(_)));
    assert_eq!(missing.value.session_id.as_deref(), Some("proj__chosen"));
    assert!(writer.history().steps.is_empty());
    writer.process_step_native(step(7, None)).unwrap();
    let chosen = writer.observe_archive().unwrap();
    assert_eq!(chosen.value.session_id.as_deref(), Some("proj__chosen"));
    assert_eq!(chosen.value.steps[0].step_number, 7);
    let detached = ReasoningServer::new_for_project(config.clone(), "proj".into());
    assert_eq!(
        serde_json::to_value(detached.observe_archive().unwrap().value).unwrap(),
        serde_json::to_value(&chosen.value).unwrap()
    );
    writer.process_step_native(step(9, Some("other"))).unwrap();
    let other = writer.observe_archive().unwrap();
    assert_eq!(other.value.session_id.as_deref(), Some("proj__other"));
    assert_eq!(other.value.steps.len(), 1);
    assert_eq!(other.value.steps[0].step_number, 9);
    assert_eq!(
        serde_json::to_value(detached.observe_archive().unwrap().value).unwrap(),
        serde_json::to_value(&chosen.value).unwrap()
    );

    let other_path = store.store_path().unwrap();
    let bytes = std::fs::read(&other_path).unwrap();
    let mut wrong: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    wrong["histories"]["proj__other"]["session_id"] = serde_json::json!("proj__chosen");
    let wrong_bytes = serde_json::to_vec(&wrong).unwrap();
    std::fs::write(&other_path, &wrong_bytes).unwrap();
    assert_eq!(
        writer.observe_archive().unwrap_err().kind,
        ReadErrorKind::Identity
    );
    assert_eq!(std::fs::read(&other_path).unwrap(), wrong_bytes);
    assert_eq!(writer.history().steps[0].step_number, 9);
    assert_eq!(
        detached.observe_archive().unwrap_err().kind,
        ReadErrorKind::Identity
    );
    std::fs::write(&other_path, bytes).unwrap();
    assert_eq!(
        serde_json::to_value(writer.observe_archive().unwrap().value).unwrap(),
        serde_json::to_value(other.value).unwrap()
    );

    config.features.default_session_id = Some("proj".into());
    let project_session = ReasoningServer::new_for_project(config, "proj".into());
    let observed = project_session.observe_archive().unwrap();
    assert_eq!(observed.value.session_id, None);
    assert_eq!(observed.value.steps[0].step_number, 1);
    assert_eq!(
        serde_json::to_value(observed.value).unwrap(),
        serde_json::to_value(default_history).unwrap()
    );
}

#[test]
fn refresh_revalidates_bytes_even_when_the_public_stamp_is_unchanged() {
    use magistr_workflow_contracts::{ReadErrorKind, ReadSource};
    let tmp = TempDir::new().unwrap();
    let mut owner = roadmap_engine(&tmp);
    add_chunk(&mut owner, "saved");
    let path = Persistence::new(&infra_cfg(&tmp), Domain::Roadmap).path_for("proj");
    let before = owner.observe().unwrap();
    let ReadSource::File(stamp) = before.source else {
        panic!("saved file");
    };
    let mut corrupted = std::fs::read(&path).unwrap();
    corrupted[0] = b'!';
    std::fs::write(&path, corrupted).unwrap();
    std::fs::File::options()
        .write(true)
        .open(&path)
        .unwrap()
        .set_times(std::fs::FileTimes::new().set_modified(stamp.modified))
        .unwrap();
    let metadata = std::fs::metadata(&path).unwrap();
    assert_eq!(metadata.modified().unwrap(), stamp.modified);
    assert_eq!(metadata.len(), stamp.length);
    assert_eq!(owner.observe().unwrap_err().kind, ReadErrorKind::Decode);
    assert!(owner.refresh_if_stale().is_err());
    assert_eq!(owner.roadmap().chunks[0].id, "saved");
}
