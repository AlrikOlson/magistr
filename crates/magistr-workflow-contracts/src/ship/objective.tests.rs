use super::*;
#[test]
fn old_objectives_default_full_and_focused_changes_the_acceptance_revision() {
    let mut objective: Objective = serde_json::from_value(serde_json::json!({
        "description":"Ship","acceptance_criteria":["Works"],"constraints":[],
        "scope":"chunk:a","status":"active","project_id":"p"
    }))
    .unwrap();
    assert_eq!(objective.verification, VerificationMode::Full);
    let original = objective.acceptance_revision();
    assert!(
        serde_json::to_value(&objective)
            .unwrap()
            .get("verification")
            .is_none()
    );
    objective.verification = VerificationMode::Focused;
    assert_ne!(objective.acceptance_revision(), original);
    assert_eq!(
        serde_json::to_value(objective).unwrap()["verification"],
        "focused"
    );
}
