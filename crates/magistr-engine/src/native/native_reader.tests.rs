use super::*;
use crate::native::{NativeEngines, ProjectAuthority, TaskStatus, WriterRole};

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

fn add(writer: &NativeEngines, id: &str, priority: u32, feature: Option<&str>) {
    writer
        .plan_add_task(
            id.into(),
            format!("Work {id}"),
            String::new(),
            TaskStatus::ToDo,
            priority,
            String::new(),
            vec![],
            vec![],
            None,
            feature.map(str::to_owned),
        )
        .unwrap();
}

#[test]
fn current_workstreams_refresh_without_redirecting_closed_or_missing_names() {
    let (_temp, config, writer) = fixture();
    add(&writer, "old", 1, Some("Rust"));
    add(&writer, "desktop", 20, Some("Rust desktop"));
    add(&writer, "unassigned", 10, None);
    let observer = NativeReader::open_for_project(&config, "workstream-read".into());
    assert_eq!(
        observer.plan_workstreams().unwrap().current_names(),
        ["Rust", "Rust desktop"]
    );
    writer.plan_set_status("old", TaskStatus::Retired).unwrap();
    let catalog = observer.plan_workstreams().unwrap();
    assert_eq!(catalog.current_names(), ["Rust desktop"]);
    assert_eq!(catalog.history["Rust"].retired, 1);
    assert_eq!(catalog.unassigned.counts.total, 1);
    assert_eq!(observer.plan_tasks().unwrap().len(), 3);

    let refusal = observer.plan_resolve_track("Rust").unwrap_err();
    assert!(refusal.contains("no unfinished work"), "{refusal}");
    assert!(refusal.contains("Rust desktop"), "{refusal}");
    assert!(observer.plan_next_in_track("Rust").is_err());
    assert!(observer.plan_frontier(Some("Rust des")).is_err());
    assert_eq!(
        observer.plan_resolve_track("Rust des").unwrap(),
        "Rust desktop"
    );
    assert_eq!(observer.plan_next().unwrap().unwrap().id, "unassigned");
    let unassigned = observer.plan_frontier(None).unwrap();
    assert_eq!(unassigned.items.len(), 1);
    assert_eq!(unassigned.items[0].id, "unassigned");
    assert_eq!(
        observer
            .plan_next_in_track("Rust desktop")
            .unwrap()
            .unwrap()
            .id,
        "desktop"
    );

    writer
        .plan_set_feature("desktop", Some("Interface".into()))
        .unwrap();
    assert!(observer.plan_next_in_track("Rust desktop").is_err());
    assert_eq!(
        observer.plan_workstreams().unwrap().current_names(),
        ["Interface"]
    );
    writer
        .plan_set_status("desktop", TaskStatus::Retired)
        .unwrap();
    let catalog = observer.plan_workstreams().unwrap();
    assert!(catalog.current.is_empty());
    assert_eq!(catalog.history["Interface"].retired, 1);
    assert_eq!(observer.plan_next().unwrap().unwrap().id, "unassigned");
}

#[test]
fn selection_refuses_an_unreadable_store_and_recovers_without_cached_success() {
    use magistr_workflow::infra::{Domain, Persistence};

    let (_temp, config, writer) = fixture();
    add(&writer, "work", 1, Some("Current"));
    let observer = NativeReader::open_for_project(&config, "workstream-read".into());
    assert_eq!(observer.plan_next().unwrap().unwrap().id, "work");
    let path =
        Persistence::new(&config.persistence_config(), Domain::Plan).path_for("workstream-read");
    let original = std::fs::read(&path).unwrap();
    std::fs::write(&path, b"{ invalid plan").unwrap();

    assert!(observer.plan_tasks().is_err());
    assert!(observer.plan_next().is_err());
    assert!(observer.plan_next_in_track("Current").is_err());
    assert!(observer.plan_resolve_track("Current").is_err());
    assert!(observer.plan_workstreams().is_err());
    assert!(observer.plan_frontier(Some("Current")).is_err());
    let newly_opened = NativeReader::open_for_project(&config, "workstream-read".into());
    assert!(newly_opened.plan_tasks().is_err());
    assert!(newly_opened.plan_next().is_err());

    std::fs::write(&path, original).unwrap();
    assert_eq!(observer.plan_next().unwrap().unwrap().id, "work");
    assert_eq!(
        observer.plan_workstreams().unwrap().current_names(),
        ["Current"]
    );
}

#[test]
fn family_failures_retain_exact_records_across_clones_and_recover_independently() {
    use crate::native::{RequestKind, StepType};
    use magistr_workflow::infra::{Domain, Persistence};

    let (_temp, config, writer) = fixture();
    add(&writer, "work", 1, Some("Current"));
    writer
        .run_open("Keep the plan".into(), vec![], vec![], "task:work".into())
        .unwrap();
    writer
        .add_step(
            "review".into(),
            "Review the change".into(),
            StepType::Test,
            None,
        )
        .unwrap();
    writer
        .request_capture(
            RequestKind::Suggestion,
            "reader".into(),
            "Keep this request".into(),
        )
        .unwrap();
    let step = serde_json::from_value(serde_json::json!({
        "note": 1,
        "estimated_total": 1,
        "purpose": "investigation",
        "context": "Observation",
        "thought": "Keep this authored step.",
        "outcome": "Retain the evidence.",
        "next_action": "Review.",
        "rationale": "Read failures must preserve it."
    }))
    .unwrap();
    writer.decision_record(step).unwrap();
    let observer = NativeReader::open_for_project(&config, "workstream-read".into());
    let clone = observer.clone();
    let persistence = config.persistence_config();
    let paths = [
        (
            "plan",
            Persistence::new(&persistence, Domain::Plan).path_for("workstream-read"),
        ),
        (
            "run",
            persistence
                .data_dir
                .join("run")
                .join("executions")
                .join(config.execution_context().key())
                .join("sessions")
                .join("workstream-read.json"),
        ),
        (
            "requests",
            Persistence::new(&persistence, Domain::Request).path_for("workstream-read"),
        ),
        (
            "decisions",
            magistr_workflow::decision::persistence::Persistence::for_project(
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
            for other in ["plan", "run", "requests", "decision"] {
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
    use crate::native::{ReadSource, RequestKind};
    use magistr_workflow::infra::{Domain, Persistence};

    let (_temp, config, writer) = fixture();
    add(&writer, "work", 1, None);
    writer
        .request_capture(
            RequestKind::Suggestion,
            "reader".into(),
            "Retain the request".into(),
        )
        .unwrap();
    let observer = NativeReader::open_for_project(&config, "workstream-read".into());
    let before = observer.snapshot();
    let path =
        Persistence::new(&config.persistence_config(), Domain::Plan).path_for("workstream-read");
    std::fs::remove_file(path).unwrap();
    let empty = observer.snapshot();
    assert_eq!(empty.plan.current().unwrap().source, ReadSource::Missing);
    assert!(empty.plan.current().unwrap().value.tasks.is_empty());
    assert_eq!(empty.requests, before.requests);
    assert_eq!(observer.plan_next().unwrap(), None);
}

#[test]
fn a_family_read_holds_its_owner_until_the_retained_observation_is_recorded() {
    use std::time::{Duration, Instant};

    let (_temp, config, writer) = fixture();
    add(&writer, "first", 1, None);
    let observer = NativeReader::open_for_project(&config, "workstream-read".into());
    observer.plan_tasks().unwrap();
    let cache = lock(&observer.observations);
    let first = observer.clone();
    let first_read = std::thread::spawn(move || first.plan_tasks().unwrap());
    let deadline = Instant::now() + Duration::from_secs(5);
    while observer.plan.try_lock().is_ok() {
        assert!(
            Instant::now() < deadline,
            "the first reader did not acquire its owner"
        );
        std::thread::yield_now();
    }
    add(&writer, "second", 2, None);
    let second = observer.clone();
    let second_read = std::thread::spawn(move || second.plan_tasks().unwrap());
    drop(cache);
    // The external write may precede the first file open. The second read
    // must still be the observation retained after both callers finish.
    first_read.join().unwrap();
    assert_eq!(second_read.join().unwrap().len(), 2);
    assert_eq!(
        lock(&observer.observations)
            .plan
            .current()
            .unwrap()
            .value
            .tasks
            .len(),
        2
    );
}

#[test]
fn failed_log_reads_retain_the_previous_logs_identity_until_recovery() {
    use crate::native::ReadState;
    use magistr_workflow::decision::config::namespace_log_id;
    use magistr_workflow::decision::persistence::Persistence;

    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().join("project");
    std::fs::create_dir(&root).unwrap();
    let config = NativeConfig::from_lookup(root, |key| match key {
        "MAGISTR_WORKFLOW_ENABLE_LOGS" => Some("true".into()),
        "MAGISTR_WORKFLOW_DEFAULT_LOG_ID" => Some("alpha".into()),
        _ => None,
    })
    .with_data_dir(temp.path().join("data"));
    let writer = ProjectAuthority::admit_for_project(&config, "log-read".into(), WriterRole::Host)
        .unwrap()
        .open();
    let note = |number, log| {
        serde_json::from_value(serde_json::json!({
            "note": number,
            "estimated_total": 2,
            "purpose": "investigation",
            "context": "Selected reasoning log",
            "thought": format!("Evidence from {log}"),
            "outcome": "Keep the selected log identity.",
            "next_action": "Review.",
            "rationale": "A read failure must not relabel retained records.",
            "log_id": log
        }))
        .unwrap()
    };
    writer.decision_record(note(1, "alpha")).unwrap();
    let observer = writer.reader();
    let first = observer.snapshot().decisions.current().unwrap().clone();
    let alpha = namespace_log_id("log-read", "alpha");
    let beta = namespace_log_id("log-read", "beta");
    assert_eq!(first.value.logs.len(), 1);
    assert_eq!(
        first.value.log(Some(&alpha)).unwrap().notes[0].thought,
        "Evidence from alpha"
    );

    writer.decision_record(note(2, "beta")).unwrap();
    let persistence = Persistence::for_project(&config.persistence_config(), "log-read");
    let path = persistence.store_path().unwrap();
    let saved = std::fs::read(&path).unwrap();
    std::fs::write(&path, b"{ unreadable reasoning").unwrap();
    for _ in 0..2 {
        let ReadState::Stale { last, .. } = observer.snapshot().decisions else {
            panic!("failed selected log must keep its last successful observation");
        };
        assert_eq!(last, first);
        assert!(observer.note(1).is_err());
    }

    // All reasoning logs share the canonical project document. A new
    // reader cannot certify any log while that document is unreadable.
    let detached = NativeReader::open_for_project(&config, "log-read".into());
    assert!(matches!(
        detached.snapshot().decisions,
        ReadState::Unavailable(_)
    ));

    std::fs::write(path, saved).unwrap();
    let recovered = observer.snapshot().decisions.current().unwrap().clone();
    assert_eq!(recovered.value.logs.len(), 2);
    assert_eq!(
        recovered.value.log(Some(&beta)).unwrap().notes[0].thought,
        "Evidence from beta"
    );
    assert_eq!(
        observer.note(1).unwrap().unwrap().thought,
        "Evidence from alpha"
    );
    assert_eq!(
        observer.decision_log(Some("alpha")).unwrap(),
        detached.decision_log(Some("alpha")).unwrap()
    );
    assert!(observer.decision_log(None).unwrap().is_none());
}

#[test]
fn complete_execution_history_survives_new_work_and_failed_reads() {
    use crate::native::{StepType, VerificationMode};
    let (_temp, config, writer) = fixture();
    writer
        .run_open_with_verification(
            "Finished work".into(),
            vec![],
            vec![],
            "first".into(),
            VerificationMode::Focused,
        )
        .unwrap();
    writer
        .add_step("verify".into(), "Verify it".into(), StepType::Test, None)
        .unwrap();
    writer.start_step("verify").unwrap();
    let check = writer
        .run_record_gate(
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
    writer.finish_step("verify", vec![]).unwrap();
    writer.run_finish(vec![], None).unwrap();
    let reader = NativeReader::open_for_project(&config, "workstream-read".into());
    let previous = reader.run_snapshot().unwrap().current.unwrap();
    writer
        .run_open_with_verification(
            "Next work".into(),
            vec![],
            vec![],
            "first".into(),
            VerificationMode::Focused,
        )
        .unwrap();
    let reopened = NativeReader::open_for_project(&config, "workstream-read".into());
    assert_eq!(
        reopened.run_run(&previous.run_id).unwrap(),
        Some(previous.clone())
    );
    assert!(reopened.run_run("absent").unwrap().is_none());
    assert_eq!(reopened.run_steps().unwrap().len(), 0);
    let check_result = reopened
        .run_check_result(check.check_result_id.as_ref().unwrap())
        .unwrap()
        .unwrap();
    assert_eq!(check_result.target.run_id, previous.run_id);
    let path = config
        .persistence_config()
        .data_dir
        .join("run")
        .join("executions")
        .join(config.execution_context().key())
        .join("sessions")
        .join("workstream-read.json");
    let bytes = std::fs::read(&path).unwrap();
    std::fs::write(&path, b"invalid").unwrap();
    assert!(reopened.run_run(&previous.run_id).is_err());
    assert!(reopened.run_run("absent").is_err());
    assert!(
        reopened
            .run_check_result(&check_result.id)
            .unwrap()
            .is_some()
    );
    std::fs::write(&path, bytes).unwrap();
    assert_eq!(reopened.run_run(&previous.run_id).unwrap(), Some(previous));
}
