use super::*;
use serde_json::json;

fn status(objective: Option<&str>, tasks: &[&str], checks: &[(&str, bool)]) -> Value {
    json!({
        "objective": objective.map(|s| json!({"status": s})),
        "task_list": tasks.iter().map(|id| json!({"id": id})).collect::<Vec<_>>(),
        "checks": checks
            .iter()
            .map(|(task, passed)| json!({"task_id": task, "name": "gate", "passed": passed}))
            .collect::<Vec<_>>(),
    })
}

#[test]
fn empty_status_has_nothing() {
    let s = ShipGuardState::from_status(&json!({}));
    assert_eq!(s, ShipGuardState::default());
    assert!(!s.has_plan());
    assert!(!s.has_check());
}

#[test]
fn defined_or_active_objective_counts_completed_does_not() {
    assert!(ShipGuardState::from_status(&status(Some("defined"), &[], &[])).objective_set);
    assert!(ShipGuardState::from_status(&status(Some("active"), &[], &[])).objective_set);
    assert!(!ShipGuardState::from_status(&status(Some("completed"), &[], &[])).objective_set);
    assert!(!ShipGuardState::from_status(&status(Some("abandoned"), &[], &[])).objective_set);
    assert!(!ShipGuardState::from_status(&status(None, &[], &[])).objective_set);
}

#[test]
fn plan_needs_objective_and_a_task() {
    assert!(!ShipGuardState::from_status(&status(Some("defined"), &[], &[])).has_plan());
    assert!(!ShipGuardState::from_status(&status(None, &["t1"], &[])).has_plan());
    let s = ShipGuardState::from_status(&status(Some("defined"), &["t1", "t2"], &[]));
    assert!(s.has_plan());
    assert_eq!(s.tasks_planned, 2);
}

#[test]
fn checks_count_only_current_tasks() {
    // A check left over from a finished cycle (task not in the list) is ignored.
    let s = ShipGuardState::from_status(&status(
        Some("active"),
        &["t1"],
        &[("old-task", true), ("t1", false)],
    ));
    assert_eq!(s.checks_recorded, 1);
    assert!(s.has_check());

    let stale_only =
        ShipGuardState::from_status(&status(Some("active"), &["t1"], &[("old-task", true)]));
    assert!(!stale_only.has_check());
}

#[test]
fn check_without_task_id_counts() {
    let doc = json!({
        "objective": {"status": "active"},
        "task_list": [{"id": "t1"}],
        "checks": [{"name": "manual", "passed": true}],
    });
    assert_eq!(ShipGuardState::from_status(&doc).checks_recorded, 1);
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
