use super::*;

#[test]
fn only_a_clean_exit_counts_as_passed() {
    assert!(CheckVerdict::Passed.passed());
    assert!(!CheckVerdict::Failed.passed());
    assert!(!CheckVerdict::TimedOut.passed());
    assert!(!CheckVerdict::Cancelled.passed());
}

#[test]
fn a_timeout_and_a_cancel_are_not_a_failure() {
    // The whole point of the enum: check_job used to fold all three into
    // one bool, so a check killed at 300s read exactly like a red test.
    assert_ne!(CheckVerdict::TimedOut, CheckVerdict::Failed);
    assert_ne!(CheckVerdict::Cancelled, CheckVerdict::Failed);
    assert_ne!(CheckVerdict::Cancelled, CheckVerdict::TimedOut);
}

#[test]
fn a_cancelled_check_is_not_evidence_of_anything() {
    // Nothing consequential — a pause, a reopen — may rest on a check the
    // cockpit's own quit ended.
    assert!(!CheckVerdict::Cancelled.is_evidence());
    assert!(CheckVerdict::Passed.is_evidence());
    assert!(CheckVerdict::Failed.is_evidence());
    assert!(CheckVerdict::TimedOut.is_evidence());
}

#[test]
fn every_verdict_has_a_label() {
    for (verdict, label) in [
        (CheckVerdict::Passed, "passed"),
        (CheckVerdict::Failed, "failed"),
        (CheckVerdict::TimedOut, "timed out"),
        (CheckVerdict::Cancelled, "cancelled"),
    ] {
        assert_eq!(verdict.label(), label);
    }
}
