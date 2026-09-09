use super::*;
use crate::cost::BudgetPeriod;

#[test]
fn an_unconstrained_resolution_is_neither_bound_nor_exhausted() {
    let resolved = ResolvedBudget {
        effective: BudgetConfig::none(),
        binding: None,
        headroom: Vec::new(),
    };
    assert!(!resolved.is_exhausted());
    assert!(!resolved.is_governed());
}

#[test]
fn any_exhausted_scope_exhausts_the_resolution() {
    let resolved = ResolvedBudget {
        effective: BudgetConfig::with_limit(0.0),
        binding: Some(BudgetScope::Team),
        headroom: vec![
            ScopeHeadroom::new(BudgetScope::Org, BudgetPeriod::Monthly, 900.0, 10.0),
            ScopeHeadroom::new(BudgetScope::Team, BudgetPeriod::Monthly, 50.0, 50.0),
        ],
    };
    assert!(resolved.is_exhausted());
    assert!(resolved.is_governed());
}

#[test]
fn the_tightest_headroom_is_the_one_reported() {
    let resolved = ResolvedBudget {
        effective: BudgetConfig::with_limit(5.0),
        binding: Some(BudgetScope::Team),
        headroom: vec![
            ScopeHeadroom::new(BudgetScope::Org, BudgetPeriod::Monthly, 900.0, 10.0),
            ScopeHeadroom::new(BudgetScope::Team, BudgetPeriod::Monthly, 50.0, 45.0),
        ],
    };
    let tightest = resolved.tightest().expect("a tightest headroom");
    assert_eq!(tightest.scope, BudgetScope::Team);
    assert_eq!(tightest.remaining, 5.0);
}
