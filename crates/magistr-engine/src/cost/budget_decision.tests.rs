use super::*;
use crate::cost::{BudgetConfig, BudgetPeriod, BudgetPolicy, BudgetRule, ScopeSpend};

fn policy() -> BudgetPolicy {
    BudgetPolicy::new(vec![
        BudgetRule {
            scope: BudgetScope::Org,
            period: BudgetPeriod::Monthly,
            limit: Some(1_000.0),
            warn_threshold: None,
        },
        BudgetRule {
            scope: BudgetScope::Session,
            period: BudgetPeriod::Session,
            limit: Some(2.0),
            warn_threshold: None,
        },
    ])
}

#[test]
fn records_which_scope_bound_the_budget_and_what_remained() {
    let local = BudgetConfig::with_limit(50.0);
    let mut spent = ScopeSpend::new();
    spent.insert((BudgetScope::Org, BudgetPeriod::Monthly), 900.0);
    let resolved = policy().resolve(&local, &spent);

    let decision = BudgetDecision::new(local.limit, &resolved);
    assert_eq!(decision.requested_limit, Some(50.0));
    assert_eq!(decision.effective_limit, Some(2.0));
    assert_eq!(decision.binding_label(), "session");
    assert!(decision.was_tightened());

    let summary = decision.headroom_summary();
    assert!(
        summary.contains("org=monthly:100.0000/1000.0000"),
        "the org's remaining headroom is in the record: {summary}"
    );
    assert!(
        summary.contains("session=session:2.0000/2.0000"),
        "the session's headroom is in the record: {summary}"
    );
}

#[test]
fn an_ungoverned_run_names_no_binding_scope() {
    let local = BudgetConfig::with_limit(5.0);
    let resolved = BudgetPolicy::default().resolve(&local, &ScopeSpend::new());
    let decision = BudgetDecision::new(local.limit, &resolved);

    assert!(!decision.was_tightened());
    assert_eq!(decision.binding_label(), "unbound");
    assert_eq!(decision.effective_limit, Some(5.0));
    assert_eq!(decision.headroom_summary(), "");
    // Logging an ungoverned decision must not panic.
    decision.log();
}

#[test]
fn a_local_budget_tighter_than_every_cap_binds_nothing() {
    let local = BudgetConfig::with_limit(0.5);
    let resolved = policy().resolve(&local, &ScopeSpend::new());
    let decision = BudgetDecision::new(local.limit, &resolved);

    assert_eq!(decision.effective_limit, Some(0.5));
    assert!(!decision.was_tightened());
    assert_eq!(decision.headroom.len(), 2, "both caps are still reported");
    decision.log();
}
