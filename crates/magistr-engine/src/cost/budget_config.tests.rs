use super::*;

#[test]
fn none_has_no_constraints() {
    let bc = BudgetConfig::none();
    assert!(!bc.is_set());
    assert!(bc.limit.is_none());
    assert!(bc.warn_threshold.is_none());
    assert!(bc.max_iterations.is_none());
    assert!(bc.max_duration_secs.is_none());
}

#[test]
fn default_is_none() {
    assert_eq!(BudgetConfig::default(), BudgetConfig::none());
}

#[test]
fn with_limit_sets_limit_only() {
    let bc = BudgetConfig::with_limit(5.0);
    assert!(bc.is_set());
    assert_eq!(bc.limit, Some(5.0));
    assert!(bc.warn_threshold.is_none());
}

#[test]
fn with_limit_and_warn() {
    let bc = BudgetConfig::with_limit_and_warn(10.0, 8.0);
    assert!(bc.is_set());
    assert_eq!(bc.limit, Some(10.0));
    assert_eq!(bc.warn_threshold, Some(8.0));
}

#[test]
fn builder_chain() {
    let bc = BudgetConfig::none()
        .limit(5.0)
        .warn_threshold(4.0)
        .max_iterations(10)
        .max_duration_secs(3600);
    assert!(bc.is_set());
    assert_eq!(bc.limit, Some(5.0));
    assert_eq!(bc.warn_threshold, Some(4.0));
    assert_eq!(bc.max_iterations, Some(10));
    assert_eq!(bc.max_duration_secs, Some(3600));
}

#[test]
fn is_set_with_only_iterations() {
    let bc = BudgetConfig::none().max_iterations(5);
    assert!(bc.is_set());
}

#[test]
fn is_set_with_only_duration() {
    let bc = BudgetConfig::none().max_duration_secs(600);
    assert!(bc.is_set());
}

#[test]
fn serde_roundtrip() {
    let bc = BudgetConfig::with_limit_and_warn(10.0, 8.0)
        .max_iterations(25)
        .max_duration_secs(7200);
    let json = serde_json::to_string(&bc).unwrap();
    let back: BudgetConfig = serde_json::from_str(&json).unwrap();
    assert_eq!(bc, back);
}

#[test]
fn serde_none_omits_optional_fields() {
    let bc = BudgetConfig::with_limit(5.0);
    let json = serde_json::to_string(&bc).unwrap();
    assert!(!json.contains("max_iterations"));
    assert!(!json.contains("max_duration_secs"));
}
