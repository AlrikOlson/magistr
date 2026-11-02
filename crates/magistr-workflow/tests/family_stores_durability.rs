//! Durability of the run/plan/signal stores under concurrent server
//! processes — the family-stores extension of `decision_trace_durability.rs`.
//!
//! Residue of the 2026-06-09 decision incident: these three engines still did
//! load-once-at-startup + whole-file-overwrite of their per-project
//! `<project_id>.json`, so two live engines in the SAME project could clobber
//! each other's acked mutations. The `*_clobber*` tests reproduce that loss
//! mechanism and MUST fail on the pre-fix unlocked overwrite persistence.

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

fn plan_engine(tmp: &TempDir, project: &str) -> PlanEngine {
    PlanEngine::new(project.into())
        .with_persistence(Persistence::new(&infra_cfg(tmp), Domain::Plan))
}

fn signal_engine(tmp: &TempDir, project: &str) -> SignalEngine {
    SignalEngine::new(project.into())
        .with_persistence(Persistence::new(&infra_cfg(tmp), Domain::Signal))
}

fn run_engine(tmp: &TempDir, project: &str) -> RunEngine {
    let cfg = run_persistence::PersistenceConfig {
        enabled: true,
        data_dir: tmp.path().to_path_buf(),
    };
    RunEngine::new(project.into()).with_persistence(run_persistence::Persistence::new(&cfg))
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

// ── plan ────────────────────────────────────────────────────────────────

/// THE INCIDENT SHAPE, on the plan store: two live engines share one data
/// dir and one project. B adds a task (acked + persisted). A — which loaded
/// before B's write and knows nothing of it — then adds its own task. A's
/// save must not erase B's acked task from disk.
#[test]
fn concurrent_plan_writer_does_not_clobber_acked_tasks() {
    let tmp = TempDir::new().unwrap();
    let mut a = plan_engine(&tmp, "proj");
    let mut b = plan_engine(&tmp, "proj");

    add_task(&mut b, "from-b");
    add_task(&mut a, "from-a");

    let reloaded = plan_engine(&tmp, "proj");
    let ids: Vec<&str> = reloaded
        .plan()
        .tasks
        .iter()
        .map(|c| c.id.as_str())
        .collect();
    assert!(
        ids.contains(&"from-b"),
        "task from-b was acked to B but a stale concurrent writer erased it; survived: {ids:?}"
    );
    assert!(
        ids.contains(&"from-a"),
        "task from-a lost; survived: {ids:?}"
    );
}

/// Per-task conflicts resolve by recency: a stale writer that still holds an
/// old copy of task X must not roll back the status another process advanced.
#[test]
fn stale_plan_writer_does_not_roll_back_task_status() {
    let tmp = TempDir::new().unwrap();
    {
        let mut seed = plan_engine(&tmp, "proj");
        add_task(&mut seed, "x");
    }

    let mut a = plan_engine(&tmp, "proj"); // both load x@Pending
    let mut b = plan_engine(&tmp, "proj");

    b.set_status("x", TaskStatus::InProgress).unwrap(); // B advances x
    add_task(&mut a, "y"); // A persists with a STALE x@Pending

    let reloaded = plan_engine(&tmp, "proj");
    let x = reloaded
        .plan()
        .tasks
        .iter()
        .find(|c| c.id == "x")
        .expect("x must survive");
    assert_eq!(
        x.status,
        TaskStatus::InProgress,
        "stale writer rolled back x's acked status transition"
    );
    assert!(reloaded.plan().tasks.iter().any(|c| c.id == "y"));
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

// ── run ───────────────────────────────────────────────────────────────────

/// Two engines working the SAME cycle (B loaded the goal A persisted):
/// a step acked to either process must survive the other's saves.
#[test]
fn concurrent_run_writers_union_steps_within_one_cycle() {
    let tmp = TempDir::new().unwrap();
    let mut a = run_engine(&tmp, "proj");
    a.set_goal("Shared cycle".into(), vec![], vec![], String::new())
        .unwrap();

    // B loads the persisted goal — same cycle as A.
    let mut b = run_engine(&tmp, "proj");
    assert!(b.goal.is_some(), "B must join A's cycle");

    a.add_step(
        "a-step".into(),
        "A's step".into(),
        StepType::Implement,
        None,
        None,
    )
    .unwrap();
    // B never saw a-step; its save must not erase it.
    b.add_step(
        "b-step".into(),
        "B's step".into(),
        StepType::Test,
        None,
        None,
    )
    .unwrap();

    let reloaded = run_engine(&tmp, "proj");
    let ids: Vec<&str> = reloaded.steps.iter().map(|t| t.id.as_str()).collect();
    assert!(
        ids.contains(&"a-step"),
        "step acked to A erased by stale writer B; survived: {ids:?}"
    );
    assert!(ids.contains(&"b-step"), "step from B lost: {ids:?}");
}

/// A stale process still holding a PRIOR cycle must not clobber the new cycle
/// another process started after a reset.
#[test]
fn stale_run_cycle_does_not_clobber_a_newer_cycle() {
    let tmp = TempDir::new().unwrap();
    let mut a = run_engine(&tmp, "proj");
    a.set_goal("Old cycle".into(), vec![], vec![], String::new())
        .unwrap();

    let mut b = run_engine(&tmp, "proj"); // B holds the old cycle
    assert_eq!(b.goal.as_ref().unwrap().description, "Old cycle");

    a.reset().unwrap();
    a.set_goal("New cycle".into(), vec![], vec![], String::new())
        .unwrap();

    // B (stale, old cycle) saves — it must not resurrect the old cycle.
    b.add_step(
        "zombie".into(),
        "stale".into(),
        StepType::Implement,
        None,
        None,
    )
    .unwrap_err();

    let reloaded = run_engine(&tmp, "proj");
    assert_eq!(
        reloaded.goal.as_ref().map(|o| o.description.as_str()),
        Some("New cycle"),
        "a stale writer resurrected a reset-away cycle"
    );
    assert!(
        reloaded.steps.iter().all(|t| t.id != "zombie"),
        "a stale cycle's step leaked into the new cycle"
    );
}

/// Reset stays a reset for the resetting process itself: its own next cycle
/// must not re-merge the pre-reset steps. (Guards the merge-on-save change —
/// this already passes pre-fix and must keep passing.)
#[test]
fn run_reset_is_not_resurrected_by_later_saves() {
    let tmp = TempDir::new().unwrap();
    let mut a = run_engine(&tmp, "proj");
    a.set_goal("Cycle 1".into(), vec![], vec![], String::new())
        .unwrap();
    a.add_step(
        "old".into(),
        "pre-reset".into(),
        StepType::Implement,
        None,
        None,
    )
    .unwrap();

    a.reset().unwrap();
    a.set_goal("Cycle 2".into(), vec![], vec![], String::new())
        .unwrap();
    a.add_step(
        "new".into(),
        "post-reset".into(),
        StepType::Implement,
        None,
        None,
    )
    .unwrap();

    let reloaded = run_engine(&tmp, "proj");
    assert_eq!(
        reloaded.goal.as_ref().map(|o| o.description.as_str()),
        Some("Cycle 2")
    );
    let ids: Vec<&str> = reloaded.steps.iter().map(|t| t.id.as_str()).collect();
    assert_eq!(ids, vec!["new"], "reset-away step resurrected: {ids:?}");
}

#[test]
fn rejected_plan_and_signal_writes_preserve_bytes_and_retry_after_repair() {
    use magistr_workflow::plan::engine::PlanMutationError;
    use magistr_workflow::signal::engine::SignalMutationError;
    use magistr_workflow_contracts::{ReadErrorKind, ReadSource, WriteErrorKind};
    let tmp = TempDir::new().unwrap();
    let mut plan = plan_engine(&tmp, "proj");
    let mut signal = signal_engine(&tmp, "proj");
    add_task(&mut plan, "saved");
    let captured = signal
        .capture(SignalKind::Idea, "author".into(), "saved".into())
        .unwrap();
    let plan_path = Persistence::new(&infra_cfg(&tmp), Domain::Plan).path_for("proj");
    let signal_path = Persistence::new(&infra_cfg(&tmp), Domain::Signal).path_for("proj");
    let plan_bytes = std::fs::read(&plan_path).unwrap();
    let signal_bytes = std::fs::read(&signal_path).unwrap();
    for (bad, kind) in [
        (b"{".to_vec(), ReadErrorKind::Decode),
        (b"{}".to_vec(), ReadErrorKind::Decode),
    ] {
        std::fs::write(&plan_path, &bad).unwrap();
        std::fs::write(&signal_path, &bad).unwrap();
        let PlanMutationError::Store(error) = plan.link_task("saved", "decision:1").unwrap_err()
        else {
            panic!("expected store refusal")
        };
        assert_eq!(error.kind, WriteErrorKind::Read(kind));
        assert_eq!(error.effect(), "none");
        let SignalMutationError::Store(error) =
            signal.link(&captured.value.id, "decision:1").unwrap_err()
        else {
            panic!("expected store refusal")
        };
        assert_eq!(error.kind, WriteErrorKind::Read(kind));
        assert_eq!(error.effect(), "none");
        assert_eq!(std::fs::read(&plan_path).unwrap(), bad);
        assert_eq!(std::fs::read(&signal_path).unwrap(), bad);
        assert!(plan.plan().tasks[0].refs.is_empty());
        assert!(signal.signals().signals[0].refs.is_empty());
    }
    std::fs::write(&plan_path, plan_bytes).unwrap();
    std::fs::write(&signal_path, signal_bytes).unwrap();
    let saved = plan.link_task("saved", "decision:1").unwrap();
    assert!(matches!(saved.source, ReadSource::File(_)));
    assert_eq!(saved.source, plan.observe().unwrap().source);
    let saved = signal.link(&captured.value.id, "decision:1").unwrap();
    assert_eq!(saved.source, signal.observe().unwrap().source);
    assert_eq!(saved.value.refs, ["decision:1"]);
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
                let mut plan = plan_engine(tmp, "proj");
                let mut signals = signal_engine(tmp, "proj");
                barrier.wait();
                add_task(&mut plan, &format!("work-{i}"));
                signals
                    .capture(SignalKind::Idea, "author".into(), format!("idea {i}"))
                    .unwrap();
            });
        }
    });
    assert_eq!(
        plan_engine(&tmp, "proj")
            .observe()
            .unwrap()
            .value
            .tasks
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
