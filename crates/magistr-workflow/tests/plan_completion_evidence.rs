//! Completion governed by the run run's check_results, on every plan route.

use std::sync::{Arc, Mutex};

use magistr_workflow::plan::PlanEngine;
use magistr_workflow::plan::application::{PlanError, PlanUseCases, TaskDraft};
use magistr_workflow::plan::domain::TaskStatus;
use magistr_workflow::run::application::{
    CheckExecution, CheckOutcome, CheckRequest, EvidencePort, GateContract, RunUseCases,
    SourceFreshness, SourceObservation, SourceWitness,
};
use magistr_workflow::run::domain::{check::CheckType, step::StepType};
use magistr_workflow::run::engine::RunEngine;

/// A source that never moves while a gate runs: the revision it was
/// observed at is the revision it validates.
struct Fixed(String);

impl SourceWitness for Fixed {
    fn observation(&self) -> SourceObservation {
        SourceObservation {
            inputs: Vec::new(),
            execution_digest: String::new(),
            workspace_root: "/w".into(),
            workspace_generation: None,
            scheme: "fake/1".into(),
            policy_digest: "p".into(),
            revision: self.0.clone(),
            source_digest: self.0.clone(),
            config_digest: "c".into(),
            consistency: "optimistic".into(),
            captured_at_ms: 0,
        }
    }
    fn validate(&self) -> Result<SourceFreshness, String> {
        Ok(SourceFreshness::Unchanged)
    }
}

/// A fake evidence capability whose source revision the test moves between
/// a gate and the completion that consults it.
struct Port {
    revision: Arc<Mutex<String>>,
}

impl EvidencePort for Port {
    fn observe(&self) -> Result<Box<dyn SourceWitness>, String> {
        Ok(Box::new(Fixed(self.revision.lock().unwrap().clone())))
    }
    fn required_gates(&self) -> Result<Vec<GateContract>, String> {
        Ok(vec![GateContract {
            name: "test".into(),
            command: "cargo test".into(),
            check_type: CheckType::Test,
        }])
    }
}

fn task(id: &str, status: TaskStatus) -> TaskDraft {
    TaskDraft {
        id: id.into(),
        title: format!("Task {id}"),
        name: String::new(),
        status,
        priority: 100,
        description: String::new(),
        acceptance: vec![],
        deps: vec![],
        shared: false,
        feature: None,
        content: None,
    }
}

/// A plan over `run`, with `c1` started.
fn plan_over(run: &RunUseCases) -> PlanUseCases {
    let plan = PlanUseCases::new(
        Arc::new(Mutex::new(PlanEngine::new("project".into()))),
        run.clone(),
    );
    plan.add_task(task("c1", TaskStatus::ToDo)).unwrap();
    plan.start_task("c1").unwrap();
    plan
}

/// The run family with the fake evidence capability, and the handle that
/// moves its source revision.
fn evidenced() -> (RunUseCases, Arc<Mutex<String>>) {
    let revision = Arc::new(Mutex::new("r1".to_owned()));
    let run = RunUseCases::new(Arc::new(Mutex::new(RunEngine::new("project".into()))))
        .with_evidence(Arc::new(Port {
            revision: Arc::clone(&revision),
        }));
    (run, revision)
}

fn composed() -> (PlanUseCases, RunUseCases) {
    let (run, _) = evidenced();
    (plan_over(&run), run)
}

fn goal(run: &RunUseCases, scope: &str) {
    run.set_goal("Do c1".into(), vec![], vec![], scope.into())
        .unwrap();
    run.add_step(
        "verify".into(),
        "Verify".into(),
        StepType::Review,
        None,
        None,
    )
    .unwrap();
    run.start_step("verify").unwrap();
}

fn gate(run: &RunUseCases, code: i32) {
    let admission = run
        .admit_check(CheckRequest {
            options: Default::default(),
            step_id: None,
            check_type: CheckType::Test,
            name: "test".into(),
            passed: None,
            details: String::new(),
            required: true,
            command: Some("cargo test".into()),
            report: None,
        })
        .unwrap();
    run.settle_check(
        admission.value,
        CheckOutcome {
            passed: code == 0,
            details: String::new(),
            verified: true,
            command: Some("cargo test".into()),
            exit_code: Some(code),
            execution: CheckExecution::Exited { code },
            report: None,
            results: None,
        },
    )
    .unwrap();
}

#[test]
fn completion_refuses_without_check_results_and_names_the_resolved_step_when_it_completes() {
    let (plan, run) = composed();
    let refusal = plan.complete_task("c1", None).unwrap_err();
    assert!(refusal.to_string().contains("no run goal"), "{refusal}");

    goal(&run, "task:c1");
    let refusal = plan.complete_task("c1", None).unwrap_err();
    assert!(
        refusal.to_string().contains("no check was recorded"),
        "{refusal}"
    );

    gate(&run, 1);
    let refusal = plan.complete_task("c1", Some("step:verify")).unwrap_err();
    let text = refusal.to_string();
    assert!(
        text.contains("test: the latest attempt did not pass (exit 1)"),
        "{text}"
    );
    assert!(
        text.contains("Re-run the outstanding gates on `verify`"),
        "{text}"
    );
    assert_eq!(
        plan.engine().lock().unwrap().plan().tasks[0].status,
        TaskStatus::InProgress
    );

    gate(&run, 0);
    let done = plan.complete_task("c1", None).unwrap();
    assert_eq!(done.value.status, TaskStatus::Done);
    assert!(
        done.value.refs.iter().any(|r| r == "step:verify"),
        "{:?}",
        done.value.refs
    );
}

#[test]
fn a_foreign_scope_or_step_is_refused_by_name() {
    let (plan, run) = composed();
    goal(&run, "task:other");
    gate(&run, 0);
    let refusal = plan.complete_task("c1", None).unwrap_err();
    assert!(
        refusal.to_string().contains("scoped to `task:other`"),
        "{refusal}"
    );
    run.set_goal("Do c1".into(), vec![], vec![], "task:c1".into())
        .unwrap();
    let refusal = plan.complete_task("c1", Some("step:ghost")).unwrap_err();
    assert!(
        refusal.to_string().contains("step `ghost` is not a step"),
        "{refusal}"
    );
}

#[test]
fn the_plain_status_routes_cannot_reach_done() {
    let (plan, _run) = composed();
    assert!(matches!(
        plan.set_status("c1", TaskStatus::Done),
        Err(PlanError::InvalidState(reason)) if reason.contains("plan_complete_task")
    ));
    plan.propose_status(
        "c1",
        TaskStatus::Done,
        "ticket closed".into(),
        "tracker".into(),
    )
    .unwrap();
    assert!(matches!(
        plan.resolve_status_proposal("c1", true),
        Err(PlanError::InvalidState(reason)) if reason.contains("plan_complete_task")
    ));
    // Rejecting the proposal, and every other status, stays open.
    plan.resolve_status_proposal("c1", false).unwrap();
    plan.set_status("c1", TaskStatus::Waiting).unwrap();
}

#[test]
fn stale_source_evidence_is_refused_until_the_gate_reruns() {
    let (run, revision) = evidenced();
    let plan = plan_over(&run);
    goal(&run, "task:c1");
    gate(&run, 0);
    *revision.lock().unwrap() = "r2".into();
    let refusal = plan.complete_task("c1", None).unwrap_err();
    assert!(
        refusal
            .to_string()
            .contains("the source changed since the gate ran"),
        "{refusal}"
    );
    assert_eq!(
        plan.engine().lock().unwrap().plan().tasks[0].status,
        TaskStatus::InProgress
    );
    gate(&run, 0);
    assert_eq!(
        plan.complete_task("c1", None).unwrap().value.status,
        TaskStatus::Done
    );
}

/// A run family composed without an evidence capability is still the
/// plan's completion authority — one that refuses every completion with
/// the reason. There is no permissive branch to fall back to: the
/// constructor takes the authority, and the plain status routes refuse
/// `done` whatever the run family can certify.
#[test]
fn without_an_evidence_capability_every_route_to_done_is_refused_and_planning_continues() {
    let run = RunUseCases::new(Arc::new(Mutex::new(RunEngine::new("project".into()))));
    let plan = plan_over(&run);
    goal(&run, "task:c1");

    let refusal = plan.complete_task("c1", Some("step:verify")).unwrap_err();
    assert!(
        refusal
            .to_string()
            .contains("no verification evidence capability is composed"),
        "{refusal}"
    );
    assert!(matches!(
        plan.set_status("c1", TaskStatus::Done),
        Err(PlanError::InvalidState(reason)) if reason.contains("plan_complete_task")
    ));
    plan.propose_status(
        "c1",
        TaskStatus::Done,
        "ticket closed".into(),
        "tracker".into(),
    )
    .unwrap();
    assert!(matches!(
        plan.resolve_status_proposal("c1", true),
        Err(PlanError::InvalidState(reason)) if reason.contains("plan_complete_task")
    ));
    let tasks = plan.engine().lock().unwrap().plan().tasks.clone();
    assert_eq!(tasks[0].status, TaskStatus::InProgress);
    assert!(tasks[0].refs.is_empty(), "{:?}", tasks[0].refs);

    // Ordinary planning mutations and completed history are untouched.
    plan.resolve_status_proposal("c1", false).unwrap();
    assert_eq!(
        plan.set_status("c1", TaskStatus::Waiting)
            .unwrap()
            .value
            .status,
        TaskStatus::Waiting
    );
    assert_eq!(
        plan.link_task("c1", "decision:7").unwrap().value.refs,
        vec!["decision:7".to_owned()]
    );
    assert_eq!(
        plan.add_task(task("shipped-last-year", TaskStatus::Done))
            .unwrap()
            .value
            .status,
        TaskStatus::Done
    );
    assert_eq!(
        plan.retire_task("c1", "superseded".into())
            .unwrap()
            .value
            .status,
        TaskStatus::Retired
    );
}
