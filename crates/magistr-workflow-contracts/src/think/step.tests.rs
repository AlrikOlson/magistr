use super::ThinkStep;
use serde_json::json;

#[test]
fn recorded_identity_round_trips_without_deriving_it_from_prose_or_number() {
    let mut step: ThinkStep = serde_json::from_value(json!({
        "step_number": 3, "purpose": "analysis", "thought": "same words",
        "timestamp": "2026-10-14T00:00:00Z", "record_id": "authored-id"
    }))
    .unwrap();
    step.step_number = 900;
    let copy: ThinkStep = serde_json::from_value(serde_json::to_value(&step).unwrap()).unwrap();
    assert_eq!(copy.record_id.as_deref(), Some("authored-id"));
    assert_eq!(copy.step_number, 900);
}

#[test]
fn unrecorded_input_has_no_manufactured_identity() {
    let step: ThinkStep = serde_json::from_value(json!({
        "purpose": "analysis", "thought": "same words"
    }))
    .unwrap();
    assert_eq!(step.record_id, None);
}
