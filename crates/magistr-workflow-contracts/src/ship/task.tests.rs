use super::*;
#[test]
fn old_tasks_keep_status_and_default_empty_evidence_collections() {
    let task: Task = serde_json::from_value(serde_json::json!({
        "id":"t","title":"Check","type":"review","status":"blocked","blocked_reason":"Waiting"
    }))
    .unwrap();
    assert_eq!(task.status, TaskStatus::Blocked);
    assert_eq!(task.task_type, TaskType::Review);
    assert_eq!(task.blocked_reason.as_deref(), Some("Waiting"));
    assert!(task.checks.is_empty() && task.actions.is_empty() && task.artifacts.is_empty());
    let wire = serde_json::to_value(&task).unwrap();
    assert_eq!(serde_json::from_value::<Task>(wire).unwrap(), task);
}
