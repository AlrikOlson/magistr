use super::*;

#[test]
fn only_a_clean_exit_counts_as_passed() {
    assert!(GateVerdict::Passed.passed());
    assert!(!GateVerdict::Failed.passed());
    assert!(!GateVerdict::TimedOut.passed());
    assert!(!GateVerdict::Cancelled.passed());
}

#[test]
fn a_timeout_and_a_cancel_are_not_a_failure() {
    // The whole point of the enum: gate_job used to fold all three into
    // one bool, so a gate killed at 300s read exactly like a red test.
    assert_ne!(GateVerdict::TimedOut, GateVerdict::Failed);
    assert_ne!(GateVerdict::Cancelled, GateVerdict::Failed);
    assert_ne!(GateVerdict::Cancelled, GateVerdict::TimedOut);
}

#[test]
fn a_cancelled_gate_is_not_evidence_of_anything() {
    // Nothing consequential — a pause, a reopen — may rest on a gate the
    // cockpit's own quit ended.
    assert!(!GateVerdict::Cancelled.is_evidence());
    assert!(GateVerdict::Passed.is_evidence());
    assert!(GateVerdict::Failed.is_evidence());
    assert!(GateVerdict::TimedOut.is_evidence());
}

#[test]
fn every_verdict_has_a_label() {
    for (verdict, label) in [
        (GateVerdict::Passed, "passed"),
        (GateVerdict::Failed, "failed"),
        (GateVerdict::TimedOut, "timed out"),
        (GateVerdict::Cancelled, "cancelled"),
    ] {
        assert_eq!(verdict.label(), label);
    }
}
