use serde_json::json;

use super::*;

fn status_doc() -> serde_json::Value {
    json!({
        "objective": {
            "description": "ship the rail",
            "scope": "chunk:tui2-2a1-rail",
            "status": "in_progress",
        },
        "task_list": [
            {"id": "a", "title": "explore", "status": "completed"},
            {"id": "b", "title": "implement", "status": "active"},
            {"id": "c", "title": "verify", "status": "planned"},
        ],
        "checks": [
            {"task_id": "a", "passed": true},
            {"task_id": "a", "passed": false},
            {"task_id": "b", "passed": true},
        ],
    })
}

#[test]
fn seeds_objective_scope_and_tasks_from_a_status_document() {
    let view = ShipView::from_status(&status_doc());
    assert_eq!(view.objective(), Some("ship the rail"));
    assert_eq!(view.scope(), Some("chunk:tui2-2a1-rail"));
    assert!(!view.is_shipped());
    let tasks = view.tasks();
    assert_eq!(tasks.len(), 3);
    assert_eq!(tasks[0].state, TaskState::Completed);
    assert_eq!((tasks[0].checks_passed, tasks[0].checks_failed), (1, 1));
    assert_eq!(tasks[1].state, TaskState::Active);
    assert_eq!((tasks[1].checks_passed, tasks[1].checks_failed), (1, 0));
    assert_eq!(tasks[2].state, TaskState::Planned);
}

#[test]
fn an_empty_status_document_seeds_an_empty_view() {
    let view = ShipView::from_status(&serde_json::Value::Null);
    assert_eq!(view, ShipView::default());
}

#[test]
fn a_completed_objective_reads_as_shipped() {
    let mut doc = status_doc();
    doc["objective"]["status"] = json!("completed");
    assert!(ShipView::from_status(&doc).is_shipped());
}

#[test]
fn folds_an_objective_and_its_tasks_from_frames() {
    let mut view = ShipView::default();
    view.apply(
        "objective_set",
        &json!({"objective": {"description": "goal", "scope": "chunk:x"}}),
    );
    view.apply("task_added", &json!({"task_id": "t1", "title": "first"}));
    view.apply("task_added", &json!({"task_id": "t2", "title": "second"}));
    view.apply("task_started", &json!({"task_id": "t1"}));
    view.apply(
        "check_recorded",
        &json!({"task_id": "t1", "check": {"passed": true}}),
    );
    view.apply("task_completed", &json!({"task_id": "t1"}));
    view.apply("task_blocked", &json!({"task_id": "t2"}));
    assert_eq!(view.objective(), Some("goal"));
    assert_eq!(view.tasks()[0].state, TaskState::Completed);
    assert_eq!(view.tasks()[0].checks_passed, 1);
    assert_eq!(view.tasks()[1].state, TaskState::Blocked);
}

#[test]
fn a_repeated_task_id_is_added_once() {
    let mut view = ShipView::default();
    view.apply("task_added", &json!({"task_id": "t1", "title": "first"}));
    view.apply("task_added", &json!({"task_id": "t1", "title": "again"}));
    assert_eq!(view.tasks().len(), 1);
    assert_eq!(view.tasks()[0].title, "first");
}

#[test]
fn a_fresh_objective_keeps_an_in_flight_plan_and_clears_a_shipped_one() {
    let mut view = ShipView::default();
    view.apply(
        "objective_set",
        &json!({"objective": {"description": "one"}}),
    );
    view.apply("task_added", &json!({"task_id": "t1", "title": "first"}));
    view.apply(
        "objective_set",
        &json!({"objective": {"description": "two"}}),
    );
    assert_eq!(view.tasks().len(), 1, "in-flight objective keeps its tasks");
    view.apply("objective_shipped", &json!({}));
    assert!(view.is_shipped());
    view.apply(
        "objective_set",
        &json!({"objective": {"description": "three"}}),
    );
    assert!(
        view.tasks().is_empty(),
        "a shipped objective clears the plan"
    );
    assert!(!view.is_shipped());
}

#[test]
fn cleared_forgets_everything() {
    let mut view = ShipView::from_status(&status_doc());
    view.apply("cleared", &json!({}));
    assert_eq!(view, ShipView::default());
}

#[test]
fn unknown_kinds_and_unknown_task_ids_are_ignored() {
    let mut view = ShipView::from_status(&status_doc());
    let before = view.clone();
    view.apply("task_started", &json!({"task_id": "nope"}));
    view.apply("something_else", &json!({"task_id": "a"}));
    assert_eq!(view, before);
}
