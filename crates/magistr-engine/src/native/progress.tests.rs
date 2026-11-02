use super::*;
use magistr_workflow::plan::PlanEngine;

fn engine_with(tasks: &[(&str, TaskStatus, Option<&str>)]) -> PlanEngine {
    let mut e = PlanEngine::new("p".into());
    for (id, status, feature) in tasks {
        e.add_task(
            (*id).into(),
            format!("Task {id}"),
            TaskStatus::ToDo,
            100,
            String::new(),
            vec![],
            vec![],
            false,
        )
        .expect("add");
        if let Some(g) = feature {
            e.set_feature(id, Some((*g).into())).expect("feature");
        }
        if *status != TaskStatus::ToDo {
            // Done requires InProgress first; go through the legal path.
            if *status == TaskStatus::Done {
                e.set_status(id, TaskStatus::InProgress).expect("ip");
            }
            e.set_status(id, *status).expect("status");
        }
    }
    e
}

#[test]
fn empty_plan_has_no_progress() {
    let e = PlanEngine::new("p".into());
    assert_eq!(
        progress_from_tasks(&e.plan().tasks),
        PlanProgress::default()
    );
}

#[test]
fn counts_tasks_as_tasks_and_features_as_phases() {
    let e = engine_with(&[
        ("a", TaskStatus::Done, Some("Engine")),
        ("b", TaskStatus::Done, Some("Engine")),
        ("c", TaskStatus::ToDo, Some("GUI")),
        ("d", TaskStatus::InProgress, None),
    ]);
    let p = progress_from_tasks(&e.plan().tasks);
    assert_eq!(p.total_tasks, 4);
    assert_eq!(p.checked_tasks, 2);
    assert_eq!(p.total_phases, 2);
    assert_eq!(p.completed_phases, 1, "Engine is fully done, GUI is not");
    assert!(!p.is_complete());
}

#[test]
fn retired_tasks_count_toward_nothing() {
    let e = engine_with(&[
        ("a", TaskStatus::Done, Some("Engine")),
        ("z", TaskStatus::Retired, Some("Engine")),
        ("y", TaskStatus::Retired, Some("Dead")),
    ]);
    let p = progress_from_tasks(&e.plan().tasks);
    assert_eq!(p.total_tasks, 1);
    assert_eq!(p.checked_tasks, 1);
    assert_eq!(
        p.total_phases, 1,
        "a feature of only retired tasks is not a phase"
    );
    assert_eq!(p.completed_phases, 1);
    assert!(p.is_complete());
}
