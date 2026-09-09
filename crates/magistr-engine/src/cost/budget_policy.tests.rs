use super::*;
use crate::cost::{BudgetPeriod, BudgetScope};

fn spend(pairs: &[(BudgetScope, BudgetPeriod, f64)]) -> ScopeSpend {
    pairs
        .iter()
        .map(|(s, p, v)| ((*s, *p), *v))
        .collect::<ScopeSpend>()
}

#[test]
fn an_empty_policy_leaves_the_local_budget_alone() {
    let policy = BudgetPolicy::default();
    let local = BudgetConfig::with_limit_and_warn(5.0, 3.0);
    let resolved = policy.resolve(&local, &ScopeSpend::new());
    assert_eq!(resolved.effective, local);
    assert_eq!(resolved.binding, None);
    assert!(resolved.headroom.is_empty());
}

#[test]
fn a_rule_with_no_local_budget_becomes_the_effective_limit() {
    let policy = BudgetPolicy::new(vec![
        BudgetRule::new(BudgetScope::Team, BudgetPeriod::Monthly).limit(100.0),
    ]);
    let resolved = policy.resolve(&BudgetConfig::none(), &ScopeSpend::new());
    assert_eq!(resolved.effective.limit, Some(100.0));
    assert_eq!(resolved.binding, Some(BudgetScope::Team));
}

#[test]
fn the_narrowest_headroom_wins_across_scopes() {
    let policy = BudgetPolicy::new(vec![
        BudgetRule::new(BudgetScope::Org, BudgetPeriod::Monthly).limit(1000.0),
        BudgetRule::new(BudgetScope::Team, BudgetPeriod::Monthly).limit(200.0),
        BudgetRule::new(BudgetScope::Project, BudgetPeriod::Daily).limit(20.0),
    ]);
    let resolved = policy.resolve(&BudgetConfig::none(), &ScopeSpend::new());
    assert_eq!(resolved.effective.limit, Some(20.0));
    assert_eq!(resolved.binding, Some(BudgetScope::Project));
    assert_eq!(resolved.headroom.len(), 3);
}

#[test]
fn already_spent_money_shrinks_the_headroom_it_was_spent_against() {
    let policy = BudgetPolicy::new(vec![
        BudgetRule::new(BudgetScope::Org, BudgetPeriod::Monthly).limit(1000.0),
        BudgetRule::new(BudgetScope::Team, BudgetPeriod::Monthly).limit(200.0),
    ]);
    // The team has burned 195 of its 200 this month; the org is barely touched.
    let spent = spend(&[
        (BudgetScope::Org, BudgetPeriod::Monthly, 400.0),
        (BudgetScope::Team, BudgetPeriod::Monthly, 195.0),
    ]);
    let resolved = policy.resolve(&BudgetConfig::none(), &spent);
    assert_eq!(resolved.effective.limit, Some(5.0));
    assert_eq!(resolved.binding, Some(BudgetScope::Team));
}

#[test]
fn a_local_flag_can_tighten_the_policy() {
    let policy = BudgetPolicy::new(vec![
        BudgetRule::new(BudgetScope::Team, BudgetPeriod::Monthly).limit(100.0),
    ]);
    let resolved = policy.resolve(&BudgetConfig::with_limit(5.0), &ScopeSpend::new());
    assert_eq!(resolved.effective.limit, Some(5.0));
    // Local bound it, so no scope is named as the binding constraint.
    assert_eq!(resolved.binding, None);
}

#[test]
fn a_local_flag_can_never_loosen_the_policy() {
    let policy = BudgetPolicy::new(vec![
        BudgetRule::new(BudgetScope::Team, BudgetPeriod::Monthly).limit(10.0),
    ]);
    // A developer asks for a 10_000 dollar session.
    let resolved = policy.resolve(&BudgetConfig::with_limit(10_000.0), &ScopeSpend::new());
    assert_eq!(resolved.effective.limit, Some(10.0));
    assert_eq!(resolved.binding, Some(BudgetScope::Team));
}

#[test]
fn a_ties_goes_to_the_local_budget() {
    let policy = BudgetPolicy::new(vec![
        BudgetRule::new(BudgetScope::Team, BudgetPeriod::Monthly).limit(10.0),
    ]);
    let resolved = policy.resolve(&BudgetConfig::with_limit(10.0), &ScopeSpend::new());
    assert_eq!(resolved.effective.limit, Some(10.0));
    assert_eq!(resolved.binding, None);
}

#[test]
fn an_exhausted_scope_resolves_to_a_zero_limit() {
    let policy = BudgetPolicy::new(vec![
        BudgetRule::new(BudgetScope::Project, BudgetPeriod::Daily).limit(20.0),
    ]);
    let spent = spend(&[(BudgetScope::Project, BudgetPeriod::Daily, 25.0)]);
    let resolved = policy.resolve(&BudgetConfig::none(), &spent);
    assert_eq!(resolved.effective.limit, Some(0.0));
    assert!(resolved.is_exhausted());
}

#[test]
fn two_periods_on_one_scope_are_accounted_separately() {
    let policy = BudgetPolicy::new(vec![
        BudgetRule::new(BudgetScope::Team, BudgetPeriod::Daily).limit(50.0),
        BudgetRule::new(BudgetScope::Team, BudgetPeriod::Monthly).limit(500.0),
    ]);
    let spent = spend(&[
        (BudgetScope::Team, BudgetPeriod::Daily, 45.0),
        (BudgetScope::Team, BudgetPeriod::Monthly, 300.0),
    ]);
    let resolved = policy.resolve(&BudgetConfig::none(), &spent);
    assert_eq!(resolved.effective.limit, Some(5.0));
    assert_eq!(resolved.headroom.len(), 2);
}

#[test]
fn warn_thresholds_take_the_tightest_too() {
    let policy = BudgetPolicy::new(vec![
        BudgetRule::new(BudgetScope::Team, BudgetPeriod::Monthly)
            .limit(100.0)
            .warn_threshold(80.0),
    ]);
    let spent = spend(&[(BudgetScope::Team, BudgetPeriod::Monthly, 75.0)]);
    let resolved = policy.resolve(&BudgetConfig::none(), &spent);
    // 5 dollars of this session's spend gets the team to its warning line.
    assert_eq!(resolved.effective.warn_threshold, Some(5.0));
    assert_eq!(resolved.effective.limit, Some(25.0));
}

#[test]
fn a_rule_without_a_limit_contributes_no_headroom() {
    let policy = BudgetPolicy::new(vec![
        BudgetRule::new(BudgetScope::Org, BudgetPeriod::Monthly).warn_threshold(500.0),
    ]);
    let resolved = policy.resolve(&BudgetConfig::none(), &ScopeSpend::new());
    assert!(resolved.headroom.is_empty());
    assert_eq!(resolved.effective.limit, None);
    assert_eq!(resolved.effective.warn_threshold, Some(500.0));
}

#[test]
fn the_local_loop_bounds_survive_resolution() {
    let policy = BudgetPolicy::new(vec![
        BudgetRule::new(BudgetScope::Team, BudgetPeriod::Monthly).limit(10.0),
    ]);
    let local = BudgetConfig::none()
        .max_iterations(7)
        .max_duration_secs(600);
    let resolved = policy.resolve(&local, &ScopeSpend::new());
    assert_eq!(resolved.effective.max_iterations, Some(7));
    assert_eq!(resolved.effective.max_duration_secs, Some(600));
}

#[test]
fn headroom_is_reported_broadest_scope_first() {
    let policy = BudgetPolicy::new(vec![
        BudgetRule::new(BudgetScope::Session, BudgetPeriod::Session).limit(2.0),
        BudgetRule::new(BudgetScope::Org, BudgetPeriod::Monthly).limit(900.0),
        BudgetRule::new(BudgetScope::Project, BudgetPeriod::Daily).limit(20.0),
    ]);
    let resolved = policy.resolve(&BudgetConfig::none(), &ScopeSpend::new());
    let scopes: Vec<BudgetScope> = resolved.headroom.iter().map(|h| h.scope).collect();
    assert_eq!(
        scopes,
        vec![BudgetScope::Org, BudgetScope::Project, BudgetScope::Session]
    );
}

#[test]
fn is_set_reports_whether_the_policy_carries_any_rule() {
    assert!(!BudgetPolicy::default().is_set());
    assert!(
        BudgetPolicy::new(vec![BudgetRule::new(
            BudgetScope::Org,
            BudgetPeriod::Monthly
        )])
        .is_set()
    );
}

#[test]
fn the_policy_round_trips_through_serde() {
    let policy = BudgetPolicy::new(vec![
        BudgetRule::new(BudgetScope::Org, BudgetPeriod::Monthly).limit(1000.0),
    ]);
    let json = serde_json::to_string(&policy).expect("serialize");
    let back: BudgetPolicy = serde_json::from_str(&json).expect("deserialize");
    assert_eq!(back, policy);
}
