use super::*;

#[test]
fn remaining_is_the_limit_less_what_the_scope_already_spent() {
    let h = ScopeHeadroom::new(BudgetScope::Team, BudgetPeriod::Monthly, 100.0, 30.0);
    assert_eq!(h.remaining, 70.0);
    assert!(!h.is_exhausted());
}

#[test]
fn overspend_floors_at_zero_rather_than_going_negative() {
    let h = ScopeHeadroom::new(BudgetScope::Org, BudgetPeriod::Daily, 10.0, 12.5);
    assert_eq!(h.remaining, 0.0);
    assert!(h.is_exhausted());
}

#[test]
fn spending_exactly_the_limit_is_exhausted() {
    let h = ScopeHeadroom::new(BudgetScope::Project, BudgetPeriod::Weekly, 5.0, 5.0);
    assert_eq!(h.remaining, 0.0);
    assert!(h.is_exhausted());
}
