use super::*;
#[test]
fn old_steps_keep_status_and_default_empty_evidence_collections() {
    let step: Step = serde_json::from_value(serde_json::json!({
        "id":"t","title":"Check","type":"review","status":"waiting","waiting_reason":"Waiting"
    }))
    .unwrap();
    assert_eq!(step.status, StepStatus::Waiting);
    assert_eq!(step.step_type, StepType::Review);
    assert_eq!(step.waiting_reason.as_deref(), Some("Waiting"));
    assert!(step.checks.is_empty() && step.progress.is_empty() && step.artifacts.is_empty());
    let wire = serde_json::to_value(&step).unwrap();
    assert_eq!(serde_json::from_value::<Step>(wire).unwrap(), step);
}
