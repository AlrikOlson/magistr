use alloc::format;

use super::*;

#[test]
fn check_kind_labels() {
    assert_eq!(StandardCheckKind::Check.label(), "Check");
    assert_eq!(StandardCheckKind::Test.label(), "Test");
    assert_eq!(StandardCheckKind::Lint.label(), "Lint");
}

#[test]
fn check_kind_display() {
    assert_eq!(format!("{}", StandardCheckKind::Check), "Check");
}

#[test]
fn check_status_labels() {
    assert_eq!(CheckState::Pending.label(), "pending");
    assert_eq!(CheckState::Running.label(), "running");
    assert_eq!(CheckState::Passed.label(), "passed");
    assert_eq!(CheckState::Failed.label(), "failed");
    assert_eq!(CheckState::Cached.label(), "cached");
}

#[test]
fn check_status_icons() {
    assert_eq!(CheckState::Pending.icon(), "○");
    assert_eq!(CheckState::Running.icon(), "●");
    assert_eq!(CheckState::Passed.icon(), "✓");
    assert_eq!(CheckState::Failed.icon(), "✗");
    assert_eq!(CheckState::Cached.icon(), "◎");
}

#[test]
fn check_status_terminal() {
    assert!(!CheckState::Pending.is_terminal());
    assert!(!CheckState::Running.is_terminal());
    assert!(CheckState::Passed.is_terminal());
    assert!(CheckState::Failed.is_terminal());
    assert!(CheckState::Cached.is_terminal());
}

#[test]
fn check_outcome_passed() {
    let outcome = StandardCheckOutcome {
        kind: StandardCheckKind::Check,
        status: CheckState::Passed,
        output: None,
        duration_ms: Some(1200),
    };
    assert!(outcome.passed());
}

#[test]
fn check_outcome_cached_counts_as_passed() {
    let outcome = StandardCheckOutcome {
        kind: StandardCheckKind::Test,
        status: CheckState::Cached,
        output: None,
        duration_ms: None,
    };
    assert!(outcome.passed());
}

#[test]
fn check_outcome_failed() {
    let outcome = StandardCheckOutcome {
        kind: StandardCheckKind::Lint,
        status: CheckState::Failed,
        output: Some("error: unused variable".into()),
        duration_ms: Some(500),
    };
    assert!(!outcome.passed());
}

#[test]
fn check_kind_serialization_roundtrip() {
    let kind = StandardCheckKind::Test;
    let json = serde_json::to_string(&kind).unwrap();
    assert_eq!(json, "\"test\"");
    let parsed: StandardCheckKind = serde_json::from_str(&json).unwrap();
    assert_eq!(kind, parsed);
}

#[test]
fn check_status_serialization_roundtrip() {
    let status = CheckState::Passed;
    let json = serde_json::to_string(&status).unwrap();
    assert_eq!(json, "\"passed\"");
    let parsed: CheckState = serde_json::from_str(&json).unwrap();
    assert_eq!(status, parsed);
}

#[test]
fn check_outcome_serialization_roundtrip() {
    let outcome = StandardCheckOutcome {
        kind: StandardCheckKind::Check,
        status: CheckState::Failed,
        output: Some("error output".into()),
        duration_ms: Some(3000),
    };
    let json = serde_json::to_string(&outcome).unwrap();
    let parsed: StandardCheckOutcome = serde_json::from_str(&json).unwrap();
    assert_eq!(outcome, parsed);
}
