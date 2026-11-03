use super::*;

#[test]
fn check_result_passed() {
    let result = CheckReport {
        name: "test".into(),
        passed: true,
        duration_secs: 1.5,
        output: Some("42 tests passed".into()),
        test_count: Some(42),
        rows: Vec::new(),
    };
    assert!(result.passed);
    assert_eq!(result.name, "test");
    assert_eq!(result.test_count, Some(42));
}

#[test]
fn check_result_failed() {
    let result = CheckReport {
        name: "lint".into(),
        passed: false,
        duration_secs: 0.8,
        output: Some("error[E0308]: mismatched types".into()),
        test_count: None,
        rows: Vec::new(),
    };
    assert!(!result.passed);
    assert!(result.test_count.is_none());
}

#[test]
fn check_summary_compact() {
    let summary = CheckBrief {
        name: "check".into(),
        passed: true,
        duration_secs: 2.3,
    };
    assert!(summary.passed);
    assert_eq!(summary.name, "check");
}

#[test]
fn iteration_outcome_eq() {
    assert_eq!(IterationOutcome::Passed, IterationOutcome::Passed);
    assert_ne!(IterationOutcome::Passed, IterationOutcome::Failed);
}

#[test]
fn iteration_phase_labels() {
    assert_eq!(IterationPhase::Research.label(), "research");
    assert_eq!(IterationPhase::Plan.label(), "plan");
    assert_eq!(IterationPhase::Execute.label(), "execute");
    assert_eq!(IterationPhase::Verify.label(), "verify");
    assert_eq!(IterationPhase::Check.label(), "check");
}

#[test]
fn check_history_max_constant() {
    assert_eq!(CHECK_HISTORY_MAX, 10);
}
