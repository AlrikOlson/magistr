use super::*;

#[test]
fn a_rule_defaults_to_the_session_period() {
    let json = r#"{"scope":"team","limit":25.0}"#;
    let rule: BudgetRule = serde_json::from_str(json).expect("deserialize");
    assert_eq!(rule.scope, BudgetScope::Team);
    assert_eq!(rule.period, BudgetPeriod::Session);
    assert_eq!(rule.limit, Some(25.0));
    assert_eq!(rule.warn_threshold, None);
}

#[test]
fn round_trips_through_serde() {
    let rule = BudgetRule::new(BudgetScope::Org, BudgetPeriod::Monthly).limit(1000.0);
    let json = serde_json::to_string(&rule).expect("serialize");
    let back: BudgetRule = serde_json::from_str(&json).expect("deserialize");
    assert_eq!(back, rule);
}

#[test]
fn the_builder_sets_both_thresholds() {
    let rule = BudgetRule::new(BudgetScope::Project, BudgetPeriod::Daily)
        .limit(10.0)
        .warn_threshold(8.0);
    assert_eq!(rule.limit, Some(10.0));
    assert_eq!(rule.warn_threshold, Some(8.0));
}

#[test]
fn an_empty_rule_serializes_without_null_thresholds() {
    let rule = BudgetRule::new(BudgetScope::Session, BudgetPeriod::Session);
    let json = serde_json::to_string(&rule).expect("serialize");
    assert_eq!(json, r#"{"scope":"session","period":"session"}"#);
}
