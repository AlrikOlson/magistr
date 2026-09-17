use super::*;
#[test]
fn legacy_record_identity_survives_renumbering_and_existing_ids() {
    let mut step: ThinkStep = serde_json::from_value(serde_json::json!({
        "step_number":1,"timestamp":"then","purpose":"Check","thought":"Evidence",
        "next_action":{"action":"Verify","expectedOutput":"Result"}
    }))
    .unwrap();
    assert_eq!(step.outcome, "");
    let original = step.backfilled_record_id().unwrap();
    step.step_number = 900;
    assert_eq!(
        step.backfilled_record_id().as_deref(),
        Some(original.as_str())
    );
    assert!(step.ensure_record_id());
    assert!(!step.ensure_record_id());
    assert_eq!(
        serde_json::to_value(step).unwrap()["next_action"]["expectedOutput"],
        "Result"
    );
}
