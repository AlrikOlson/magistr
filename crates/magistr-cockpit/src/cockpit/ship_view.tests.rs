use magistr_workflow_contracts::ShipFrame;
use magistr_workflow_contracts::{Objective, Task};
use serde_json::json;

use super::*;
use crate::harness::fixtures;

fn objective() -> Objective {
    fixtures::objective_from(json!({
        "description": "ship the rail",
        "scope": "chunk:tui2-2a1-rail",
        "status": "active",
    }))
}

fn tasks() -> Vec<Task> {
    vec![
        fixtures::task_from(json!({
            "id": "a", "title": "explore", "status": "completed",
            "checks": [{"passed": true}, {"passed": false}],
        })),
        fixtures::task_from(json!({
            "id": "b", "title": "implement", "status": "active",
            "checks": [{"passed": true}],
        })),
        fixtures::task_from(json!({"id": "c", "title": "verify", "status": "planned"})),
    ]
}

fn objective_set(description: &str) -> ShipFrame {
    ShipFrame::ObjectiveSet {
        objective: fixtures::objective(description),
    }
}

fn task_added(id: &str, title: &str) -> ShipFrame {
    ShipFrame::TaskAdded {
        task_id: id.to_owned(),
        title: title.to_owned(),
    }
}

fn check_recorded(task_id: &str, passed: bool) -> ShipFrame {
    ShipFrame::CheckRecorded {
        task_id: task_id.to_owned(),
        check: fixtures::check("test", passed),
    }
}

#[test]
fn seeds_objective_scope_and_tasks_from_the_store_records() {
    let view = ShipView::from_ship(Some(&objective()), &tasks());
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
    assert_eq!(view.task("b").map(|t| t.title.as_str()), Some("implement"));
    assert_eq!(view.task("nope"), None);
}

#[test]
fn no_objective_and_no_tasks_seed_an_empty_view() {
    assert_eq!(ShipView::from_ship(None, &[]), ShipView::default());
}

#[test]
fn an_empty_description_and_scope_read_as_none() {
    let view = ShipView::from_ship(Some(&fixtures::objective("")), &[]);
    assert_eq!(view.objective(), None);
    assert_eq!(view.scope(), None);
}

#[test]
fn a_completed_objective_reads_as_shipped() {
    let mut objective = objective();
    objective.status = magistr_workflow_contracts::ObjectiveStatus::Completed;
    assert!(ShipView::from_ship(Some(&objective), &tasks()).is_shipped());
}

#[test]
fn folds_an_objective_and_its_tasks_from_events() {
    let mut view = ShipView::default();
    view.apply(&ShipFrame::ObjectiveSet {
        objective: fixtures::objective_from(json!({"description": "goal", "scope": "chunk:x"})),
    });
    view.apply(&task_added("t1", "first"));
    view.apply(&task_added("t2", "second"));
    view.apply(&ShipFrame::TaskStarted {
        task_id: "t1".to_owned(),
    });
    view.apply(&check_recorded("t1", true));
    view.apply(&ShipFrame::TaskCompleted {
        task: fixtures::task_from(json!({"id":"t1","title":"first","status":"completed",
            "completed_at":"2026-10-04T01:00:00Z",
            "checks":[{"name":"check","passed":true}],
            "artifacts":[{"type":"file","ref":"report.txt","description":"Saved report"}]})),
    });
    view.apply(&ShipFrame::TaskBlocked {
        task_id: "t2".to_owned(),
        reason: "waiting".to_owned(),
    });
    assert_eq!(view.objective(), Some("goal"));
    assert_eq!(view.scope(), Some("chunk:x"));
    assert_eq!(view.tasks()[0].state, TaskState::Completed);
    assert_eq!(view.tasks()[0].checks_passed, 1);
    assert_eq!(
        view.tasks()[0].artifacts,
        vec![("report.txt".into(), "Saved report".into())]
    );
    assert_eq!(
        view.tasks()[0].completed_at.as_deref(),
        Some("2026-10-04T01:00:00Z")
    );
    assert_eq!(view.tasks()[1].state, TaskState::Blocked);
}

#[test]
fn a_repeated_task_id_is_added_once() {
    let mut view = ShipView::default();
    view.apply(&task_added("t1", "first"));
    view.apply(&task_added("t1", "again"));
    assert_eq!(view.tasks().len(), 1);
    assert_eq!(view.tasks()[0].title, "first");
}

#[test]
fn a_fresh_objective_keeps_an_in_flight_plan_and_clears_a_shipped_one() {
    let mut view = ShipView::default();
    view.apply(&objective_set("one"));
    view.apply(&task_added("t1", "first"));
    view.apply(&objective_set("two"));
    assert_eq!(view.tasks().len(), 1, "in-flight objective keeps its tasks");
    view.apply(&ShipFrame::ObjectiveShipped {
        warnings: Vec::new(),
    });
    assert!(view.is_shipped());
    view.apply(&objective_set("three"));
    assert!(
        view.tasks().is_empty(),
        "a shipped objective clears the plan"
    );
    assert!(!view.is_shipped());
}

#[test]
fn cleared_forgets_everything() {
    let mut view = ShipView::from_ship(Some(&objective()), &tasks());
    view.apply(&ShipFrame::Cleared);
    assert_eq!(view, ShipView::default());
}

#[test]
fn actions_and_unknown_task_ids_leave_the_plan_alone() {
    let mut view = ShipView::from_ship(Some(&objective()), &tasks());
    let before = view.clone();
    view.apply(&ShipFrame::TaskStarted {
        task_id: "nope".to_owned(),
    });
    view.apply(&check_recorded("nope", true));
    view.apply(&ShipFrame::ActionRecorded {
        task_id: "a".to_owned(),
        action: fixtures::action(1, "did a thing"),
    });
    assert_eq!(view, before);
}
