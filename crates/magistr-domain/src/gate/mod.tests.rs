use alloc::format;

use super::*;

#[test]
fn gate_kind_labels() {
    assert_eq!(GateKind::Check.label(), "Check");
    assert_eq!(GateKind::Test.label(), "Test");
    assert_eq!(GateKind::Lint.label(), "Lint");
}

#[test]
fn gate_kind_display() {
    assert_eq!(format!("{}", GateKind::Check), "Check");
}

#[test]
fn gate_status_labels() {
    assert_eq!(GateStatus::Pending.label(), "pending");
    assert_eq!(GateStatus::Running.label(), "running");
    assert_eq!(GateStatus::Passed.label(), "passed");
    assert_eq!(GateStatus::Failed.label(), "failed");
    assert_eq!(GateStatus::Cached.label(), "cached");
}

#[test]
fn gate_status_icons() {
    assert_eq!(GateStatus::Pending.icon(), "○");
    assert_eq!(GateStatus::Running.icon(), "●");
    assert_eq!(GateStatus::Passed.icon(), "✓");
    assert_eq!(GateStatus::Failed.icon(), "✗");
    assert_eq!(GateStatus::Cached.icon(), "◎");
}

#[test]
fn gate_status_terminal() {
    assert!(!GateStatus::Pending.is_terminal());
    assert!(!GateStatus::Running.is_terminal());
    assert!(GateStatus::Passed.is_terminal());
    assert!(GateStatus::Failed.is_terminal());
    assert!(GateStatus::Cached.is_terminal());
}

#[test]
fn gate_outcome_passed() {
    let outcome = GateOutcome {
        kind: GateKind::Check,
        status: GateStatus::Passed,
        output: None,
        duration_ms: Some(1200),
    };
    assert!(outcome.passed());
}

#[test]
fn gate_outcome_cached_counts_as_passed() {
    let outcome = GateOutcome {
        kind: GateKind::Test,
        status: GateStatus::Cached,
        output: None,
        duration_ms: None,
    };
    assert!(outcome.passed());
}

#[test]
fn gate_outcome_failed() {
    let outcome = GateOutcome {
        kind: GateKind::Lint,
        status: GateStatus::Failed,
        output: Some("error: unused variable".into()),
        duration_ms: Some(500),
    };
    assert!(!outcome.passed());
}

#[test]
fn gate_kind_serialization_roundtrip() {
    let kind = GateKind::Test;
    let json = serde_json::to_string(&kind).unwrap();
    assert_eq!(json, "\"test\"");
    let parsed: GateKind = serde_json::from_str(&json).unwrap();
    assert_eq!(kind, parsed);
}

#[test]
fn gate_status_serialization_roundtrip() {
    let status = GateStatus::Passed;
    let json = serde_json::to_string(&status).unwrap();
    assert_eq!(json, "\"passed\"");
    let parsed: GateStatus = serde_json::from_str(&json).unwrap();
    assert_eq!(status, parsed);
}

#[test]
fn gate_outcome_serialization_roundtrip() {
    let outcome = GateOutcome {
        kind: GateKind::Check,
        status: GateStatus::Failed,
        output: Some("error output".into()),
        duration_ms: Some(3000),
    };
    let json = serde_json::to_string(&outcome).unwrap();
    let parsed: GateOutcome = serde_json::from_str(&json).unwrap();
    assert_eq!(outcome, parsed);
}
