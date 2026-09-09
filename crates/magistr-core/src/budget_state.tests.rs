use super::*;

#[test]
fn budget_state_default() {
    let state = BudgetState::default();
    assert!(state.limit.is_none());
    assert!(state.warn_threshold.is_none());
}

#[test]
fn check_no_limit() {
    let state = BudgetState::default();
    assert_eq!(state.check(100.0), BudgetStatus::Ok);
}

#[test]
fn check_under_limit() {
    let state = BudgetState {
        limit: Some(10.0),
        warn_threshold: Some(8.0),
    };
    assert_eq!(state.check(5.0), BudgetStatus::Ok);
}

#[test]
fn check_warning() {
    let state = BudgetState {
        limit: Some(10.0),
        warn_threshold: Some(8.0),
    };
    match state.check(9.0) {
        BudgetStatus::Warning { ratio } => {
            assert!((ratio - 0.9).abs() < f64::EPSILON);
        }
        other => panic!("expected Warning, got {other:?}"),
    }
}

#[test]
fn check_exceeded() {
    let state = BudgetState {
        limit: Some(10.0),
        warn_threshold: Some(8.0),
    };
    match state.check(12.0) {
        BudgetStatus::Exceeded { overage } => {
            assert!((overage - 2.0).abs() < f64::EPSILON);
        }
        other => panic!("expected Exceeded, got {other:?}"),
    }
}

#[test]
fn check_exactly_at_limit() {
    let state = BudgetState {
        limit: Some(10.0),
        warn_threshold: None,
    };
    match state.check(10.0) {
        BudgetStatus::Exceeded { overage } => {
            assert!((overage).abs() < f64::EPSILON);
        }
        other => panic!("expected Exceeded, got {other:?}"),
    }
}

#[test]
fn set_budget() {
    let mut state = BudgetState::default();
    state.set_budget(Some(50.0), Some(40.0));
    assert_eq!(state.limit, Some(50.0));
    assert_eq!(state.warn_threshold, Some(40.0));
}
