use super::*;
use crate::run::domain::check_result_stage::CheckResultStage;

fn legacy(settlement: bool) -> serde_json::Value {
    let mut value = serde_json::json!({
        "id": "legacy",
        "target": {
            "project_id": "project",
            "execution": null,
            "run_id": "older",
            "step_id": "verify",
            "step_instance": "older-step"
        },
        "attempt": 1,
        "gate": "test",
        "check_type": "test",
        "command": "cargo test",
        "required": true,
        "acceptance_revision": "acceptance",
        "source": null,
        "admitted_at": "2026-10-14T00:47:16.891016+00:00",
        "duration_ms": 13704,
        "execution": {"kind": "exited", "code": 1},
        "freshness": {"freshness": "unchanged"}
    });
    if settlement {
        value["settled_at"] = "2026-10-14T00:47:30.595621+00:00".into();
        value["settlement"] = serde_json::json!({"settlement": "applied"});
    }
    value
}

#[test]
fn a_pre_staged_settled_check_result_reads_as_settled_with_its_execution_as_outcome() {
    let check_result: CheckResult = serde_json::from_value(legacy(true)).unwrap();
    assert_eq!(check_result.stage, CheckResultStage::Settled);
    assert_eq!(
        check_result.execution(),
        Some(&CheckExecution::Exited { code: 1 })
    );
    assert!(!check_result.passed() && check_result.verified() && check_result.is_applied());
    let outcome = check_result.outcome.as_ref().unwrap();
    assert_eq!(outcome.exit_code, Some(1));
    assert_eq!(outcome.command.as_deref(), Some("cargo test"));
    assert_eq!(check_result.recorded_at, check_result.settled_at);
    assert_eq!(check_result.duration_ms, Some(13704));
}

#[test]
fn a_pre_staged_unsettled_result_reads_as_recorded() {
    let check_result: CheckResult = serde_json::from_value(legacy(false)).unwrap();
    assert_eq!(check_result.stage, CheckResultStage::Recorded);
    assert!(check_result.outcome.is_some() && check_result.recorded_at.is_none());
    assert!(check_result.settlement.is_none() && check_result.settled_at.is_none());
}

#[test]
fn a_current_check_result_round_trips_unchanged() {
    let mut legacy = legacy(true);
    legacy["stage"] = "settled".into();
    legacy["recorded_at"] = "2026-10-14T00:47:29+00:00".into();
    legacy["outcome"] = serde_json::json!({
        "passed": false, "details": "boom", "verified": true, "command": "cargo test",
        "exit_code": 1, "execution": {"kind": "exited", "code": 1}, "report": null, "results": null
    });
    legacy.as_object_mut().unwrap().remove("execution");
    let check_result: CheckResult = serde_json::from_value(legacy).unwrap();
    assert_eq!(
        check_result.recorded_at.as_deref(),
        Some("2026-10-14T00:47:29+00:00")
    );
    assert_eq!(check_result.outcome.as_ref().unwrap().details, "boom");
    let again: CheckResult =
        serde_json::from_slice(&serde_json::to_vec(&check_result).unwrap()).unwrap();
    assert_eq!(again, check_result);
}
