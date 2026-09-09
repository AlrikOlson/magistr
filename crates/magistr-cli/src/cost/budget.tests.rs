use super::*;
use crate::cost::budget_config::BudgetConfig;
use crate::cost::check_budget::check_budget;

#[test]
fn no_budget_always_ok() {
    let config = BudgetConfig::none();
    assert_eq!(check_budget(0.0, &config), BudgetStatus::Ok);
    assert_eq!(check_budget(100.0, &config), BudgetStatus::Ok);
}

#[test]
fn under_limit_is_ok() {
    let config = BudgetConfig::with_limit(5.0);
    let status = check_budget(3.0, &config);
    assert_eq!(status, BudgetStatus::Ok);
    assert!(!status.should_stop());
    assert!(!status.should_warn());
}

#[test]
fn at_limit_is_exceeded() {
    let config = BudgetConfig::with_limit(5.0);
    let status = check_budget(5.0, &config);
    assert!(matches!(
        status,
        BudgetStatus::Exceeded {
            overage,
            limit: 5.0
        } if overage.abs() < f64::EPSILON
    ));
    assert!(status.should_stop());
}

#[test]
fn over_limit_is_exceeded() {
    let config = BudgetConfig::with_limit(5.0);
    let status = check_budget(7.50, &config);
    assert!(matches!(
        status,
        BudgetStatus::Exceeded {
            overage,
            limit: 5.0
        } if (overage - 2.50).abs() < f64::EPSILON
    ));
}

#[test]
fn warning_threshold_triggers() {
    let config = BudgetConfig::with_limit_and_warn(5.0, 3.0);
    let status = check_budget(3.50, &config);
    assert!(matches!(
        status,
        BudgetStatus::Warning { threshold: 3.0, .. }
    ));
    assert!(status.should_warn());
    assert!(!status.should_stop());
}

#[test]
fn warning_remaining_is_correct() {
    let config = BudgetConfig::with_limit_and_warn(5.0, 3.0);
    let status = check_budget(3.50, &config);
    if let BudgetStatus::Warning { remaining, .. } = status {
        assert!((remaining - 1.50).abs() < f64::EPSILON);
    } else {
        panic!("expected Warning status");
    }
}

#[test]
fn exceeded_takes_priority_over_warning() {
    let config = BudgetConfig::with_limit_and_warn(5.0, 3.0);
    let status = check_budget(6.0, &config);
    assert!(matches!(status, BudgetStatus::Exceeded { .. }));
}

#[test]
fn warn_only_without_limit() {
    let config = BudgetConfig {
        limit: None,
        warn_threshold: Some(3.0),
    };
    let status = check_budget(4.0, &config);
    assert!(matches!(
        status,
        BudgetStatus::Warning {
            remaining,
            threshold: 3.0,
        } if remaining.abs() < f64::EPSILON
    ));
}

#[test]
fn below_warning_is_ok() {
    let config = BudgetConfig::with_limit_and_warn(5.0, 3.0);
    let status = check_budget(2.50, &config);
    assert_eq!(status, BudgetStatus::Ok);
}
