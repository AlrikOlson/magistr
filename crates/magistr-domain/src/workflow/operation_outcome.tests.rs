use super::*;
use alloc::string::ToString;

const ALL: [OperationOutcome; 6] = [
    OperationOutcome::Accepted,
    OperationOutcome::Running,
    OperationOutcome::Completed,
    OperationOutcome::Failed,
    OperationOutcome::Cancelled,
    OperationOutcome::Indeterminate,
];

#[test]
fn settled_outcomes_latch_and_unsettled_ones_do_not() {
    for outcome in ALL {
        let settled = matches!(
            outcome,
            OperationOutcome::Completed | OperationOutcome::Failed | OperationOutcome::Cancelled
        );
        assert_eq!(outcome.is_settled(), settled, "{outcome}");
        for next in ALL {
            if settled {
                assert!(!outcome.can_settle_to(next), "{outcome} -> {next}");
                assert!(!outcome.can_reconcile_to(next), "{outcome} -> {next}");
            }
        }
    }
}

#[test]
fn a_live_owner_reports_forward_only() {
    let accepted = OperationOutcome::Accepted;
    assert!(!accepted.can_settle_to(OperationOutcome::Accepted));
    for next in ALL.into_iter().skip(1) {
        assert!(accepted.can_settle_to(next), "accepted -> {next}");
    }
    let running = OperationOutcome::Running;
    assert!(!running.can_settle_to(OperationOutcome::Accepted));
    assert!(!running.can_settle_to(OperationOutcome::Running));
    for next in ALL.into_iter().skip(2) {
        assert!(running.can_settle_to(next), "running -> {next}");
    }
}

#[test]
fn indeterminate_leaves_only_by_reconciliation_to_a_settled_outcome() {
    let lost = OperationOutcome::Indeterminate;
    for next in ALL {
        assert!(!lost.can_settle_to(next), "indeterminate settle -> {next}");
        assert_eq!(lost.can_reconcile_to(next), next.is_settled(), "{next}");
    }
    for outcome in ALL {
        if outcome != OperationOutcome::Indeterminate {
            assert!(!outcome.can_reconcile_to(OperationOutcome::Completed));
        }
    }
}

#[test]
fn wire_spelling_is_snake_case() {
    assert_eq!(
        serde_json::to_string(&OperationOutcome::Indeterminate).unwrap(),
        "\"indeterminate\""
    );
    assert_eq!(
        serde_json::from_str::<OperationOutcome>("\"cancelled\"").unwrap(),
        OperationOutcome::Cancelled
    );
    assert_eq!(OperationOutcome::Running.to_string(), "running");
}
