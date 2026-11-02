use super::*;
use crate::native::{
    EvidencePort, GateContract, NativeConfig, NativeReader, NativeSnapshot, ProjectAuthority,
    SourceFreshness, SourceObservation, SourceWitness, WriterRole,
};
use magistr_workflow::decision::domain::Note;
use magistr_workflow::plan::domain::{TaskStatus, WaitingReason};
use magistr_workflow::request::domain::RequestKind;
use magistr_workflow::run::domain::check::{Check, CheckType};
use magistr_workflow::run::domain::progress::ProgressType;
use magistr_workflow::run::domain::step::StepType;

/// A source that never moves and a project that configures no gates: the
/// evidence capability every writer here is admitted with, so a recorded
/// gate certifies a source and a task can complete on it.
struct Fixed;

impl SourceWitness for Fixed {
    fn observation(&self) -> SourceObservation {
        SourceObservation {
            inputs: Vec::new(),
            execution_digest: String::new(),
            workspace_root: "/w".into(),
            workspace_generation: None,
            scheme: "fake/1".into(),
            policy_digest: "p".into(),
            revision: "r1".into(),
            source_digest: "r1".into(),
            config_digest: "c".into(),
            consistency: "optimistic".into(),
            captured_at_ms: 0,
        }
    }
    fn validate(&self) -> Result<SourceFreshness, String> {
        Ok(SourceFreshness::Unchanged)
    }
}

struct Evidence;

impl EvidencePort for Evidence {
    fn observe(&self) -> Result<Box<dyn SourceWitness>, String> {
        Ok(Box::new(Fixed))
    }
    fn required_gates(&self) -> Result<Vec<GateContract>, String> {
        Ok(Vec::new())
    }
}

fn temp_config(tmp: &tempfile::TempDir) -> NativeConfig {
    std::fs::create_dir_all(tmp.path().join("proj")).expect("project");
    NativeConfig::from_lookup(tmp.path().join("proj"), |_| None)
        .with_data_dir(tmp.path().join("data"))
        .with_evidence(Arc::new(Evidence))
}

/// The host's admitted writer on `cfg`.
fn host(cfg: &NativeConfig, project_id: &str) -> NativeEngines {
    ProjectAuthority::admit_for_project(cfg, project_id.into(), WriterRole::Host)
        .expect("host admitted")
        .open()
}

/// The agent's `magistr mcp` on the same store: the other admitted
/// writer role, holding its own cached engines.
fn service(cfg: &NativeConfig, project_id: &str) -> NativeEngines {
    ProjectAuthority::admit_for_project(cfg, project_id.into(), WriterRole::Service)
        .expect("service admitted")
        .open()
}

/// An observer on the store, admitted by nobody.
fn reader(cfg: &NativeConfig, project_id: &str) -> NativeReader {
    NativeReader::open_for_project(cfg, project_id.into())
}

fn add_task(engines: &NativeEngines, id: &str, priority: u32, deps: &[&str]) {
    engines
        .plan_add_task(
            id.into(),
            format!("Task {id}"),
            String::new(),
            TaskStatus::ToDo,
            priority,
            String::new(),
            vec![],
            deps.iter().map(|d| (*d).to_string()).collect(),
            None,
            None,
        )
        .expect("add task");
}

fn note(n: u32, purpose: &str) -> Note {
    serde_json::from_value(serde_json::json!({
        "note": n,
        "estimated_total": 3,
        "purpose": purpose,
        "context": "in-process test",
        "thought": "Reasoning body.",
        "outcome": "Recorded.",
        "next_action": "Continue.",
        "rationale": "Because the test says so.",
    }))
    .expect("note")
}

#[test]
fn compaction_owner_command_preserves_check_results_for_current_and_retained_runs() {
    use crate::native::{OwnerCommand, OwnerReply, RunCommand};
    let temp = tempfile::tempdir().unwrap();
    let cfg = temp_config(&temp);
    let engines = host(&cfg, "compaction");
    assert!(engines.run_compact_checks().is_err());
    let mut old_id = String::new();
    for n in 0..2 {
        engines
            .run_open(format!("Run {n}"), vec![], vec![], String::new())
            .unwrap();
        engines
            .add_step("verify".into(), "Verify".into(), StepType::Test, None)
            .unwrap();
        engines.start_step("verify").unwrap();
        let admission = engines
            .run_admit_gate(None, "test", "true".into(), true)
            .unwrap();
        let check_result_id = admission.value.check_result_id.clone();
        engines
            .run_settle_gate(
                admission.value,
                CheckExecution::Exited { code: 0 },
                String::new(),
                None,
            )
            .unwrap();
        if n == 0 {
            old_id = check_result_id;
            engines.finish_step("verify", vec![]).unwrap();
            engines.run_finish(vec![], None).unwrap();
        }
    }
    assert!(engines.run_check_result(&old_id).unwrap().is_some());
    let command: OwnerCommand = serde_json::from_value(
        serde_json::to_value(OwnerCommand::Run(RunCommand::CompactCheckResults)).unwrap(),
    )
    .unwrap();
    let OwnerReply::Report(report) = engines.apply(command).unwrap() else {
        panic!("compaction report")
    };
    assert_eq!(report["removed"], 0);
    assert_eq!(report["retained_current"], 1);
    assert_eq!(report["retained_referenced"], 1);
    assert!(engines.run_check_result(&old_id).unwrap().is_some());
}

#[test]
fn thread_lifecycle_and_exact_merge_destination_survive_reader_restart() {
    use crate::native::ThreadStatus;

    let tmp = tempfile::tempdir().expect("tempdir");
    let cfg = temp_config(&tmp);
    let project = "reasoning-restart";
    let observer = reader(&cfg, project);
    assert!(
        observer
            .snapshot()
            .decisions
            .current()
            .unwrap()
            .value
            .logs
            .is_empty()
    );
    {
        let engines = host(&cfg, project);
        engines
            .decision_record(note(1, "Keep recovery isolated"))
            .expect("origin");
        let mut thread = note(2, "Can each tenant keep its own check_result?");
        thread.thread_id = Some("check_result".into());
        thread.thread_name = Some("Can each tenant keep its own check_result?".into());
        thread.thread_from = Some(1);
        thread.outcome = "Keep one check_result per tenant.".into();
        engines.decision_record(thread).expect("thread");
        engines
            .decision_record(note(3, "Check the log format"))
            .expect("unrelated note");
        engines
            .decision_record(note(4, "Use tenant check_results during recovery"))
            .expect("synthesis");
        engines
            .services
            .decision
            .set_thread_status("check_result", "merged", Some(4))
            .expect("merge");
        let snapshot = observer.snapshot();
        let check_result = snapshot
            .decisions
            .current()
            .unwrap()
            .value
            .log(None)
            .unwrap()
            .threads
            .iter()
            .flatten()
            .find(|thread| thread.id == "check_result")
            .expect("persisted thread");
        assert_eq!(
            check_result.name,
            "Can each tenant keep its own check_result?"
        );
        assert_eq!(check_result.from_note, 1);
        assert_eq!(check_result.status, ThreadStatus::Merged);
        assert_eq!(check_result.merged_into, Some(4));
        assert_eq!(
            snapshot
                .decisions
                .current()
                .unwrap()
                .value
                .log(None)
                .unwrap()
                .notes[1]
                .thread_id
                .as_deref(),
            Some("check_result")
        );
        assert_eq!(
            snapshot
                .decisions
                .current()
                .unwrap()
                .value
                .log(None)
                .unwrap()
                .notes[1]
                .outcome,
            "Keep one check_result per tenant."
        );
    }

    let restarted = reader(&cfg, project);
    let persisted = restarted.snapshot();
    let check_result = persisted
        .decisions
        .current()
        .unwrap()
        .value
        .log(None)
        .unwrap()
        .threads
        .iter()
        .flatten()
        .find(|thread| thread.id == "check_result")
        .expect("thread after restart");
    assert_eq!(check_result.status, ThreadStatus::Merged);
    assert_eq!(
        check_result.merged_into,
        Some(4),
        "do not substitute the first later main-line note"
    );

    {
        let engines = host(&cfg, project);
        engines
            .services
            .decision
            .set_thread_status("check_result", "abandoned", None)
            .expect("stop investigation after restart");
    }
    let stopped = restarted.snapshot();
    let check_result = stopped
        .decisions
        .current()
        .unwrap()
        .value
        .log(None)
        .unwrap()
        .threads
        .iter()
        .flatten()
        .find(|thread| thread.id == "check_result")
        .expect("stopped thread");
    assert_eq!(check_result.status, ThreadStatus::Abandoned);
    assert_eq!(
        check_result.merged_into, None,
        "reseed clears obsolete merge provenance"
    );
}

#[test]
fn round_trip_through_a_temp_store() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let cfg = temp_config(&tmp);

    {
        let engines = host(&cfg, "round-trip");
        assert_eq!(engines.project_id(), "round-trip");
        add_task(&engines, "base", 100, &[]);
        add_task(&engines, "next", 200, &["base"]);
        let started = engines.plan_start_task("base").expect("start");
        assert_eq!(started.value.status, TaskStatus::InProgress);

        assert!(engines.decision_record(note(1, "open")).is_ok());
        // Returns the prior pinned state; a fresh step was not pinned.
        assert!(!engines.decision_pin(1, true).unwrap().value);

        assert_eq!(
            engines
                .run_open("obj".into(), vec![], vec![], "task:base".into())
                .expect("goal")
                .value,
            0
        );
        engines
            .add_step("t1".into(), "Step one".into(), StepType::Implement, None)
            .expect("add step");
        let t = engines.start_step("t1").expect("start step");
        assert_eq!(t.value.id, "t1");
        let progress = engines
            .record_progress(
                None,
                ProgressType::Code,
                "wrote code".into(),
                vec!["a.rs".into()],
                vec![],
                Some(1),
            )
            .expect("progress");
        assert_eq!(progress.value.note, Some(1));
        let check = engines
            .run_record_gate(
                None,
                "test",
                "cargo nextest".into(),
                0,
                String::new(),
                true,
                None,
            )
            .expect("gate");
        assert!(check.value.passed && check.value.verified);
        assert_eq!(check.value.exit_code, Some(0));
        let failed = engines
            .run_record_gate(None, "lint", "clippy".into(), 1, String::new(), true, None)
            .expect("gate");
        assert!(!failed.value.passed && failed.value.verified);
        // The red lint is current evidence against completion until it
        // is re-run green; then it is a recovered failure, not an
        // outstanding one.
        let refused = engines
            .plan_complete_task("base", Some("step:t1"))
            .expect_err("a failed required gate refuses completion");
        assert!(
            refused
                .to_string()
                .contains("lint: the latest attempt did not pass (exit 1)"),
            "{refused}"
        );
        engines
            .run_record_gate(None, "lint", "clippy".into(), 0, String::new(), true, None)
            .expect("gate");
        engines.finish_step("t1", vec![]).expect("complete");

        let sig = engines
            .request_capture(
                RequestKind::Suggestion,
                "alice".into(),
                "make it faster".into(),
            )
            .expect("capture");
        assert_eq!(sig.value.body, "make it faster");
        let done = engines
            .plan_complete_task("base", Some("step:t1"))
            .expect("complete task");
        assert_eq!(done.value.status, TaskStatus::Done);
        let report = engines
            .run_finish(vec![], Some("done".into()))
            .expect("finalize");
        assert_eq!(report.value.status, "finished");
        assert_eq!(report.value.recovered_failures, vec!["lint (step: t1)"]);
    }

    // A second process on the same store sees everything.
    let reopened = reader(&cfg, "round-trip");
    let snap = reopened.plan_snapshot().unwrap();
    assert_eq!(snap.count("done"), 1);
    assert_eq!(snap.next_ready.as_deref(), Some("next"));
    assert_eq!(reopened.plan_progress().unwrap().checked_tasks, 1);
    assert_eq!(
        reopened.plan_task("base").unwrap().map(|c| c.refs),
        Some(vec!["step:t1".to_string()])
    );
    let pinned = reopened
        .decision_log(None)
        .unwrap()
        .unwrap()
        .notes
        .iter()
        .filter(|note| note.pinned == Some(true))
        .count();
    assert_eq!(pinned, 1, "decision log persisted with its pin");
    assert_eq!(
        reopened.run_goal().unwrap().unwrap().status,
        crate::native::GoalStatus::Finished
    );
    assert_eq!(reopened.request_status().unwrap()["counts"]["new"], 1);

    // Every family wrote under its own partition of the one data dir.
    for family in ["plan", "decisions", "run", "requests"] {
        let base = tmp.path().join("data").join(family);
        let base = if family == "run" {
            base.join("executions").join(cfg.execution_context().key())
        } else {
            base
        };
        let dir = base.join("sessions");
        assert!(
            dir.join("round-trip.json").exists(),
            "{family} store missing"
        );
    }
}

#[test]
fn two_writers_merge_without_data_loss() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let cfg = temp_config(&tmp);

    // A = Magistr in-process, B = an external magistr server, both
    // holding the same (empty) store in memory.
    let a = host(&cfg, "shared");
    let b = a.clone();

    add_task(&a, "from-a", 100, &[]);
    add_task(&b, "from-b", 200, &[]);
    // B wrote last from stale memory (it never saw from-a). A plain
    // overwrite would have erased from-a; the locked merge keeps both.
    let c = reader(&cfg, "shared");
    let ids: Vec<String> = c.plan_tasks().unwrap().into_iter().map(|c| c.id).collect();
    assert!(
        ids.contains(&"from-a".to_string()),
        "A's task survived: {ids:?}"
    );
    assert!(
        ids.contains(&"from-b".to_string()),
        "B's task survived: {ids:?}"
    );

    // The merge lands on disk and in the writer's memory: B's own save
    // handed it the merged file, so B answers A's task without a reload,
    // and A's next query observes B's task the same way.
    assert!(
        b.plan_task("from-a").unwrap().is_some(),
        "B's memory is the merged file after its own write"
    );
    assert!(
        a.plan_task("from-b").unwrap().is_some(),
        "A's query re-read the moved store"
    );
    b.plan_start_task("from-b").expect("start");
    assert_eq!(
        a.plan_task("from-b").unwrap().map(|c| c.status),
        Some(TaskStatus::InProgress),
        "A observes B's transition on its next query"
    );
    let d = reader(&cfg, "shared");
    assert_eq!(d.plan_snapshot().unwrap().count("in_progress"), 1);
    assert_eq!(d.plan_snapshot().unwrap().count("to_do"), 1);
    // A snapshot read on the writer that never reloaded is the current
    // store too, family by family.
    let snapshot = a.snapshot();
    assert_eq!(snapshot.plan.current().unwrap().value.tasks.len(), 2);
}

#[test]
fn open_resolves_identity_from_the_project_root() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let root = tmp.path().join("ident");
    std::fs::create_dir_all(&root).expect("mkdir");
    let cfg = NativeConfig::from_lookup(&root, |_| None).with_data_dir(tmp.path().join("data"));
    let declared =
        magistr_workflow::infra::project_id_for_path(&root.canonicalize().expect("canonical"));
    std::fs::create_dir_all(root.join(".magistr")).unwrap();
    std::fs::write(
        root.join(".magistr/project.json"),
        serde_json::json!({"project":{"id":&declared,"name":"Example"}}).to_string(),
    )
    .unwrap();
    let engines = ProjectAuthority::admit(&cfg, WriterRole::Host)
        .expect("admitted")
        .open();
    assert_eq!(engines.project_id(), declared);
    assert_eq!(engines.authority().project_id(), declared);
    assert_eq!(NativeReader::open(&cfg).project_id(), declared);
}

#[test]
fn persistence_off_writes_nothing() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let cfg = temp_config(&tmp).persist(false);
    let engines = host(&cfg, "ephemeral");
    add_task(&engines, "a", 1, &[]);
    assert!(!tmp.path().join("data").join("plan").exists());
    let again = reader(&cfg, "ephemeral");
    assert!(again.plan_tasks().unwrap().is_empty());
}

#[test]
fn plan_mutation_wrappers_round_trip() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let cfg = temp_config(&tmp);
    let engines = host(&cfg, "mutations");

    let added = engines
        .plan_add_task(
            "auth".into(),
            "Authenticate users".into(),
            String::new(),
            TaskStatus::ToDo,
            200,
            "desc".into(),
            vec!["logs in".into()],
            vec![],
            None,
            Some("Identity".into()),
        )
        .expect("add");
    assert_eq!(added.value.feature.as_deref(), Some("Identity"));
    assert!(!added.value.name.is_empty(), "name is derived from the id");

    let updated = engines
        .plan_update_task(
            "auth",
            Some("Authenticate every user".into()),
            Some(100),
            None,
            Some(vec!["logs in".into(), "logs out".into()]),
            None,
            None,
        )
        .expect("update");
    assert_eq!(updated.value.title, "Authenticate every user");
    assert_eq!(updated.value.priority, 100);
    assert_eq!(updated.value.acceptance.len(), 2);

    let proposed = engines
        .plan_reprioritize("auth", 300, "less urgent".into())
        .expect("propose");
    assert_eq!(proposed.value.priority, 100, "a proposal never reorders");
    assert_eq!(
        proposed
            .value
            .reprioritize
            .as_ref()
            .map(|p| p.suggested_priority),
        Some(300)
    );

    let linked = engines.plan_link("auth", "decision:7").expect("link");
    assert!(linked.value.refs.contains(&"decision:7".to_string()));

    let regrouped = engines
        .plan_set_feature("auth", Some("Accounts".into()))
        .expect("set feature");
    assert_eq!(regrouped.value.feature.as_deref(), Some("Accounts"));

    let idea = engines
        .plan_set_status("auth", TaskStatus::Idea)
        .expect("set status");
    assert_eq!(idea.value.status, TaskStatus::Idea);
    let refused = engines
        .plan_set_status("auth", TaskStatus::Done)
        .expect_err("done is never set by name");
    assert!(
        refused.to_string().contains("plan_complete_task"),
        "{refused}"
    );

    let gone = engines
        .plan_retire_task("auth", "superseded".into())
        .expect("obsolete");
    assert_eq!(gone.value.status, TaskStatus::Retired);
    assert_eq!(gone.value.retired_reason.as_deref(), Some("superseded"));

    assert!(engines.plan_link("missing", "decision:1").is_err());
}

#[test]
fn repeated_validated_reads_pick_up_another_writers_changes() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let cfg = temp_config(&tmp);
    let host = reader(&cfg, "reload");
    let server = service(&cfg, "reload");
    assert_eq!(host.config(), &cfg);

    add_task(&server, "only-on-disk", 100, &[]);
    server
        .run_open("obj".into(), vec![], vec![], "task:only-on-disk".into())
        .expect("goal");
    server
        .add_step("t1".into(), "step".into(), StepType::Implement, None)
        .expect("add step");

    // The host opened before those writes; every query observes durable truth.
    assert_eq!(
        host.plan_next().unwrap().map(|c| c.id).as_deref(),
        Some("only-on-disk")
    );
    assert_eq!(host.run_snapshot().unwrap().steps().len(), 1);

    assert_eq!(
        host.plan_next().unwrap().map(|c| c.id).as_deref(),
        Some("only-on-disk")
    );
    assert_eq!(host.run_snapshot().unwrap().steps().len(), 1);
    assert_eq!(
        host.run_snapshot().unwrap().goal().unwrap().scope,
        "task:only-on-disk"
    );
}

#[test]
fn decision_log_sees_another_writers_notes_without_reopening() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let cfg = temp_config(&tmp);
    let host = reader(&cfg, "decision-store");
    assert!(host.decision_log(None).unwrap().is_none());

    // The agent's server records after the host opened.
    let server = service(&cfg, "decision-store");
    assert!(server.decision_record(note(1, "open")).is_ok());
    assert!(server.decision_record(note(2, "ground")).is_ok());

    let numbers: Vec<u32> = host
        .decision_log(None)
        .unwrap()
        .unwrap()
        .notes
        .iter()
        .map(|s| s.note)
        .collect();
    assert_eq!(numbers, vec![1, 2]);
}

#[test]
fn reasoning_observation_uses_live_memory_when_persistence_is_disabled() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let cfg = temp_config(&tmp).persist(false);
    let engines = host(&cfg, "ephemeral-decision");
    assert!(engines.decision_record(note(1, "open")).is_ok());
    assert_eq!(engines.decision_log(None).unwrap().unwrap().notes.len(), 1);
    assert_eq!(
        engines.snapshot().decisions.current().unwrap().source,
        crate::native::ReadSource::Memory
    );
}

#[test]
fn request_triage_wrappers_round_trip() {
    use magistr_workflow::request::domain::RequestStatus;
    let tmp = tempfile::tempdir().expect("tempdir");
    let cfg = temp_config(&tmp);
    let engines = host(&cfg, "triage");

    let a = engines
        .request_capture(RequestKind::Suggestion, "alice".into(), "faster".into())
        .expect("capture");
    let b = engines
        .request_capture(RequestKind::Bug, "bob".into(), "broken".into())
        .expect("capture");
    let c = engines
        .request_capture(RequestKind::Question, "cara".into(), "why".into())
        .expect("capture");
    let ids: Vec<String> = engines
        .request_list()
        .unwrap()
        .into_iter()
        .map(|s| s.id)
        .collect();
    assert!(ids.contains(&a.value.id) && ids.contains(&b.value.id) && ids.contains(&c.value.id));

    // Promote needs a researched request; a fresh one is refused with a reason.
    assert!(engines.request_promote(&a.value.id).is_err());
    let researched = engines
        .request_research(&a.value.id, "validated".into(), 0.9, vec![], None)
        .expect("research");
    assert_eq!(researched.value.status, RequestStatus::Researched);
    assert!(
        engines
            .request_research("missing", "x".into(), 0.5, vec![], None)
            .is_err()
    );
    let promoted = engines.request_promote(&a.value.id).expect("promote");
    assert_eq!(promoted.request.value.status, RequestStatus::Promoted);
    // The reciprocal half the MCP verb has always done: promotion creates
    // the idea task and links the two both ways.
    let task_id = format!("request-{}", a.value.id);
    assert!(
        promoted
            .request
            .value
            .refs
            .contains(&format!("task:{task_id}")),
        "{:?}",
        promoted.request.value.refs
    );
    let task = engines
        .plan_task(&task_id)
        .unwrap()
        .expect("promotion created the idea task");
    assert_eq!(task.status, TaskStatus::Idea);
    assert_eq!(task.title, "[suggestion] faster");
    assert!(task.refs.contains(&format!("request:{}", a.value.id)));
    // Promoting again is idempotent: the same task, still one of it.
    let again = engines.request_promote(&a.value.id).expect("promote again");
    assert_eq!(again.request.value.status, RequestStatus::Promoted);
    assert_eq!(
        engines
            .plan_tasks()
            .unwrap()
            .iter()
            .filter(|c| c.id == task_id)
            .count(),
        1
    );

    let dismissed = engines.request_dismiss(&b.value.id).expect("ignore");
    assert_eq!(dismissed.value.status, RequestStatus::Dismissed);

    let snoozed = engines.request_snooze(&c.value.id, 30).expect("snooze");
    assert!(snoozed.value.snooze_until.is_some(), "snooze stamped");
    assert_eq!(
        snoozed.value.status,
        RequestStatus::New,
        "snooze keeps status"
    );

    assert!(engines.request_promote("missing").is_err());
}

#[test]
fn proposal_resolution_wrappers_round_trip() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let cfg = temp_config(&tmp);
    let engines = host(&cfg, "proposals");
    add_task(&engines, "a", 100, &[]);

    // Title: accept adopts the suggestion, reject keeps the plan's title.
    let proposed = engines
        .plan_propose_title("a", "Better title".into(), "why".into(), "tracker".into())
        .expect("propose title");
    assert_eq!(proposed.value.title, "Task a", "a proposal never renames");
    assert!(proposed.value.title_proposal.is_some());
    let rejected = engines
        .plan_resolve_title_proposal("a", false)
        .expect("reject title");
    assert_eq!(rejected.value.title, "Task a");
    assert!(rejected.value.title_proposal.is_none());
    engines
        .plan_propose_title("a", "Better title".into(), "why".into(), "tracker".into())
        .expect("propose title again");
    let accepted = engines
        .plan_resolve_title_proposal("a", true)
        .expect("accept title");
    assert_eq!(accepted.value.title, "Better title");
    assert!(engines.plan_resolve_title_proposal("a", true).is_err());

    // Status: accept takes the suggested transition, reject keeps the
    // current status — either act disposes the proposal.
    let proposed = engines
        .plan_propose_status(
            "a",
            TaskStatus::InProgress,
            "started remotely".into(),
            "tracker".into(),
        )
        .expect("propose status");
    assert_eq!(
        proposed.value.status,
        TaskStatus::ToDo,
        "a proposal never moves"
    );
    let kept = engines
        .plan_resolve_status_proposal("a", false)
        .expect("reject status");
    assert_eq!(kept.value.status, TaskStatus::ToDo);
    assert!(kept.value.status_proposal.is_none());
    engines
        .plan_propose_status(
            "a",
            TaskStatus::InProgress,
            "started remotely".into(),
            "tracker".into(),
        )
        .expect("propose status again");
    let moved = engines
        .plan_resolve_status_proposal("a", true)
        .expect("accept status");
    assert_eq!(moved.value.status, TaskStatus::InProgress);
    assert!(moved.value.status_proposal.is_none());
    assert!(engines.plan_resolve_status_proposal("a", true).is_err());

    // Reprioritize: accept takes the suggested priority, reject keeps the
    // current one; the proposal is disposed either way — including when
    // the suggestion equals the current priority.
    engines
        .plan_reprioritize("a", 300, "later".into())
        .expect("propose reprioritize");
    let kept = engines
        .plan_resolve_reprioritize("a", false)
        .expect("reject reprioritize");
    assert_eq!(kept.value.priority, 100);
    assert!(kept.value.reprioritize.is_none());
    engines
        .plan_reprioritize("a", 300, "later".into())
        .expect("propose reprioritize again");
    let moved = engines
        .plan_resolve_reprioritize("a", true)
        .expect("accept reprioritize");
    assert_eq!(moved.value.priority, 300);
    assert!(moved.value.reprioritize.is_none());
    engines
        .plan_reprioritize("a", 300, "no-op suggestion".into())
        .expect("propose the current priority");
    let same = engines
        .plan_resolve_reprioritize("a", true)
        .expect("accept the no-op");
    assert_eq!(same.value.priority, 300);
    assert!(
        same.value.reprioritize.is_none(),
        "disposed even when identical"
    );
    assert!(engines.plan_resolve_reprioritize("a", true).is_err());
    assert!(engines.plan_resolve_reprioritize("missing", true).is_err());
}

#[test]
fn decision_pin_and_review_act_on_the_live_log() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let cfg = temp_config(&tmp).persist(false);
    let engines = host(&cfg, "pin-checkpoint");
    assert!(engines.decision_record(note(1, "hypothesis")).is_ok());

    assert!(
        !engines.decision_pin(1, true).unwrap().value,
        "the previous flag comes back"
    );
    let pinned = engines
        .note(1)
        .unwrap()
        .and_then(|note| note.pinned)
        .unwrap_or(false);
    assert!(pinned, "the flag stuck");
    assert!(engines.decision_pin(9, true).is_err(), "unknown note");

    let report = engines.decision_review().expect("review");
    assert!(
        report.get("open_hypotheses").is_some(),
        "the review report has its sections: {report}"
    );
}

#[test]
fn family_observations_identify_missing_file_and_memory_sources() {
    use crate::native::ReadSource;

    let tmp = tempfile::tempdir().expect("tempdir");
    let cfg = temp_config(&tmp);
    let engines = host(&cfg, "stamped");
    assert_eq!(
        engines.snapshot().plan.current().unwrap().source,
        ReadSource::Missing
    );

    add_task(&engines, "base", 100, &[]);
    let first = engines.snapshot().plan.current().unwrap().source;
    assert!(matches!(first, ReadSource::File(_)));
    assert_eq!(
        engines.clone().snapshot().plan.current().unwrap().source,
        first
    );
    add_task(&engines, "next", 200, &["base"]);
    assert_ne!(engines.snapshot().plan.current().unwrap().source, first);

    let off = reader(&cfg.persist(false), "stamped");
    let memory = off.snapshot();
    assert_eq!(memory.plan.current().unwrap().source, ReadSource::Memory);
    assert!(memory.plan.current().unwrap().value.tasks.is_empty());
}

/// Each family supplies whole validated records and its own source.
#[test]
fn a_snapshot_reads_every_family_whole_with_its_source() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let cfg = temp_config(&tmp);
    let engines = host(&cfg, "snapshotted");

    let empty = engines.snapshot();
    assert!(!empty.needs_retry());
    assert_eq!(
        empty.plan.current().unwrap().source,
        crate::native::ReadSource::Missing
    );
    assert_eq!(
        empty.run.current().unwrap().source,
        crate::native::ReadSource::Missing
    );
    assert_eq!(
        empty.requests.current().unwrap().source,
        crate::native::ReadSource::Missing
    );
    assert_eq!(
        empty.decisions.current().unwrap().source,
        crate::native::ReadSource::Missing
    );
    assert_ne!(
        empty,
        NativeSnapshot::default(),
        "known empty is not unobserved"
    );

    add_task(&engines, "base", 100, &[]);
    add_task(&engines, "next", 200, &["base"]);
    engines.plan_start_task("base").expect("start");
    engines
        .run_open(
            "run the snapshot".into(),
            vec![],
            vec![],
            "task:base".into(),
        )
        .expect("goal");
    engines
        .add_step("t1".into(), "read it".into(), StepType::Implement, None)
        .expect("step");
    engines.start_step("t1").expect("start step");
    engines
        .record_progress(
            Some("t1"),
            ProgressType::Code,
            "wrote the seed".into(),
            vec![],
            vec![],
            None,
        )
        .expect("progress");
    engines
        .run_record_gate(
            Some("t1"),
            "cargo nextest",
            "true".into(),
            0,
            String::new(),
            true,
            None,
        )
        .expect("gate");
    engines
        .request_capture(RequestKind::Suggestion, "alice".into(), "faster".into())
        .expect("request");
    assert!(engines.decision_record(note(1, "open")).is_ok());

    let snapshot = engines.snapshot();
    assert!(!snapshot.needs_retry());
    assert!(matches!(
        snapshot.plan.current().unwrap().source,
        crate::native::ReadSource::File(_)
    ));
    assert!(matches!(
        snapshot.run.current().unwrap().source,
        crate::native::ReadSource::File(_)
    ));
    assert!(matches!(
        snapshot.requests.current().unwrap().source,
        crate::native::ReadSource::File(_)
    ));
    assert!(matches!(
        snapshot.decisions.current().unwrap().source,
        crate::native::ReadSource::File(_)
    ));
    assert_eq!(
        snapshot
            .run
            .current()
            .unwrap()
            .value
            .goal()
            .as_ref()
            .map(|o| o.scope.as_str()),
        Some("task:base")
    );
    assert_eq!(snapshot.run.current().unwrap().value.steps().len(), 1);
    assert_eq!(
        snapshot.run.current().unwrap().value.steps()[0]
            .progress
            .len(),
        1,
        "progress ride their step"
    );
    assert_eq!(
        snapshot.run.current().unwrap().value.steps()[0]
            .checks
            .len(),
        1,
        "checks ride their step"
    );
    assert_eq!(
        snapshot.run.current().unwrap().value.steps()[0].checks[0].exit_code,
        Some(0),
        "the exit code run_status would have dropped"
    );
    let ids: Vec<&str> = snapshot
        .plan
        .current()
        .unwrap()
        .value
        .tasks
        .iter()
        .map(|c| c.id.as_str())
        .collect();
    assert_eq!(ids, vec!["base", "next"], "stored order");
    assert_eq!(
        snapshot.plan.current().unwrap().value.tasks[0].status,
        TaskStatus::InProgress
    );
    assert_eq!(
        snapshot.plan.current().unwrap().value.progress.total_tasks,
        2
    );
    assert_eq!(snapshot.requests.current().unwrap().value.len(), 1);
    assert_eq!(
        snapshot
            .decisions
            .current()
            .unwrap()
            .value
            .log(None)
            .unwrap()
            .notes
            .len(),
        1,
        "the persisted note"
    );
    assert_eq!(
        snapshot
            .decisions
            .current()
            .unwrap()
            .value
            .log(None)
            .unwrap()
            .notes[0]
            .note,
        1
    );

    // A reader opened beside the writer reads the same store to the same
    // snapshot: the projection is a function of the files, not of the
    // process that wrote them.
    let observer = reader(&cfg, "snapshotted");
    assert_eq!(observer.snapshot(), snapshot);
}

#[test]
fn a_blocker_is_cleared_through_the_wrapper_and_erring_is_loud() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let cfg = temp_config(&tmp);
    let engines = host(&cfg, "blockers");
    add_task(&engines, "stuck", 100, &[]);

    // Nothing to clear yet: the library errs rather than answer a no-op,
    // and the wrapper passes that through — a caller who believes there
    // is a blocker is working from a stale picture.
    assert!(engines.plan_clear_waiting_on("stuck").is_err());

    // A blocker the dock would raise a card for, set the way the MCP
    // verb sets it (validate, then store).
    engines
        .plan_set_waiting_on(
            "stuck",
            WaitingReason::Person,
            "needs a person to pick the palette".into(),
            None,
        )
        .expect("set");
    assert!(
        engines
            .plan_task("stuck")
            .unwrap()
            .expect("task")
            .waiting_on
            .is_some()
    );

    let cleared = engines.plan_clear_waiting_on("stuck").expect("clear");
    assert!(
        cleared.value.waiting_on.is_none(),
        "the card's blocker is lifted"
    );
    assert!(engines.plan_clear_waiting_on("stuck").is_err());
    assert!(engines.plan_clear_waiting_on("missing").is_err());
}

#[test]
fn a_blocker_is_set_through_the_wrapper_the_way_the_verb_sets_it() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let cfg = temp_config(&tmp);
    let engines = host(&cfg, "blockers-set");
    add_task(&engines, "stuck", 100, &[]);

    let blocked = engines
        .plan_set_waiting_on(
            "stuck",
            WaitingReason::Person,
            "the judge says the acceptance cannot be met as written".into(),
            Some("decision:345".into()),
        )
        .expect("set");
    let blocker = blocked
        .value
        .waiting_on
        .expect("the blocker is on the task");
    assert_eq!(blocker.kind, WaitingReason::Person);
    assert!(blocker.reason.contains("cannot be met"));
    assert_eq!(blocker.evidence.as_deref(), Some("decision:345"));
    // Stored, not only answered: a fresh read sees it, and the clear
    // wrapper lifts exactly this.
    assert!(
        engines
            .plan_task("stuck")
            .unwrap()
            .expect("task")
            .waiting_on
            .is_some()
    );
    assert!(engines.plan_clear_waiting_on("stuck").is_ok());

    // An unknown task and an empty reason are refused, not stored.
    assert!(
        engines
            .plan_set_waiting_on("missing", WaitingReason::Person, "why".into(), None)
            .is_err()
    );
    assert!(
        engines
            .plan_set_waiting_on("stuck", WaitingReason::Person, String::new(), None)
            .is_err(),
        "a waiting reason needs a sentence"
    );
}

#[test]
fn request_queries_observe_another_writers_capture() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let cfg = temp_config(&tmp);
    let host = reader(&cfg, "reload-request");
    let server = service(&cfg, "reload-request");

    // The agent's `magistr mcp` captures while the host is open.
    let captured = server
        .request_capture(RequestKind::Suggestion, "alice".into(), "faster".into())
        .expect("capture");

    // The host's in-memory copy predates it; its queries read through.
    assert_eq!(host.request_list().unwrap().len(), 1);
    assert_eq!(host.request_status().unwrap()["counts"]["total"], 1);

    let ids: Vec<String> = host
        .request_list()
        .unwrap()
        .into_iter()
        .map(|s| s.id)
        .collect();
    assert_eq!(ids, vec![captured.value.id]);
    assert_eq!(host.request_status().unwrap()["counts"]["total"], 1);
}

/// A run trace with one active step, ready to take a gate record.
fn engines_with_active_step(tmp: &tempfile::TempDir) -> NativeEngines {
    let engines = host(&temp_config(tmp), "gate-rec");
    engines
        .run_open("record the gates".into(), vec![], vec![], String::new())
        .expect("goal");
    engines
        .add_step("t1".into(), "run gates".into(), StepType::Implement, None)
        .expect("add step");
    engines.start_step("t1").expect("start");
    engines
}

fn checks(engines: &NativeEngines) -> Vec<Check> {
    engines
        .run_steps()
        .unwrap()
        .into_iter()
        .flat_map(|t| t.checks)
        .collect()
}

#[test]
fn a_recorded_gate_is_verified_and_carries_the_command_and_the_real_exit_code() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let engines = engines_with_active_step(&tmp);

    engines
        .run_record_gate(
            None,
            "lint",
            "cargo clippy --workspace".to_owned(),
            3,
            "error: something".to_owned(),
            true,
            None,
        )
        .expect("record");

    let checks = checks(&engines);
    assert_eq!(checks.len(), 1);
    let check = &checks[0];
    assert!(check.verified, "a gate the loop ran is not self-reported");
    assert!(!check.passed);
    assert_eq!(check.exit_code, Some(3));
    assert_eq!(check.command.as_deref(), Some("cargo clippy --workspace"));
    assert!(check.required);
    assert_eq!(check.details, "error: something");
}

#[test]
fn the_check_type_is_derived_from_the_gate_name_archetype_suffix_and_all() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let engines = engines_with_active_step(&tmp);
    for name in ["check", "test:app-typescript", "lint", "structure", "test"] {
        engines
            .run_record_gate(None, name, "true".to_owned(), 0, String::new(), true, None)
            .expect("record");
    }
    let kinds: Vec<CheckType> = checks(&engines)
        .iter()
        .map(|c| c.check_type.clone())
        .collect();
    assert_eq!(
        kinds,
        vec![
            CheckType::Typecheck,
            CheckType::Test,
            CheckType::Lint,
            // A custom gate is a lint until it says otherwise — the same
            // fallback `magistr_gate`'s own mapping uses.
            CheckType::Lint,
            CheckType::Test,
        ]
    );
}

#[test]
fn a_junit_report_is_parsed_into_the_record() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let engines = engines_with_active_step(&tmp);
    let report = tmp.path().join("junit.xml");
    std::fs::write(
        &report,
        r#"<?xml version="1.0"?>
<testsuites tests="3" failures="1" skipped="1">
  <testsuite name="s" tests="3" failures="1" skipped="1">
    <testcase classname="s" name="ok"/>
    <testcase classname="s" name="bad"><failure message="boom"/></testcase>
    <testcase classname="s" name="gone"><skipped/></testcase>
  </testsuite>
</testsuites>"#,
    )
    .expect("write junit");

    engines
        .run_record_gate(
            None,
            "test",
            "cargo nextest run".to_owned(),
            100,
            "…".to_owned(),
            true,
            Some(("junit", &report.to_string_lossy())),
        )
        .expect("record");

    let checks = checks(&engines);
    let results = checks[0].results.as_ref().expect("results");
    assert_eq!(results.total, 3);
    assert_eq!(results.failed, 1);
    assert_eq!(results.skipped, 1);
    assert!(
        results.failures.iter().any(|f| f.name.contains("bad")),
        "{:?}",
        results.failures
    );
    let record = checks[0].report.as_ref().expect("report record");
    assert!(record.parsed, "{record:?}");
}

#[test]
fn a_missing_report_is_not_parsed_and_never_a_failure() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let engines = engines_with_active_step(&tmp);
    let absent = tmp.path().join("never-written.xml");

    let check = engines
        .run_record_gate(
            None,
            "test",
            "cargo nextest run".to_owned(),
            0,
            String::new(),
            true,
            Some(("junit", &absent.to_string_lossy())),
        )
        .expect("a missing report does not fail the record");

    assert!(
        check.value.passed,
        "the exit code decides, never the report"
    );
    assert!(check.value.results.is_none());
    assert_eq!(check.value.report.as_ref().map(|r| r.parsed), Some(false));
}

#[test]
fn a_gate_cannot_be_recorded_with_no_step_to_record_it_against() {
    // The loop needs the refusal to be legible: it prints it on the stage
    // rather than silently recording nothing.
    let tmp = tempfile::tempdir().expect("tempdir");
    let engines = host(&temp_config(&tmp), "no-step");
    let refusal = engines
        .run_record_gate(
            None,
            "check",
            "true".to_owned(),
            0,
            String::new(),
            true,
            None,
        )
        .expect_err("no active step");
    assert!(
        !refusal.to_string().is_empty(),
        "the refusal says something"
    );
}

/// The verb's own refusals reach the caller in the handler's words: what
/// the wire refuses, the in-process writer refuses identically, because it
/// is the same handler.
#[test]
fn the_handlers_refusals_are_the_writers_refusals() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let cfg = temp_config(&tmp).persist(false);
    let engines = host(&cfg, "refusals");

    let empty = engines
        .request_capture(RequestKind::Suggestion, "alice".into(), "   ".into())
        .expect_err("an empty body is refused as the verb refuses it");
    assert_eq!(empty.to_string(), "request body is empty");

    let captured = engines
        .request_capture(RequestKind::Suggestion, "alice".into(), "faster".into())
        .expect("capture");
    let blank = engines
        .request_research(&captured.value.id, "  ".into(), 0.5, vec![], None)
        .expect_err("a blank summary is refused");
    assert_eq!(blank.to_string(), "research summary is empty");

    add_task(&engines, "c", 100, &[]);
    let done = engines
        .plan_complete_task("c", None)
        .expect_err("no goal certifies the task");
    assert!(
        done.to_string()
            .starts_with("completion refused: no run goal is set"),
        "{done}"
    );

    let no_step = engines
        .start_step("nothing")
        .expect_err("an unknown step is refused");
    assert!(no_step.to_string().contains("nothing"), "{no_step}");
}

/// A fresh goal after a shipped one clears the stale plan — the
/// count the handler reports comes back as the typed answer.
#[test]
fn a_fresh_cycle_reports_how_many_stale_steps_it_cleared() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let cfg = temp_config(&tmp).persist(false);
    let engines = host(&cfg, "fresh-cycle");
    assert_eq!(
        engines
            .run_open("first".into(), vec![], vec![], String::new())
            .expect("goal")
            .value,
        0
    );
    engines
        .add_step("t1".into(), "one".into(), StepType::Implement, None)
        .expect("add step");
    engines
        .add_step("t2".into(), "two".into(), StepType::Test, None)
        .expect("add step");
    engines.run_finish(vec![], None).expect("finalize");
    assert_eq!(
        engines
            .run_open("second".into(), vec![], vec![], String::new())
            .expect("goal")
            .value,
        2,
        "the finished cycle's two steps were cleared"
    );
    assert!(engines.run_steps().unwrap().is_empty());
}
