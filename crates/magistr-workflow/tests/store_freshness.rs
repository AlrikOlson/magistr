//! Validated family observations distinguish durable data, absence and read failure.

use std::thread::sleep;
use std::time::Duration;

use tempfile::TempDir;

use magistr_workflow::infra::{Domain, Persistence, PersistenceConfig};
use magistr_workflow::plan::PlanEngine;
use magistr_workflow::plan::domain::TaskStatus;
use magistr_workflow::request::RequestEngine;
use magistr_workflow::request::domain::{RequestKind, RequestStatus};
use magistr_workflow::run::domain::step::StepType;
use magistr_workflow::run::engine::RunEngine;
use magistr_workflow::run::persistence as run_persistence;

fn infra_cfg(tmp: &TempDir) -> PersistenceConfig {
    PersistenceConfig::from_env()
        .with_data_dir(tmp.path().to_path_buf())
        .enabled(true)
}

fn plan_engine(tmp: &TempDir) -> PlanEngine {
    PlanEngine::new("proj".into()).with_persistence(Persistence::new(&infra_cfg(tmp), Domain::Plan))
}

fn request_engine(tmp: &TempDir) -> RequestEngine {
    RequestEngine::new("proj".into())
        .with_persistence(Persistence::new(&infra_cfg(tmp), Domain::Request))
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
fn a_conflicting_request_write_resolves_by_lifecycle_rank_not_by_who_wrote_last() {
    let tmp = TempDir::new().unwrap();
    let mut a = request_engine(&tmp);
    let id = a
        .capture(RequestKind::Suggestion, "alice".into(), "rank".into())
        .as_ref()
        .unwrap()
        .value
        .id
        .clone();
    let mut b = request_engine(&tmp);
    assert_eq!(b.refresh_if_stale(), Ok(false));

    // A researches; B, from its pre-research copy, snoozes — a write that
    // carries the older lifecycle state.
    a.research(&id, "worth it".into(), 0.9, vec![], None)
        .unwrap();
    b.snooze(&id, 5).unwrap();

    // Declared outcome: the further lifecycle rank wins the record, so the
    // research survives B's later write, and B holds the merged record.
    let seen = b.get(&id).unwrap();
    assert_eq!(seen.status, RequestStatus::Researched);
    assert_eq!(seen.enrichment.len(), 1);
    assert_eq!(b.refresh_if_stale(), Ok(false));
    assert_eq!(a.refresh_if_stale(), Ok(true));
    assert_eq!(a.get(&id).unwrap().status, RequestStatus::Researched);
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
    let mut request = RequestEngine::new("proj".into());
    let mut run = RunEngine::new("proj".into());
    assert_eq!(plan.refresh_if_stale(), Ok(false));
    assert_eq!(request.refresh_if_stale(), Ok(false));
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
    let mut request = request_engine(&tmp);
    request
        .capture(RequestKind::Suggestion, "author".into(), "kept".into())
        .unwrap();
    let mut run = run_engine(&tmp);
    run.set_goal("kept".into(), vec![], vec![], "task:kept".into())
        .unwrap();
    let paths = [
        Persistence::new(&infra_cfg(&tmp), Domain::Plan).path_for("proj"),
        Persistence::new(&infra_cfg(&tmp), Domain::Request).path_for("proj"),
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
        assert_eq!(request.observe().unwrap_err().kind, ReadErrorKind::Decode);
        assert_eq!(run.observe().unwrap_err().kind, ReadErrorKind::Decode);
        assert!(plan.refresh_if_stale().is_err());
        assert!(request.refresh_if_stale().is_err());
        assert!(run.refresh_if_stale().is_err());
    }
    assert_eq!(plan.plan().tasks.len(), 1);
    assert_eq!(request.requests().requests.len(), 1);
    assert!(run.goal.is_some());
    for (path, bytes) in paths.iter().zip(good) {
        std::fs::write(path, bytes).unwrap();
    }
    assert_eq!(plan.observe().unwrap().value.tasks.len(), 1);
    assert_eq!(request.observe().unwrap().value.requests.len(), 1);
    assert!(run.observe().unwrap().value.goal().is_some());
    for path in &paths {
        std::fs::remove_file(path).unwrap();
    }
    assert_eq!(plan.observe().unwrap().source, ReadSource::Missing);
    assert!(plan.observe().unwrap().value.tasks.is_empty());
    assert_eq!(request.observe().unwrap().source, ReadSource::Missing);
    assert!(request.observe().unwrap().value.requests.is_empty());
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
fn request_failure_keeps_the_last_committed_memory_and_observation() {
    let tmp = TempDir::new().unwrap();
    let mut request = request_engine(&tmp);
    request
        .capture(RequestKind::Suggestion, "author".into(), "saved".into())
        .unwrap();
    let path = Persistence::new(&infra_cfg(&tmp), Domain::Request).path_for("proj");
    let lock = path.with_extension("json.lock");
    std::fs::remove_file(&lock).unwrap();
    std::fs::create_dir(&lock).unwrap();
    let refused = request
        .capture(RequestKind::Suggestion, "author".into(), "unsaved".into())
        .unwrap_err();
    assert!(
        matches!(refused, magistr_workflow::request::engine::RequestMutationError::Store(ref error) if error.effect() == "none")
    );
    assert_eq!(request.requests().requests.len(), 1);
    let read = request.observe().unwrap();
    assert_eq!(read.value.requests.len(), 1);
    assert_eq!(read.value.requests[0].body, "saved");
    assert_eq!(request.requests().requests.len(), 1);
}

#[test]
fn reasoning_reads_preserve_authored_ids_numbers_and_file_bytes() {
    use magistr_workflow::decision::{
        config::DecisionConfig, domain::Note, engine::ReasoningServer,
    };
    use magistr_workflow_contracts::ReadErrorKind;
    let tmp = TempDir::new().unwrap();
    let mut config = DecisionConfig {
        persistence: infra_cfg(&tmp),
        ..DecisionConfig::default()
    };
    config.display.color_output = false;
    let mut writer = ReasoningServer::new_for_project(config.clone(), "proj".into());
    for number in [7, 9] {
        let note: Note = serde_json::from_value(serde_json::json!({
            "note": number, "estimated_total": 10, "purpose": "check",
            "context": "read integrity", "thought": format!("note {number}"),
            "outcome": "observed", "next_action": "continue", "rationale": "preserve identity"
        }))
        .unwrap();
        assert!(writer.process_note_native(note).is_ok());
    }
    let persistence = magistr_workflow::decision::persistence::Persistence::for_project(
        &config.persistence,
        "proj",
    );
    let path = persistence.store_path().unwrap();
    let original = std::fs::read(&path).unwrap();
    let before = writer.observe_log(None).unwrap().map(Option::unwrap);
    let reader = ReasoningServer::new_for_project(config, "proj".into());
    let after = reader.observe_log(None).unwrap().map(Option::unwrap);
    assert_eq!(
        after.value.notes.iter().map(|s| s.note).collect::<Vec<_>>(),
        vec![7, 9]
    );
    assert_eq!(
        after
            .value
            .notes
            .iter()
            .map(|s| &s.record_id)
            .collect::<Vec<_>>(),
        before
            .value
            .notes
            .iter()
            .map(|s| &s.record_id)
            .collect::<Vec<_>>()
    );
    assert_eq!(std::fs::read(&path).unwrap(), original);
    let mut invalid: serde_json::Value = serde_json::from_slice(&original).unwrap();
    invalid["logs"]["proj"]["notes"][1]["note"] = serde_json::json!(7);
    let bytes = serde_json::to_vec(&invalid).unwrap();
    std::fs::write(&path, &bytes).unwrap();
    assert_eq!(
        reader.observe_log(None).unwrap_err().kind,
        ReadErrorKind::Identity
    );
    assert_eq!(std::fs::read(&path).unwrap(), bytes);
}

#[test]
fn reasoning_reads_complete_explicit_logs_independently_of_writer_and_cache() {
    use magistr_workflow::decision::{
        config::DecisionConfig, domain::Note, engine::ReasoningServer,
        persistence::Persistence as DecisionPersistence,
    };
    use magistr_workflow_contracts::{ReadErrorKind, ReadSource};
    for persisted in [false, true] {
        let tmp = TempDir::new().unwrap();
        let mut config = DecisionConfig {
            persistence: infra_cfg(&tmp),
            ..DecisionConfig::default()
        };
        config.persistence.enabled = persisted;
        config.display.color_output = false;
        config.features.enable_logs = true;
        config.system.max_history_size = 2;
        config.system.log_timeout = 0;
        let mut writer = ReasoningServer::new_for_project(config.clone(), "proj".into());
        assert!(writer.observe_logs().unwrap().value.logs.is_empty());
        assert!(writer.observe_log(None).unwrap().value.is_none());
        assert!(writer.observe_log(Some(" ")).is_err());
        assert!(writer.observe_log(Some("missing")).unwrap().value.is_none());
        let note = |number, log: Option<&str>| -> Note {
            serde_json::from_value(serde_json::json!({
                "note": number, "estimated_total": 20, "purpose": "analysis",
                "context": "log scope", "thought": format!("saved evidence {number}"),
                "outcome": "observed", "next_action": "continue", "rationale": "preserve scope",
                "log_id": log,
            }))
            .unwrap()
        };
        writer.process_note_native(note(1, None)).unwrap();
        for number in 2..=7 {
            writer
                .process_note_native(note(number, Some("chosen")))
                .unwrap();
        }
        let mut thread = note(8, Some("chosen"));
        thread.thread_from = Some(2);
        thread.thread_name = Some("Saved investigation".into());
        writer.process_note_native(thread).unwrap();
        let chosen = writer.observe_log(Some("chosen")).unwrap().value.unwrap();
        assert_eq!(chosen.notes.len(), 7);
        assert_eq!(
            chosen.threads.as_ref().unwrap()[0].name,
            "Saved investigation"
        );
        writer.process_note_native(note(9, Some("other"))).unwrap();
        writer.cleanup_expired_logs(true);
        assert_eq!(
            writer.observe_log(Some("chosen")).unwrap().value.as_ref(),
            Some(&chosen)
        );
        assert_eq!(
            writer.observe_log(None).unwrap().value.unwrap().notes[0].note,
            1
        );
        assert_eq!(
            writer.observe_note(2, false).unwrap().value.unwrap(),
            chosen.notes[0]
        );
        let all = writer.observe_logs().unwrap();
        assert_eq!(all.value.logs.len(), 3);
        assert!(
            writer
                .observe_log(Some("not-recorded"))
                .unwrap()
                .value
                .is_none()
        );
        if !persisted {
            assert_eq!(all.source, ReadSource::Memory);
            continue;
        }
        let detached = ReasoningServer::new_for_project(config.clone(), "proj".into());
        assert_eq!(detached.observe_logs().unwrap(), all);
        let store = DecisionPersistence::for_project(&config.persistence, "proj");
        let path = store.store_path().unwrap();
        let bytes = std::fs::read(&path).unwrap();
        assert_eq!(
            detached.observe_log(Some("chosen")).unwrap().value.as_ref(),
            Some(&chosen)
        );
        assert_eq!(std::fs::read(&path).unwrap(), bytes, "reads never write");
        let mut invalid: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        invalid["logs"]["proj__other"]["log_id"] = serde_json::json!("proj__chosen");
        let corrupted = serde_json::to_vec(&invalid).unwrap();
        std::fs::write(&path, &corrupted).unwrap();
        assert_eq!(
            writer.observe_logs().unwrap_err().kind,
            ReadErrorKind::Identity
        );
        assert!(detached.observe_log(Some("chosen")).is_err());
        assert!(detached.observe_note(2, false).is_err());
        assert!(detached.observe_log(Some("missing")).is_err());
        assert_eq!(std::fs::read(&path).unwrap(), corrupted);
        std::fs::write(&path, &bytes).unwrap();
        assert_eq!(
            detached.observe_log(Some("chosen")).unwrap().value.unwrap(),
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
