use super::*;
use serde_json::json;

#[test]
fn the_counts_are_added_beside_the_step_it_already_returned() {
    let step: Step = serde_json::from_value(json!({
        "id": "t1",
        "title": "one",
        "type": "implement",
        "status": "active",
        "estimate": null
    }))
    .expect("a step");
    let body = serde_json::to_value(StepResult::from(step)).expect("step result json");
    assert_eq!(body["id"], "t1");
    assert_eq!(body["status"], "active");
    assert!(body.get("progress").is_none());
    assert!(body.get("checks").is_none());
    for count in ["progress_count", "checks_count", "artifacts_count"] {
        assert_eq!(body[count], 0, "{count} in {body}");
    }
}
