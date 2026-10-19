use super::*;
use crate::native::{ChunkStatus, NativeEngines, ProjectAuthority, WriterRole};

fn fixture() -> (tempfile::TempDir, NativeConfig, NativeEngines) {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().join("project");
    std::fs::create_dir(&root).unwrap();
    let config = NativeConfig::from_lookup(root, |_| None).with_data_dir(temp.path().join("data"));
    let writer =
        ProjectAuthority::admit_for_project(&config, "workstream-read".into(), WriterRole::Host)
            .unwrap()
            .open();
    (temp, config, writer)
}

fn add(writer: &NativeEngines, id: &str, priority: u32, group: Option<&str>) {
    writer
        .roadmap_add_chunk(
            id.into(),
            format!("Work {id}"),
            String::new(),
            ChunkStatus::Pending,
            priority,
            String::new(),
            vec![],
            vec![],
            None,
            group.map(str::to_owned),
        )
        .unwrap();
}

#[test]
fn current_workstreams_refresh_without_redirecting_closed_or_missing_names() {
    let (_temp, config, writer) = fixture();
    add(&writer, "old", 1, Some("Rust"));
    add(&writer, "desktop", 20, Some("Rust desktop"));
    add(&writer, "ungrouped", 10, None);
    let observer = NativeReader::open_for_project(&config, "workstream-read".into());
    assert_eq!(
        observer.roadmap_workstreams().unwrap().current_names(),
        ["Rust", "Rust desktop"]
    );
    writer
        .roadmap_set_status("old", ChunkStatus::Obsoleted)
        .unwrap();
    let catalog = observer.roadmap_workstreams().unwrap();
    assert_eq!(catalog.current_names(), ["Rust desktop"]);
    assert_eq!(catalog.history["Rust"].obsoleted, 1);
    assert_eq!(catalog.ungrouped.counts.total, 1);
    assert_eq!(observer.roadmap_chunks().unwrap().len(), 3);

    let refusal = observer.roadmap_resolve_track("Rust").unwrap_err();
    assert!(refusal.contains("no unfinished work"), "{refusal}");
    assert!(refusal.contains("Rust desktop"), "{refusal}");
    assert!(observer.roadmap_next_in_track("Rust").is_err());
    assert!(observer.roadmap_frontier(Some("Rust des")).is_err());
    assert_eq!(
        observer.roadmap_resolve_track("Rust des").unwrap(),
        "Rust desktop"
    );
    assert_eq!(observer.roadmap_next().unwrap().unwrap().id, "ungrouped");
    let ungrouped = observer.roadmap_frontier(None).unwrap();
    assert_eq!(ungrouped.items.len(), 1);
    assert_eq!(ungrouped.items[0].id, "ungrouped");
    assert_eq!(
        observer
            .roadmap_next_in_track("Rust desktop")
            .unwrap()
            .unwrap()
            .id,
        "desktop"
    );

    writer
        .roadmap_set_group("desktop", Some("Interface".into()))
        .unwrap();
    assert!(observer.roadmap_next_in_track("Rust desktop").is_err());
    assert_eq!(
        observer.roadmap_workstreams().unwrap().current_names(),
        ["Interface"]
    );
    writer
        .roadmap_set_status("desktop", ChunkStatus::Obsoleted)
        .unwrap();
    let catalog = observer.roadmap_workstreams().unwrap();
    assert!(catalog.current.is_empty());
    assert_eq!(catalog.history["Interface"].obsoleted, 1);
    assert_eq!(observer.roadmap_next().unwrap().unwrap().id, "ungrouped");
}

#[test]
fn selection_refuses_an_unreadable_store_and_recovers_without_cached_success() {
    use magistr_workflow::infra::{Domain, Persistence};

    let (_temp, config, writer) = fixture();
    add(&writer, "work", 1, Some("Current"));
    let observer = NativeReader::open_for_project(&config, "workstream-read".into());
    assert_eq!(observer.roadmap_next().unwrap().unwrap().id, "work");
    let path =
        Persistence::new(&config.persistence_config(), Domain::Roadmap).path_for("workstream-read");
    let original = std::fs::read(&path).unwrap();
    std::fs::write(&path, b"{ invalid roadmap").unwrap();

    assert!(observer.roadmap_chunks().is_err());
    assert!(observer.roadmap_next().is_err());
    assert!(observer.roadmap_next_in_track("Current").is_err());
    assert!(observer.roadmap_resolve_track("Current").is_err());
    assert!(observer.roadmap_workstreams().is_err());
    assert!(observer.roadmap_frontier(Some("Current")).is_err());
    let newly_opened = NativeReader::open_for_project(&config, "workstream-read".into());
    assert!(newly_opened.roadmap_chunks().is_err());
    assert!(newly_opened.roadmap_next().is_err());

    std::fs::write(&path, original).unwrap();
    assert_eq!(observer.roadmap_next().unwrap().unwrap().id, "work");
    assert_eq!(
        observer.roadmap_workstreams().unwrap().current_names(),
        ["Current"]
    );
}

#[test]
fn family_failures_retain_exact_records_across_clones_and_recover_independently() {
    use crate::native::{SignalKind, TaskType};
    use magistr_workflow::infra::{Domain, Persistence};

    let (_temp, config, writer) = fixture();
    add(&writer, "work", 1, Some("Current"));
    writer
        .ship_set_objective("Keep the plan".into(), vec![], vec![], "chunk:work".into())
        .unwrap();
    writer
        .ship_add_task(
            "review".into(),
            "Review the change".into(),
            TaskType::Test,
            None,
        )
        .unwrap();
    writer
        .signal_capture(SignalKind::Idea, "reader".into(), "Keep this signal".into())
        .unwrap();
    let step = serde_json::from_value(serde_json::json!({
        "step_number": 1,
        "estimated_total": 1,
        "purpose": "investigation",
        "context": "Observation",
        "thought": "Keep this authored step.",
        "outcome": "Retain the evidence.",
        "next_action": "Review.",
        "rationale": "Read failures must preserve it."
    }))
    .unwrap();
    writer.think_record_step(step).unwrap();
    let observer = NativeReader::open_for_project(&config, "workstream-read".into());
    let clone = observer.clone();
    let persistence = config.persistence_config();
    let paths = [
        (
            "roadmap",
            Persistence::new(&persistence, Domain::Roadmap).path_for("workstream-read"),
        ),
        (
            "ship",
            persistence
                .data_dir
                .join("ship")
                .join("executions")
                .join(config.execution_context().key())
                .join("sessions")
                .join("workstream-read.json"),
        ),
        (
            "signals",
            Persistence::new(&persistence, Domain::Signal).path_for("workstream-read"),
        ),
        (
            "think",
            magistr_workflow::think::persistence::Persistence::for_project(
                &persistence,
                "workstream-read",
            )
            .store_path()
            .unwrap(),
        ),
    ];
    for (family, path) in paths {
        let good = serde_json::to_value(observer.snapshot()).unwrap();
        let original = std::fs::read(&path).unwrap();
        std::fs::write(&path, b"{ unreadable record").unwrap();
        for _ in 0..2 {
            let failed = serde_json::to_value(clone.snapshot()).unwrap();
            assert_eq!(failed[family]["state"], "stale", "{family}");
            assert_eq!(
                failed[family]["data"]["last"], good[family]["data"],
                "{family}"
            );
            for other in ["roadmap", "ship", "signals", "think"] {
                if other != family {
                    assert_eq!(
                        failed[other], good[other],
                        "{family} must not change {other}"
                    );
                }
            }
        }
        let cold = NativeReader::open_for_project(&config, "workstream-read".into());
        let unavailable = serde_json::to_value(cold.snapshot()).unwrap();
        assert_eq!(unavailable[family]["state"], "unavailable", "{family}");
        assert_eq!(std::fs::read(&path).unwrap(), b"{ unreadable record");
        std::fs::write(&path, original).unwrap();
        let recovered = serde_json::to_value(clone.snapshot()).unwrap();
        assert_eq!(recovered[family]["state"], "current", "{family}");
        assert_eq!(
            recovered[family]["data"]["value"], good[family]["data"]["value"],
            "{family}"
        );
    }
}

#[test]
fn observed_deletion_is_current_empty_and_does_not_clear_other_families() {
    use crate::native::{ReadSource, SignalKind};
    use magistr_workflow::infra::{Domain, Persistence};

    let (_temp, config, writer) = fixture();
    add(&writer, "work", 1, None);
    writer
        .signal_capture(
            SignalKind::Idea,
            "reader".into(),
            "Retain the signal".into(),
        )
        .unwrap();
    let observer = NativeReader::open_for_project(&config, "workstream-read".into());
    let before = observer.snapshot();
    let path =
        Persistence::new(&config.persistence_config(), Domain::Roadmap).path_for("workstream-read");
    std::fs::remove_file(path).unwrap();
    let empty = observer.snapshot();
    assert_eq!(empty.roadmap.current().unwrap().source, ReadSource::Missing);
    assert!(empty.roadmap.current().unwrap().value.chunks.is_empty());
    assert_eq!(empty.signals, before.signals);
    assert_eq!(observer.roadmap_next().unwrap(), None);
}

#[test]
fn a_family_read_holds_its_owner_until_the_retained_observation_is_recorded() {
    use std::time::{Duration, Instant};

    let (_temp, config, writer) = fixture();
    add(&writer, "first", 1, None);
    let observer = NativeReader::open_for_project(&config, "workstream-read".into());
    observer.roadmap_chunks().unwrap();
    let cache = lock(&observer.observations);
    let first = observer.clone();
    let first_read = std::thread::spawn(move || first.roadmap_chunks().unwrap());
    let deadline = Instant::now() + Duration::from_secs(5);
    while observer.roadmap.try_lock().is_ok() {
        assert!(
            Instant::now() < deadline,
            "the first reader did not acquire its owner"
        );
        std::thread::yield_now();
    }
    add(&writer, "second", 2, None);
    let second = observer.clone();
    let second_read = std::thread::spawn(move || second.roadmap_chunks().unwrap());
    drop(cache);
    // The external write may precede the first file open. The second read
    // must still be the observation retained after both callers finish.
    first_read.join().unwrap();
    assert_eq!(second_read.join().unwrap().len(), 2);
    assert_eq!(
        lock(&observer.observations)
            .roadmap
            .current()
            .unwrap()
            .value
            .chunks
            .len(),
        2
    );
}

#[test]
fn failed_session_reads_retain_the_previous_sessions_identity_until_recovery() {
    use crate::native::ReadState;
    use magistr_workflow::think::config::namespace_session_id;
    use magistr_workflow::think::persistence::Persistence;

    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().join("project");
    std::fs::create_dir(&root).unwrap();
    let config = NativeConfig::from_lookup(root, |key| match key {
        "MAGISTR_WORKFLOW_ENABLE_SESSIONS" => Some("true".into()),
        "MAGISTR_WORKFLOW_DEFAULT_SESSION_ID" => Some("alpha".into()),
        _ => None,
    })
    .with_data_dir(temp.path().join("data"));
    let writer =
        ProjectAuthority::admit_for_project(&config, "session-read".into(), WriterRole::Host)
            .unwrap()
            .open();
    let step = |number, session| {
        serde_json::from_value(serde_json::json!({
            "step_number": number,
            "estimated_total": 2,
            "purpose": "investigation",
            "context": "Selected reasoning session",
            "thought": format!("Evidence from {session}"),
            "outcome": "Keep the selected session identity.",
            "next_action": "Review.",
            "rationale": "A read failure must not relabel retained records.",
            "session_id": session
        }))
        .unwrap()
    };
    writer.think_record_step(step(1, "alpha")).unwrap();
    let observer = writer.reader();
    let first = observer.snapshot().think.current().unwrap().clone();
    let alpha = namespace_session_id("session-read", "alpha");
    let beta = namespace_session_id("session-read", "beta");
    assert_eq!(first.value.sessions.len(), 1);
    assert_eq!(
        first.value.session(Some(&alpha)).unwrap().steps[0].thought,
        "Evidence from alpha"
    );

    writer.think_record_step(step(2, "beta")).unwrap();
    let persistence = Persistence::for_project(&config.persistence_config(), "session-read");
    let path = persistence.store_path().unwrap();
    let saved = std::fs::read(&path).unwrap();
    std::fs::write(&path, b"{ unreadable reasoning").unwrap();
    for _ in 0..2 {
        let ReadState::Stale { last, .. } = observer.snapshot().think else {
            panic!("failed selected session must keep its last successful observation");
        };
        assert_eq!(last, first);
        assert!(observer.think_step(1).is_err());
    }

    // All reasoning sessions share the canonical project document. A new
    // reader cannot certify any session while that document is unreadable.
    let detached = NativeReader::open_for_project(&config, "session-read".into());
    assert!(matches!(
        detached.snapshot().think,
        ReadState::Unavailable(_)
    ));

    std::fs::write(path, saved).unwrap();
    let recovered = observer.snapshot().think.current().unwrap().clone();
    assert_eq!(recovered.value.sessions.len(), 2);
    assert_eq!(
        recovered.value.session(Some(&beta)).unwrap().steps[0].thought,
        "Evidence from beta"
    );
    assert_eq!(
        observer.think_step(1).unwrap().unwrap().thought,
        "Evidence from alpha"
    );
    assert_eq!(
        observer.think_session(Some("alpha")).unwrap(),
        detached.think_session(Some("alpha")).unwrap()
    );
    assert!(observer.think_session(None).unwrap().is_none());
}

#[test]
fn complete_execution_history_survives_new_work_and_failed_reads() {
    use crate::native::{TaskType, VerificationMode};
    let (_temp, config, writer) = fixture();
    writer
        .ship_set_objective_with_verification(
            "Finished work".into(),
            vec![],
            vec![],
            "first".into(),
            VerificationMode::Focused,
        )
        .unwrap();
    writer
        .ship_add_task("verify".into(), "Verify it".into(), TaskType::Test, None)
        .unwrap();
    writer.ship_start_task("verify").unwrap();
    let check = writer
        .ship_record_gate(
            Some("verify"),
            "focused",
            "true".into(),
            0,
            "actual result".into(),
            true,
            None,
        )
        .unwrap()
        .value;
    writer.ship_complete_task("verify", vec![]).unwrap();
    writer.ship_finalize(vec![], None).unwrap();
    let reader = NativeReader::open_for_project(&config, "workstream-read".into());
    let previous = reader.ship_snapshot().unwrap().current.unwrap();
    writer
        .ship_set_objective_with_verification(
            "Next work".into(),
            vec![],
            vec![],
            "first".into(),
            VerificationMode::Focused,
        )
        .unwrap();
    let reopened = NativeReader::open_for_project(&config, "workstream-read".into());
    assert_eq!(
        reopened.ship_run(&previous.run_id).unwrap(),
        Some(previous.clone())
    );
    assert!(reopened.ship_run("absent").unwrap().is_none());
    assert_eq!(reopened.ship_tasks().unwrap().len(), 0);
    let receipt = reopened
        .ship_receipt(check.receipt_id.as_ref().unwrap())
        .unwrap()
        .unwrap();
    assert_eq!(receipt.target.run_id, previous.run_id);
    let path = config
        .persistence_config()
        .data_dir
        .join("ship")
        .join("executions")
        .join(config.execution_context().key())
        .join("sessions")
        .join("workstream-read.json");
    let bytes = std::fs::read(&path).unwrap();
    std::fs::write(&path, b"invalid").unwrap();
    assert!(reopened.ship_run(&previous.run_id).is_err());
    assert!(reopened.ship_run("absent").is_err());
    assert!(reopened.ship_receipt(&receipt.id).unwrap().is_some());
    std::fs::write(&path, bytes).unwrap();
    assert_eq!(reopened.ship_run(&previous.run_id).unwrap(), Some(previous));
}
