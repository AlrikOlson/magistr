//! Validated family observations distinguish durable data, absence and read failure.

use std::thread::sleep;
use std::time::Duration;

use tempfile::TempDir;

use magistr_workflow::infra::{Domain, Persistence, PersistenceConfig};
use magistr_workflow::plan::PlanEngine;
use magistr_workflow::plan::domain::TaskStatus;
use magistr_workflow::run::domain::step::StepType;
use magistr_workflow::run::engine::RunEngine;
use magistr_workflow::run::persistence as run_persistence;
use magistr_workflow::signal::SignalEngine;
use magistr_workflow::signal::domain::{SignalKind, SignalStatus};

fn infra_cfg(tmp: &TempDir) -> PersistenceConfig {
    PersistenceConfig::from_env()
        .with_data_dir(tmp.path().to_path_buf())
        .enabled(true)
}

fn plan_engine(tmp: &TempDir) -> PlanEngine {
    PlanEngine::new("proj".into()).with_persistence(Persistence::new(&infra_cfg(tmp), Domain::Plan))
}

fn signal_engine(tmp: &TempDir) -> SignalEngine {
    SignalEngine::new("proj".into())
        .with_persistence(Persistence::new(&infra_cfg(tmp), Domain::Signal))
}

fn run_engine(tmp: &TempDir) -> RunEngine {
    let cfg = run_persistence::PersistenceConfig {
        enabled: true,
        data_dir: tmp.path().to_path_buf(),
    };
    RunEngine::new("proj".into()).with_persistence(run_persistence::Persistence::new(&cfg))
}

fn add_task(e: &mut PlanEngine, id: &str) {
    e.add_task(
        id.into(),
        format!("Task {id}"),
        TaskStatus::ToDo,
        10,
        String::new(),
        Vec::new(),
        Vec::new(),
        false,
    )
    .unwrap();
}

fn task_ids(e: &PlanEngine) -> Vec<String> {
    let mut ids: Vec<String> = e.plan().tasks.iter().map(|c| c.id.clone()).collect();
    ids.sort();
    ids
}

#[test]
fn a_plan_refresh_validates_the_store_and_reports_changes() {
    let tmp = TempDir::new().unwrap();
    let mut a = plan_engine(&tmp);
    let mut b = plan_engine(&tmp);
    assert_eq!(a.refresh_if_stale(), Ok(false), "nothing written yet");

    add_task(&mut b, "from-b");
    assert_eq!(a.refresh_if_stale(), Ok(true), "the stamp moved");
    assert_eq!(task_ids(&a), vec!["from-b".to_owned()]);
    assert_eq!(a.refresh_if_stale(), Ok(false), "unchanged: no re-read");

    // A's own save records the stamp of the file it wrote, so its next
    // refresh is a stat; B, which never saw it, reads once.
    add_task(&mut a, "from-a");
    assert_eq!(a.refresh_if_stale(), Ok(false));
    assert_eq!(b.refresh_if_stale(), Ok(true));
    assert_eq!(task_ids(&b), vec!["from-a".to_owned(), "from-b".to_owned()]);
    assert_eq!(b.refresh_if_stale(), Ok(false));
}

#[test]
fn a_writers_memory_is_the_merged_file_after_its_own_save() {
    let tmp = TempDir::new().unwrap();
    let mut a = plan_engine(&tmp);
    let mut b = plan_engine(&tmp);
    add_task(&mut b, "from-b");
    // A writes from memory that predates B's task: the locked merge keeps
    // B's task on disk, and A adopts what landed rather than its stale copy.
    add_task(&mut a, "from-a");
    assert_eq!(task_ids(&a), vec!["from-a".to_owned(), "from-b".to_owned()]);
    assert_eq!(a.refresh_if_stale(), Ok(false));
}

#[test]
fn plan_changes_apply_to_fresh_state_without_overwriting_other_fields() {
    let tmp = TempDir::new().unwrap();
    let mut a = plan_engine(&tmp);
    add_task(&mut a, "c");
    let mut b = plan_engine(&tmp);
    assert_eq!(
        b.refresh_if_stale(),
        Ok(false),
        "opened on the current file"
    );

    // Both engines hold the current record. A starts the task; B, still
    // holding the pre-start copy, retitles it a moment later without
    // refreshing — the window between a read and a save.
    a.start_task("c").unwrap();
    sleep(Duration::from_millis(5));
    b.update_task("c", Some("B's title".into()), None, None, None, None, None)
        .unwrap();

    // The title edit applies to the current committed task and retains its status.
    let on_disk = plan_engine(&tmp);
    let task = &on_disk.plan().tasks[0];
    assert_eq!(task.title, "B's title");
    assert_eq!(task.status, TaskStatus::InProgress);
    assert_eq!(a.refresh_if_stale(), Ok(true));
    let seen = &a.plan().tasks[0];
    assert_eq!(
        (seen.title.as_str(), seen.status),
        ("B's title", TaskStatus::InProgress)
    );
    // A's next transition validates against the record it now holds.
    assert!(
        a.start_task("c").is_err(),
        "the started task cannot start twice"
    );
    assert_eq!(
        b.refresh_if_stale(),
        Ok(false),
        "a refused write changes no file"
    );
    assert_eq!(b.plan().tasks[0].status, TaskStatus::InProgress);
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
fn a_run_refresh_validates_the_store_and_reports_changes() {
    let tmp = TempDir::new().unwrap();
    let mut a = run_engine(&tmp);
    let mut b = run_engine(&tmp);
    assert_eq!(b.refresh_if_stale(), Ok(false));

    a.set_goal("cycle".into(), vec![], vec![], "task:c".into())
        .unwrap();
    a.add_step("t".into(), "step".into(), StepType::Implement, None, None)
        .unwrap();
    assert_eq!(b.refresh_if_stale(), Ok(true));
    assert_eq!(b.steps.len(), 1);
    assert_eq!(b.refresh_if_stale(), Ok(false));
    assert_eq!(a.refresh_if_stale(), Ok(false), "A wrote the file it holds");

    // B's write on the same cycle merges and B holds the merged state.
    b.start_step("t").unwrap();
    assert_eq!(b.refresh_if_stale(), Ok(false));
    assert_eq!(a.refresh_if_stale(), Ok(true));
    assert_eq!(a.active_step_id().as_deref(), Some("t"));
}

#[test]
fn engines_without_persistence_never_refresh() {
    let mut plan = PlanEngine::new("proj".into());
    let mut signal = SignalEngine::new("proj".into());
    let mut run = RunEngine::new("proj".into());
    assert_eq!(plan.refresh_if_stale(), Ok(false));
    assert_eq!(signal.refresh_if_stale(), Ok(false));
    assert_eq!(run.refresh_if_stale(), Ok(false));
}

#[test]
fn corrupt_cold_run_reads_never_confirm_empty_and_recover_without_reopening() {
    use magistr_workflow_contracts::ReadErrorKind;
    let tmp = TempDir::new().unwrap();
    let mut writer = run_engine(&tmp);
    writer
        .set_goal("kept".into(), vec![], vec![], "task:c".into())
        .unwrap();
    let persistence = run_persistence::Persistence::new(&run_persistence::PersistenceConfig {
        enabled: true,
        data_dir: tmp.path().to_owned(),
    });
    let path = persistence.path_for("proj");
    let good = std::fs::read(&path).unwrap();
    std::fs::write(&path, "{").unwrap();
    let mut cold = run_engine(&tmp);
    for _ in 0..2 {
        assert_eq!(cold.observe().unwrap_err().kind, ReadErrorKind::Decode);
        assert!(cold.refresh_if_stale().is_err());
        assert_eq!(std::fs::read(&path).unwrap(), b"{");
    }
    std::fs::write(&path, &good).unwrap();
    assert_eq!(
        cold.observe().unwrap().value.goal().unwrap().description,
        "kept"
    );
    assert!(cold.refresh_if_stale().unwrap());
    assert_eq!(cold.goal.as_ref().unwrap().description, "kept");
}

#[test]
fn each_family_retries_refused_reads_and_observes_deletion() {
    use magistr_workflow_contracts::{ReadErrorKind, ReadSource};
    let tmp = TempDir::new().unwrap();
    let mut plan = plan_engine(&tmp);
    add_task(&mut plan, "kept");
    let mut signal = signal_engine(&tmp);
    signal
        .capture(SignalKind::Idea, "author".into(), "kept".into())
        .unwrap();
    let mut run = run_engine(&tmp);
    run.set_goal("kept".into(), vec![], vec![], "task:kept".into())
        .unwrap();
    let paths = [
        Persistence::new(&infra_cfg(&tmp), Domain::Plan).path_for("proj"),
        Persistence::new(&infra_cfg(&tmp), Domain::Signal).path_for("proj"),
        run_persistence::Persistence::new(&run_persistence::PersistenceConfig {
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
        assert_eq!(plan.observe().unwrap_err().kind, ReadErrorKind::Decode);
        assert_eq!(signal.observe().unwrap_err().kind, ReadErrorKind::Decode);
        assert_eq!(run.observe().unwrap_err().kind, ReadErrorKind::Decode);
        assert!(plan.refresh_if_stale().is_err());
        assert!(signal.refresh_if_stale().is_err());
        assert!(run.refresh_if_stale().is_err());
    }
    assert_eq!(plan.plan().tasks.len(), 1);
    assert_eq!(signal.signals().signals.len(), 1);
    assert!(run.goal.is_some());
    for (path, bytes) in paths.iter().zip(good) {
        std::fs::write(path, bytes).unwrap();
    }
    assert_eq!(plan.observe().unwrap().value.tasks.len(), 1);
    assert_eq!(signal.observe().unwrap().value.signals.len(), 1);
    assert!(run.observe().unwrap().value.goal().is_some());
    for path in &paths {
        std::fs::remove_file(path).unwrap();
    }
    assert_eq!(plan.observe().unwrap().source, ReadSource::Missing);
    assert!(plan.observe().unwrap().value.tasks.is_empty());
    assert_eq!(signal.observe().unwrap().source, ReadSource::Missing);
    assert!(signal.observe().unwrap().value.signals.is_empty());
    assert_eq!(run.observe().unwrap().source, ReadSource::Missing);
    assert!(run.observe().unwrap().value.goal().is_none());
}

#[test]
fn run_observes_the_opened_execution_and_refuses_an_unsavable_mutation() {
    use magistr_workflow_contracts::ReadSource;
    let tmp = TempDir::new().unwrap();
    let config = run_persistence::PersistenceConfig {
        enabled: true,
        data_dir: tmp.path().to_owned(),
    };
    let a_store = run_persistence::Persistence::new(&config).for_execution("a");
    let b_store = run_persistence::Persistence::new(&config).for_execution("b");
    let a_path = a_store.path_for("proj");
    let mut a = RunEngine::new("proj".into()).with_persistence(a_store.clone());
    let a_reader = RunEngine::new("proj".into()).with_persistence(a_store);
    let mut b = RunEngine::new("proj".into()).with_persistence(b_store);
    a.set_goal("execution a".into(), vec![], vec![], "task:a".into())
        .unwrap();
    b.set_goal("execution b".into(), vec![], vec![], "task:b".into())
        .unwrap();
    assert_eq!(
        a_reader
            .observe()
            .unwrap()
            .value
            .goal()
            .unwrap()
            .description,
        "execution a"
    );
    assert_eq!(
        b.observe().unwrap().value.goal().unwrap().description,
        "execution b"
    );
    let lock = a_path.with_extension("json.lock");
    std::fs::remove_file(&lock).unwrap();
    std::fs::create_dir(&lock).unwrap();
    let refused = a
        .add_step(
            "unsaved".into(),
            "Refuse the unsavable step".into(),
            StepType::Implement,
            None,
            None,
        )
        .unwrap_err();
    assert_eq!(refused.effect(), "none");
    assert!(a.steps.is_empty());
    assert!(a.observe().unwrap().value.steps().is_empty());
    assert!(a.steps.is_empty());
    std::fs::remove_file(&a_path).unwrap();
    assert_eq!(a_reader.observe().unwrap().source, ReadSource::Missing);
    assert!(b.observe().unwrap().value.goal().is_some());
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
    let before = writer.observe_session(None).unwrap().map(Option::unwrap);
    let reader = ReasoningServer::new_for_project(config, "proj".into());
    let after = reader.observe_session(None).unwrap().map(Option::unwrap);
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
        reader.observe_session(None).unwrap_err().kind,
        ReadErrorKind::Identity
    );
    assert_eq!(std::fs::read(&path).unwrap(), bytes);
}

#[test]
fn reasoning_reads_complete_explicit_sessions_independently_of_writer_and_cache() {
    use magistr_workflow::think::{
        config::ThinkConfig, domain::ThinkStep, engine::ReasoningServer,
        persistence::Persistence as ThinkPersistence,
    };
    use magistr_workflow_contracts::{ReadErrorKind, ReadSource};
    for persisted in [false, true] {
        let tmp = TempDir::new().unwrap();
        let mut config = ThinkConfig {
            persistence: infra_cfg(&tmp),
            ..ThinkConfig::default()
        };
        config.persistence.enabled = persisted;
        config.display.color_output = false;
        config.features.enable_sessions = true;
        config.system.max_history_size = 2;
        config.system.session_timeout = 0;
        let mut writer = ReasoningServer::new_for_project(config.clone(), "proj".into());
        assert!(writer.observe_sessions().unwrap().value.sessions.is_empty());
        assert!(writer.observe_session(None).unwrap().value.is_none());
        assert!(writer.observe_session(Some(" ")).is_err());
        assert!(
            writer
                .observe_session(Some("missing"))
                .unwrap()
                .value
                .is_none()
        );
        let step = |number, session: Option<&str>| -> ThinkStep {
            serde_json::from_value(serde_json::json!({
                "step_number": number, "estimated_total": 20, "purpose": "analysis",
                "context": "session scope", "thought": format!("saved evidence {number}"),
                "outcome": "observed", "next_action": "continue", "rationale": "preserve scope",
                "session_id": session,
            }))
            .unwrap()
        };
        writer.process_step_native(step(1, None)).unwrap();
        for number in 2..=7 {
            writer
                .process_step_native(step(number, Some("chosen")))
                .unwrap();
        }
        let mut branch = step(8, Some("chosen"));
        branch.branch_from = Some(2);
        branch.branch_name = Some("Saved investigation".into());
        writer.process_step_native(branch).unwrap();
        let chosen = writer
            .observe_session(Some("chosen"))
            .unwrap()
            .value
            .unwrap();
        assert_eq!(chosen.steps.len(), 7);
        assert_eq!(
            chosen.branches.as_ref().unwrap()[0].name,
            "Saved investigation"
        );
        writer.process_step_native(step(9, Some("other"))).unwrap();
        writer.cleanup_expired_sessions(true);
        assert_eq!(
            writer
                .observe_session(Some("chosen"))
                .unwrap()
                .value
                .as_ref(),
            Some(&chosen)
        );
        assert_eq!(
            writer.observe_session(None).unwrap().value.unwrap().steps[0].step_number,
            1
        );
        assert_eq!(
            writer.observe_step(2, false).unwrap().value.unwrap(),
            chosen.steps[0]
        );
        let all = writer.observe_sessions().unwrap();
        assert_eq!(all.value.sessions.len(), 3);
        assert!(
            writer
                .observe_session(Some("not-recorded"))
                .unwrap()
                .value
                .is_none()
        );
        if !persisted {
            assert_eq!(all.source, ReadSource::Memory);
            continue;
        }
        let detached = ReasoningServer::new_for_project(config.clone(), "proj".into());
        assert_eq!(detached.observe_sessions().unwrap(), all);
        let store = ThinkPersistence::for_project(&config.persistence, "proj");
        let path = store.store_path().unwrap();
        let bytes = std::fs::read(&path).unwrap();
        assert_eq!(
            detached
                .observe_session(Some("chosen"))
                .unwrap()
                .value
                .as_ref(),
            Some(&chosen)
        );
        assert_eq!(std::fs::read(&path).unwrap(), bytes, "reads never write");
        let mut invalid: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        invalid["histories"]["proj__other"]["session_id"] = serde_json::json!("proj__chosen");
        let corrupted = serde_json::to_vec(&invalid).unwrap();
        std::fs::write(&path, &corrupted).unwrap();
        assert_eq!(
            writer.observe_sessions().unwrap_err().kind,
            ReadErrorKind::Identity
        );
        assert!(detached.observe_session(Some("chosen")).is_err());
        assert!(detached.observe_step(2, false).is_err());
        assert!(detached.observe_session(Some("missing")).is_err());
        assert_eq!(std::fs::read(&path).unwrap(), corrupted);
        std::fs::write(&path, &bytes).unwrap();
        assert_eq!(
            detached
                .observe_session(Some("chosen"))
                .unwrap()
                .value
                .unwrap(),
            chosen
        );
    }
}

#[test]
fn refresh_revalidates_bytes_even_when_the_public_stamp_is_unchanged() {
    use magistr_workflow_contracts::{ReadErrorKind, ReadSource};
    let tmp = TempDir::new().unwrap();
    let mut owner = plan_engine(&tmp);
    add_task(&mut owner, "saved");
    let path = Persistence::new(&infra_cfg(&tmp), Domain::Plan).path_for("proj");
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
    assert_eq!(owner.plan().tasks[0].id, "saved");
}
