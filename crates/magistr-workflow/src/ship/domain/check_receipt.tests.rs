use super::*;
use crate::ship::domain::receipt_stage::ReceiptStage;

fn legacy(settlement: bool) -> serde_json::Value {
    let mut value = serde_json::json!({
        "id": "legacy",
        "target": {
            "project_id": "project",
            "execution": null,
            "run_id": "older",
            "task_id": "verify",
            "task_instance": "older-task"
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
fn a_pre_staged_settled_receipt_reads_as_settled_with_its_execution_as_outcome() {
    let receipt: CheckReceipt = serde_json::from_value(legacy(true)).unwrap();
    assert_eq!(receipt.stage, ReceiptStage::Settled);
    assert_eq!(
        receipt.execution(),
        Some(&CheckExecution::Exited { code: 1 })
    );
    assert!(!receipt.passed() && receipt.verified() && receipt.is_applied());
    let outcome = receipt.outcome.as_ref().unwrap();
    assert_eq!(outcome.exit_code, Some(1));
    assert_eq!(outcome.command.as_deref(), Some("cargo test"));
    assert_eq!(receipt.recorded_at, receipt.settled_at);
    assert_eq!(receipt.duration_ms, Some(13704));
}

#[test]
fn a_pre_staged_unsettled_result_reads_as_recorded() {
    let receipt: CheckReceipt = serde_json::from_value(legacy(false)).unwrap();
    assert_eq!(receipt.stage, ReceiptStage::Recorded);
    assert!(receipt.outcome.is_some() && receipt.recorded_at.is_none());
    assert!(receipt.settlement.is_none() && receipt.settled_at.is_none());
}

#[test]
fn a_current_receipt_round_trips_unchanged() {
    let mut legacy = legacy(true);
    legacy["stage"] = "settled".into();
    legacy["recorded_at"] = "2026-10-14T00:47:29+00:00".into();
    legacy["outcome"] = serde_json::json!({
        "passed": false, "details": "boom", "verified": true, "command": "cargo test",
        "exit_code": 1, "execution": {"kind": "exited", "code": 1}, "report": null, "results": null
    });
    legacy.as_object_mut().unwrap().remove("execution");
    let receipt: CheckReceipt = serde_json::from_value(legacy).unwrap();
    assert_eq!(
        receipt.recorded_at.as_deref(),
        Some("2026-10-14T00:47:29+00:00")
    );
    assert_eq!(receipt.outcome.as_ref().unwrap().details, "boom");
    let again: CheckReceipt =
        serde_json::from_slice(&serde_json::to_vec(&receipt).unwrap()).unwrap();
    assert_eq!(again, receipt);
}
