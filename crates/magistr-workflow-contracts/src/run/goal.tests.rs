use super::*;

#[test]
fn new_work_defaults_to_focused() {
    assert_eq!(VerificationMode::default(), VerificationMode::Focused);
}

#[test]
fn old_goals_default_full_and_focused_changes_the_acceptance_revision() {
    let mut goal: Goal = serde_json::from_value(serde_json::json!({
        "description":"Run","acceptance_criteria":["Works"],"constraints":[],
        "scope":"chunk:a","status":"active","project_id":"p"
    }))
    .unwrap();
    assert_eq!(goal.verification, VerificationMode::Full);
    let original = goal.acceptance_revision();
    assert!(
        serde_json::to_value(&goal)
            .unwrap()
            .get("verification")
            .is_none()
    );
    goal.verification = VerificationMode::Focused;
    assert_ne!(goal.acceptance_revision(), original);
    assert_eq!(
        serde_json::to_value(goal).unwrap()["verification"],
        "focused"
    );
}
