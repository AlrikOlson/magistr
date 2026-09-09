use super::*;

#[test]
fn scopes_order_broad_to_narrow() {
    assert!(BudgetScope::Org < BudgetScope::Team);
    assert!(BudgetScope::Team < BudgetScope::Project);
    assert!(BudgetScope::Project < BudgetScope::Session);
}

#[test]
fn all_lists_every_scope_broadest_first() {
    assert_eq!(
        BudgetScope::ALL,
        [
            BudgetScope::Org,
            BudgetScope::Team,
            BudgetScope::Project,
            BudgetScope::Session
        ]
    );
}

#[test]
fn as_str_is_the_serialized_name() {
    assert_eq!(BudgetScope::Org.as_str(), "org");
    assert_eq!(BudgetScope::Session.as_str(), "session");
}

#[test]
fn serde_round_trips_lowercase() {
    let json = serde_json::to_string(&BudgetScope::Project).expect("serialize");
    assert_eq!(json, "\"project\"");
    let back: BudgetScope = serde_json::from_str(&json).expect("deserialize");
    assert_eq!(back, BudgetScope::Project);
}

#[test]
fn display_matches_as_str() {
    assert_eq!(BudgetScope::Team.to_string(), "team");
}
