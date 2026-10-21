use super::*;
use serde_json::json;

#[test]
fn the_counts_are_added_beside_the_task_it_already_returned() {
    let task: Task = serde_json::from_value(json!({
        "id": "t1",
        "title": "one",
        "type": "implement",
        "status": "active",
        "estimate": null
    }))
    .expect("a task");
    let body = serde_json::to_value(TaskResult::from(task)).expect("task result json");
    assert_eq!(body["id"], "t1");
    assert_eq!(body["status"], "active");
    assert!(body.get("actions").is_none());
    assert!(body.get("checks").is_none());
    for count in ["actions_count", "checks_count", "artifacts_count"] {
        assert_eq!(body[count], 0, "{count} in {body}");
    }
}
