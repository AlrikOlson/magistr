use super::*;
#[test]
fn legacy_actions_default_optional_evidence_without_inventing_it() {
    let action: Action = serde_json::from_value(serde_json::json!({
        "id":1,"task_id":"t","timestamp":"then","type":"review","description":"Reviewed","result":"OK"
    })).unwrap();
    assert!(action.files_touched.is_empty() && action.tools_used.is_empty());
    assert_eq!(action.think_step, None);
    assert_eq!(action.action_type.as_wire(), "review");
    assert_eq!(serde_json::to_value(action).unwrap()["type"], "review");
}
