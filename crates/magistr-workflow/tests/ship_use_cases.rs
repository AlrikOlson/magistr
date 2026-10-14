//! Typed Ship use cases, native persistence, and transport behavior.

use std::sync::{Arc, Mutex};

use magistr_workflow::ship::ShipService;
use magistr_workflow::ship::application::{
    CheckExecution, CheckOutcome, CheckRequest, CheckSettlement, ShipUseCases,
};
use magistr_workflow::ship::domain::{check::CheckType, task::TaskType};
use magistr_workflow::ship::engine::ShipEngine;
use magistr_workflow::ship::mcp::handlers::{NoArgs, PlanAction, PlanArgs, ShipArgs};
use magistr_workflow::ship::persistence::{Persistence, PersistenceConfig};
use rmcp::handler::server::wrapper::Parameters;
use serde_json::{Value, json};

fn stored(root: &std::path::Path) -> ShipUseCases {
    let persistence = Persistence::new(&PersistenceConfig {
        enabled: true,
        data_dir: root.into(),
    });
    ShipUseCases::new(Arc::new(Mutex::new(
        ShipEngine::new("project".into()).with_persistence(persistence),
    )))
}

fn planned(cases: &ShipUseCases) {
    cases
        .set_objective("Current run".into(), vec![], vec![], "chunk:test".into())
        .unwrap();
    cases
        .add_task("verify".into(), "Verify".into(), TaskType::Test, None, None)
        .unwrap();
    cases.start_task("verify").unwrap();
}

fn request() -> CheckRequest {
    CheckRequest {
        task_id: None,
        check_type: CheckType::Test,
        name: "test".into(),
        passed: None,
        details: "runner detail".into(),
        required: true,
        command: Some("exit 0".into()),
        report: None,
    }
}

fn outcome() -> CheckOutcome {
    CheckOutcome {
        passed: true,
        details: "completed execution output".into(),
        verified: true,
        command: Some("exit 0".into()),
        exit_code: Some(0),
        execution: CheckExecution::Exited { code: 0 },
        report: None,
        results: None,
    }
}

#[tokio::test]
async fn native_and_mcp_views_share_one_engine_and_response_contract() {
    let temp = tempfile::tempdir().unwrap();
    let cases = stored(temp.path());
    cases
        .set_objective("Goal".into(), vec![], vec![], "scope".into())
        .unwrap();
    let service = ShipService::from_use_cases(cases.clone());
    assert!(Arc::ptr_eq(&cases.engine(), &service.engine()));
    let answer = service
        .ship_plan(Parameters(PlanArgs {
            action: PlanAction::Add,
            task_id: "verify".into(),
            title: None,
            task_type: None,
            estimate: None,
            after: None,
            think_branch: None,
        }))
        .await
        .unwrap();
    assert_eq!(
        answer.structured_content.unwrap(),
        serde_json::to_value(cases.plan().unwrap()).unwrap()
    );
    assert_eq!(
        cases.snapshot().unwrap().tasks[0].task_type,
        TaskType::Implement
    );
    cases.start_task("verify").unwrap();
    let status = service.ship_status(Parameters(NoArgs {})).await.unwrap();
    assert_eq!(
        status.structured_content.unwrap(),
        serde_json::to_value(cases.status().unwrap()).unwrap()
    );
    assert!(
        cases
            .start_task("verify")
            .unwrap_err()
            .contains("already active")
    );
}

#[tokio::test]
async fn ordinary_completion_keeps_failed_required_checks_as_warnings() {
    let temp = tempfile::tempdir().unwrap();
    let cases = stored(temp.path());
    planned(&cases);
    let mut failed = outcome();
    failed.passed = false;
    failed.exit_code = Some(7);
    failed.execution = CheckExecution::Exited { code: 7 };
    cases.record_check(request(), failed).unwrap();
    cases.complete_task("verify", vec![]).unwrap();
    let service = ShipService::from_use_cases(cases.clone());
    let report = service
        .ship_finalize(Parameters(ShipArgs {
            artifacts: vec![],
            summary: None,
        }))
        .await
        .unwrap()
        .structured_content
        .unwrap();
    assert_eq!(
        report,
        json!({ "status": "shipped", "summary": null,
        "tasks": { "total": 1, "completed": 1 }, "artifacts_count": 0,
        "ship_artifacts": [], "warnings": ["required checks failed: test (task: verify)"],
        "recovered_failures": [], "outstanding_failures": ["test (task: verify)"] })
    );
    let restored = stored(temp.path());
    assert_eq!(
        serde_json::to_value(restored.status().unwrap()).unwrap()["objective"]["status"],
        "completed"
    );
}

#[tokio::test]
async fn check_result_keeps_its_admitted_task_when_active_task_changes() {
    let temp = tempfile::tempdir().unwrap();
    let cases = stored(temp.path());
    planned(&cases);
    let admission = cases.admit_check(request()).unwrap();
    cases.block_task("verify", "other work".into()).unwrap();
    cases
        .add_task("other".into(), "Other".into(), TaskType::Test, None, None)
        .unwrap();
    cases.start_task("other").unwrap();
    let result = ShipUseCases::execute_check(&admission).await;
    assert!(result.passed && result.verified);
    assert!(matches!(
        cases.settle_check(admission, result).unwrap(),
        CheckSettlement::Applied { .. }
    ));
    let snapshot = cases.snapshot().unwrap();
    assert_eq!(snapshot.active_task().unwrap().id, "other");
    assert_eq!(snapshot.tasks[0].checks.len(), 1);
    assert!(snapshot.tasks[1].checks.is_empty());
}

#[tokio::test]
async fn late_async_result_after_peer_rollover_is_returned_intact() {
    let temp = tempfile::tempdir().unwrap();
    let cases = stored(temp.path());
    planned(&cases);
    let admission = cases.admit_check(request()).unwrap();
    let admitted_run = admission.target.run_id.clone();
    let expected = serde_json::to_value(&admission).unwrap();
    let peer = stored(temp.path());
    let (started, ready) = tokio::sync::oneshot::channel();
    let (release, execution) = tokio::sync::oneshot::channel();
    let settling = cases.clone();
    let worker = tokio::spawn(async move {
        started.send(()).unwrap();
        let result = execution.await.unwrap();
        settling.settle_check(admission, result).unwrap()
    });
    ready.await.unwrap();
    peer.complete_task("verify", vec![]).unwrap();
    peer.finalize(vec![], None).unwrap();
    planned(&peer);
    assert_ne!(peer.admit_task(None).unwrap().run_id, admitted_run);
    release.send(outcome()).unwrap();
    let settlement = tokio::time::timeout(std::time::Duration::from_secs(3), worker)
        .await
        .unwrap()
        .unwrap();
    let CheckSettlement::Stale {
        admission,
        outcome: returned,
        receipt,
    } = settlement
    else {
        panic!("late run result applied");
    };
    assert_eq!(serde_json::to_value(&admission).unwrap(), expected);
    // The late result kept its own durable record under the original run.
    assert_eq!(receipt.id, admission.receipt_id);
    assert_eq!(receipt.target.run_id, admitted_run);
    assert!(!receipt.is_applied());
    assert!(receipt.passed());
    assert_eq!(
        serde_json::to_value(returned).unwrap(),
        serde_json::to_value(outcome()).unwrap()
    );
    assert!(cases.snapshot().unwrap().tasks[0].checks.is_empty());
    assert!(
        stored(temp.path()).snapshot().unwrap().tasks[0]
            .checks
            .is_empty()
    );
}

#[test]
fn task_removal_survives_stale_writer_and_reuse_gets_a_new_instance() {
    let temp = tempfile::tempdir().unwrap();
    let cases = stored(temp.path());
    planned(&cases);
    let admission = cases.admit_check(request()).unwrap();
    let stale = stored(temp.path());
    cases.block_task("verify", "replace task".into()).unwrap();
    cases.remove_task("verify").unwrap();
    assert!(stored(temp.path()).snapshot().unwrap().tasks.is_empty());
    // Exercise the inherited merge path with an engine that has not refreshed.
    stale.engine().lock().unwrap().add_task(
        "other".into(),
        "Other".into(),
        TaskType::Docs,
        None,
        None,
    );
    assert!(
        stored(temp.path())
            .snapshot()
            .unwrap()
            .tasks
            .iter()
            .all(|task| task.id != "verify")
    );
    cases
        .add_task(
            "verify".into(),
            "Replacement".into(),
            TaskType::Test,
            None,
            None,
        )
        .unwrap();
    cases.start_task("verify").unwrap();
    assert_ne!(
        cases.admit_task(None).unwrap().task_instance,
        admission.target.task_instance
    );
    assert!(matches!(
        cases.settle_check(admission, outcome()).unwrap(),
        CheckSettlement::Stale { .. }
    ));
    assert!(
        stored(temp.path())
            .snapshot()
            .unwrap()
            .active_task()
            .unwrap()
            .checks
            .is_empty()
    );
}

#[test]
fn reset_during_execution_does_not_resurrect_the_native_file() {
    let temp = tempfile::tempdir().unwrap();
    let cases = stored(temp.path());
    planned(&cases);
    let admission = cases.admit_check(request()).unwrap();
    stored(temp.path()).reset().unwrap();
    let path = temp.path().join("ship/sessions/project.json");
    assert!(!path.exists());
    assert!(matches!(
        cases.settle_check(admission, outcome()).unwrap(),
        CheckSettlement::Stale { .. }
    ));
    assert!(!path.exists(), "late settlement cannot undo reset");
}

#[test]
fn incomplete_execution_state_refuses_admission_and_preserves_the_record() {
    let temp = tempfile::tempdir().unwrap();
    let cases = stored(temp.path());
    planned(&cases);
    let path = temp.path().join("ship/sessions/project.json");
    let mut incomplete: Value = serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
    incomplete.as_object_mut().unwrap().remove("execution");
    let bytes = serde_json::to_vec(&incomplete).unwrap();
    std::fs::write(&path, &bytes).unwrap();
    let reopened = stored(temp.path());
    assert!(
        reopened.admit_task(None).is_err(),
        "a reader must not invent execution identity"
    );
    assert!(
        reopened.admit_check(request()).is_err(),
        "an incomplete record grants no check target"
    );
    assert_eq!(
        std::fs::read(path).unwrap(),
        bytes,
        "refusal never reconstructs the record"
    );
}

#[test]
fn editing_an_inflight_objective_keeps_its_run_and_admission() {
    let temp = tempfile::tempdir().unwrap();
    let cases = stored(temp.path());
    planned(&cases);
    let admission = cases.admit_check(request()).unwrap();
    cases
        .set_objective(
            "Clarified objective".into(),
            vec![],
            vec![],
            "chunk:test".into(),
        )
        .unwrap();
    assert_eq!(cases.admit_task(None).unwrap(), admission.target);
    assert!(matches!(
        cases.settle_check(admission, outcome()).unwrap(),
        CheckSettlement::Applied { .. }
    ));
}

#[test]
fn invalid_check_arguments_are_refused_before_task_admission() {
    let cases = ShipUseCases::new(Arc::new(Mutex::new(ShipEngine::new("empty".into()))));
    let mut invalid = request();
    invalid.command = None;
    let refused = cases.admit_check(invalid).unwrap_err();
    assert_eq!(refused.kind, "invalid_args");
    assert!(refused.message.contains("`passed` is required"));
}

#[tokio::test]
async fn approval_defaults_and_refusals_match_the_mcp_adapter() {
    use magistr_workflow::ship::application::ApprovalGateRequest;
    use magistr_workflow::ship::mcp::handlers::{GateOpenArgs, GateWaitArgs};

    let cases = ShipUseCases::new(Arc::new(Mutex::new(ShipEngine::new("headless".into()))));
    let service = ShipService::from_use_cases(cases.clone());
    let request = ApprovalGateRequest {
        question: "Proceed?".into(),
        body: String::new(),
        options: None,
        default_key: Some("no".into()),
        timeout_secs: None,
    };
    let typed = cases.open_gate(request.clone()).await.unwrap();
    let wire = service
        .ship_gate_open(Parameters(GateOpenArgs {
            question: request.question,
            body: request.body,
            options: Value::Null,
            default_key: request.default_key,
            timeout_secs: request.timeout_secs,
        }))
        .await
        .unwrap();
    assert_eq!(
        wire.structured_content.unwrap(),
        serde_json::to_value(typed).unwrap()
    );
    let typed_error = cases.wait_gate("", None).await.unwrap_err();
    let wire_error = service
        .ship_gate_wait(Parameters(GateWaitArgs {
            gate_id: String::new(),
            wait_secs: None,
        }))
        .await
        .unwrap()
        .structured_content
        .unwrap();
    assert_eq!(wire_error["error_kind"], typed_error.kind);
    assert_eq!(wire_error["message"], typed_error.message);
}

#[test]
fn switching_scopes_restores_the_exact_unfinished_plan_across_restart() {
    let temp = tempfile::tempdir().unwrap();
    let cases = stored(temp.path());
    planned(&cases);
    let admitted = cases.admit_check(request()).unwrap();
    cases.settle_check(admitted, outcome()).unwrap();
    cases
        .block_task("verify", "Waiting for an answer".into())
        .unwrap();
    let original = serde_json::to_value(cases.snapshot().unwrap()).unwrap();

    cases
        .set_objective("Other work".into(), vec![], vec![], "chunk:other".into())
        .unwrap();
    assert!(cases.snapshot().unwrap().tasks.is_empty());
    cases
        .add_task(
            "verify".into(),
            "Other verification".into(),
            TaskType::Test,
            None,
            None,
        )
        .unwrap();
    cases.start_task("verify").unwrap();
    let other = serde_json::to_value(cases.snapshot().unwrap()).unwrap();

    let reopened = stored(temp.path());
    reopened
        .set_objective("Current run".into(), vec![], vec![], "chunk:test".into())
        .unwrap();
    assert_eq!(
        serde_json::to_value(reopened.snapshot().unwrap()).unwrap(),
        original
    );
    reopened
        .set_objective("Other work".into(), vec![], vec![], "chunk:other".into())
        .unwrap();
    assert_eq!(
        serde_json::to_value(reopened.snapshot().unwrap()).unwrap(),
        other
    );
}

#[tokio::test]
async fn mcp_scope_recovery_preserves_blocked_work_and_saved_objective_metadata() {
    use magistr_workflow::ship::domain::objective::VerificationMode;
    use magistr_workflow::ship::mcp::handlers::SetObjectiveArgs;

    let temp = tempfile::tempdir().unwrap();
    let cases = stored(temp.path());
    cases
        .set_objective_with_verification(
            "Finish the benchmark".into(),
            vec!["Record every trial".into()],
            vec!["Preserve original evidence".into()],
            "benchmark".into(),
            VerificationMode::Focused,
        )
        .unwrap();
    cases
        .add_task(
            "trials".into(),
            "Run trials".into(),
            TaskType::Test,
            None,
            None,
        )
        .unwrap();
    cases.start_task("trials").unwrap();
    let admitted = cases.admit_check(request()).unwrap();
    cases.settle_check(admitted, outcome()).unwrap();
    cases
        .block_task("trials", "Waiting for credentials".into())
        .unwrap();
    let benchmark = serde_json::to_value(cases.snapshot().unwrap()).unwrap();

    cases
        .set_objective_with_verification(
            "Read selected records".into(),
            vec!["Show all recorded content".into()],
            vec![],
            "chunk:rich-records".into(),
            VerificationMode::Focused,
        )
        .unwrap();
    cases
        .add_task(
            "render".into(),
            "Render records".into(),
            TaskType::Implement,
            None,
            None,
        )
        .unwrap();
    cases.start_task("render").unwrap();
    let selected = serde_json::to_value(cases.snapshot().unwrap()).unwrap();

    // Recovery arguments seed only a new scope, never replace a saved objective.
    let reopened = stored(temp.path());
    let service = ShipService::from_use_cases(reopened.clone());
    for (scope, expected) in [("benchmark", benchmark), ("chunk:rich-records", selected)] {
        let answer = service
            .ship_set_objective(Parameters(SetObjectiveArgs {
                description: "Newly supplied description".into(),
                acceptance_criteria: vec!["Newly supplied criterion".into()],
                constraints: vec!["Newly supplied constraint".into()],
                scope: scope.into(),
                verification: VerificationMode::Full,
            }))
            .await
            .unwrap()
            .structured_content
            .unwrap();
        assert_eq!(answer, expected["objective"]);
        assert_eq!(
            serde_json::to_value(reopened.snapshot().unwrap()).unwrap(),
            expected
        );
        assert_eq!(
            serde_json::to_value(stored(temp.path()).snapshot().unwrap()).unwrap(),
            expected
        );
    }
}

#[test]
fn stale_writers_cannot_reselect_a_saved_plan_or_erase_its_latest_tasks() {
    let temp = tempfile::tempdir().unwrap();
    let current = stored(temp.path());
    planned(&current);
    let mut stale =
        ShipEngine::new("project".into()).with_persistence(Persistence::new(&PersistenceConfig {
            enabled: true,
            data_dir: temp.path().into(),
        }));
    current
        .add_task(
            "latest".into(),
            "Latest acknowledged task".into(),
            TaskType::Implement,
            None,
            None,
        )
        .unwrap();
    current
        .set_objective("Other".into(), vec![], vec![], "chunk:other".into())
        .unwrap();
    stale.add_task(
        "stale".into(),
        "Stale mutation".into(),
        TaskType::Implement,
        None,
        None,
    );
    let reopened = stored(temp.path());
    assert_eq!(
        reopened.snapshot().unwrap().objective.unwrap().scope,
        "chunk:other"
    );
    reopened
        .set_objective("Current run".into(), vec![], vec![], "chunk:test".into())
        .unwrap();
    let ids: Vec<_> = reopened
        .snapshot()
        .unwrap()
        .tasks
        .into_iter()
        .map(|task| task.id)
        .collect();
    assert_eq!(ids, ["verify", "latest"]);
}

#[test]
fn a_reset_run_cannot_be_replaced_by_a_stale_track_selection() {
    let temp = tempfile::tempdir().unwrap();
    let cases = stored(temp.path());
    planned(&cases);
    cases
        .set_objective("Other".into(), vec![], vec![], "chunk:other".into())
        .unwrap();
    let mut stale =
        ShipEngine::new("project".into()).with_persistence(Persistence::new(&PersistenceConfig {
            enabled: true,
            data_dir: temp.path().into(),
        }));
    cases.reset().unwrap();
    planned(&cases);
    let run = cases.snapshot().unwrap().run_id;
    stale.add_task(
        "stale".into(),
        "Stale task".into(),
        TaskType::Test,
        None,
        None,
    );
    let actual = stored(temp.path()).snapshot().unwrap();
    assert_eq!(actual.run_id, run);
    assert_eq!(actual.tasks.len(), 1);
}
