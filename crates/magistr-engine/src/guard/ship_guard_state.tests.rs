use super::*;
use magistr_workflow::ship::domain::check::{Check, CheckType};
use magistr_workflow::ship::domain::task::{TaskStatus, TaskType};

fn objective(status: ObjectiveStatus) -> Objective {
    Objective {
        description: "goal".to_owned(),
        acceptance_criteria: Vec::new(),
        constraints: Vec::new(),
        scope: String::new(),
        status,
        project_id: String::new(),
        created_at: None,
        completed_at: None,
    }
}

fn task(id: &str, checks: usize) -> Task {
    Task {
        id: id.to_owned(),
        title: id.to_owned(),
        task_type: TaskType::Implement,
        status: TaskStatus::Planned,
        estimate: None,
        started_at: None,
        completed_at: None,
        artifacts: Vec::new(),
        checks: (0..checks)
            .map(|_| Check {
                check_type: CheckType::Test,
                name: "gate".to_owned(),
                passed: false,
                details: String::new(),
                required: true,
                verified: true,
                command: None,
                exit_code: None,
                report: None,
                results: None,
                timestamp: String::new(),
            })
            .collect(),
        actions: Vec::new(),
        blocked_reason: None,
        think_branch: None,
    }
}

#[test]
fn an_empty_ship_has_nothing() {
    let s = ShipGuardState::from_ship(None, &[]);
    assert_eq!(s, ShipGuardState::default());
    assert!(!s.has_plan());
    assert!(!s.has_check());
}

#[test]
fn defined_or_active_objective_counts_completed_does_not() {
    let set = |status| ShipGuardState::from_ship(Some(&objective(status)), &[]).objective_set;
    assert!(set(ObjectiveStatus::Defined));
    assert!(set(ObjectiveStatus::Active));
    assert!(!set(ObjectiveStatus::Completed));
    assert!(!set(ObjectiveStatus::Abandoned));
    assert!(!ShipGuardState::from_ship(None, &[]).objective_set);
}

#[test]
fn plan_needs_objective_and_a_task() {
    let defined = objective(ObjectiveStatus::Defined);
    assert!(!ShipGuardState::from_ship(Some(&defined), &[]).has_plan());
    assert!(!ShipGuardState::from_ship(None, &[task("t1", 0)]).has_plan());
    let s = ShipGuardState::from_ship(Some(&defined), &[task("t1", 0), task("t2", 0)]);
    assert!(s.has_plan());
    assert_eq!(s.tasks_planned, 2);
}

#[test]
fn checks_are_counted_on_the_current_plans_tasks() {
    // The plan is the current cycle's tasks: a check from a finished cycle
    // is on no task here and is not counted.
    let active = objective(ObjectiveStatus::Active);
    let s = ShipGuardState::from_ship(Some(&active), &[task("t1", 1), task("t2", 2)]);
    assert_eq!(s.checks_recorded, 3);
    assert!(s.has_check());
    assert!(!ShipGuardState::from_ship(Some(&active), &[task("t1", 0)]).has_check());
}

#[test]
fn research_and_plan_are_always_open() {
    let s = ShipGuardState::default();
    assert!(s.phase_block("research").is_none());
    assert!(s.phase_block("plan").is_none());
}

#[test]
fn execute_and_verify_need_a_plan() {
    let none = ShipGuardState::default();
    let reason = none.phase_block("execute").expect("blocked");
    assert!(reason.contains("ship_set_objective"), "{reason}");
    assert!(reason.contains("ship_plan"), "{reason}");
    assert!(none.phase_block("verify").is_some());

    let planned = ShipGuardState {
        objective_set: true,
        tasks_planned: 1,
        checks_recorded: 0,
    };
    assert!(planned.phase_block("execute").is_none());
    assert!(planned.phase_block("verify").is_none());
}

#[test]
fn done_needs_a_recorded_check() {
    let planned = ShipGuardState {
        objective_set: true,
        tasks_planned: 1,
        checks_recorded: 0,
    };
    let reason = planned.phase_block("done").expect("blocked");
    assert!(reason.contains("ship_check"), "{reason}");

    let checked = ShipGuardState {
        checks_recorded: 1,
        ..planned
    };
    assert!(checked.phase_block("done").is_none());
}

#[test]
fn unknown_phase_is_not_blocked_here() {
    // Phase-name validation belongs to the caller; the guard only rules on
    // the phases it knows.
    assert!(ShipGuardState::default().phase_block("bogus").is_none());
}
