use super::*;
use crate::cost::types::TokenBreakdown;
use chrono::{Duration, Utc};
use magistr_engine::cost::{BudgetPeriod, BudgetRule, BudgetScope};

fn entry(project: &str, actor: Option<&str>, cost: f64, days_ago: i64) -> CostEntry {
    CostEntry {
        session_id: "s".into(),
        project: project.into(),
        timestamp: Utc::now() - Duration::days(days_ago),
        cost_usd: Some(cost),
        tokens: TokenBreakdown::default(),
        turns: 1,
        duration_ms: 1,
        actor: actor.map(String::from),
        model: None,
    }
}

#[test]
fn a_policy_with_no_rules_needs_no_spend() {
    let spent = spend_by_scope(
        &[entry("magistr", None, 5.0, 0)],
        &BudgetPolicy::default(),
        &SpendContext::for_project("magistr"),
        Utc::now(),
    );
    assert!(spent.is_empty());
}

#[test]
fn each_rule_gets_the_spend_of_its_own_scope_and_window() {
    let entries = vec![
        entry("magistr", Some("ada"), 3.0, 0),
        entry("other", Some("ada"), 7.0, 0),
    ];
    let policy = BudgetPolicy::new(vec![
        BudgetRule::new(BudgetScope::Org, BudgetPeriod::Monthly).limit(100.0),
        BudgetRule::new(BudgetScope::Project, BudgetPeriod::Monthly).limit(50.0),
    ]);
    let now = Utc::now();
    let spent = spend_by_scope(
        &entries,
        &policy,
        &SpendContext::for_project("magistr"),
        now,
    );
    assert_eq!(spent[&(BudgetScope::Org, BudgetPeriod::Monthly)], 10.0);
    assert_eq!(spent[&(BudgetScope::Project, BudgetPeriod::Monthly)], 3.0);
}

#[test]
fn spend_outside_the_window_does_not_count() {
    let entries = vec![
        entry("magistr", None, 4.0, 0),
        entry("magistr", None, 100.0, 40),
    ];
    let policy = BudgetPolicy::new(vec![
        BudgetRule::new(BudgetScope::Org, BudgetPeriod::Daily).limit(10.0),
    ]);
    let spent = spend_by_scope(
        &entries,
        &policy,
        &SpendContext::for_project("magistr"),
        Utc::now(),
    );
    assert_eq!(spent[&(BudgetScope::Org, BudgetPeriod::Daily)], 4.0);
}

#[test]
fn a_session_scoped_rule_starts_from_zero() {
    let entries = vec![entry("magistr", None, 99.0, 0)];
    let policy = BudgetPolicy::new(vec![
        BudgetRule::new(BudgetScope::Session, BudgetPeriod::Session).limit(5.0),
    ]);
    let spent = spend_by_scope(
        &entries,
        &policy,
        &SpendContext::for_project("magistr"),
        Utc::now(),
    );
    assert_eq!(spent[&(BudgetScope::Session, BudgetPeriod::Session)], 0.0);
}

#[test]
fn a_rule_with_no_limit_or_warning_needs_no_spend() {
    let policy = BudgetPolicy::new(vec![BudgetRule::new(
        BudgetScope::Org,
        BudgetPeriod::Monthly,
    )]);
    let spent = spend_by_scope(
        &[entry("magistr", None, 5.0, 0)],
        &policy,
        &SpendContext::for_project("magistr"),
        Utc::now(),
    );
    assert!(spent.is_empty());
}
