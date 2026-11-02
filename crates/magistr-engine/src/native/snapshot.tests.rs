use super::*;
use magistr_workflow::plan::PlanEngine;

fn add(e: &mut PlanEngine, id: &str, priority: u32, deps: &[&str]) {
    e.add_task(
        id.into(),
        format!("Task {id}"),
        TaskStatus::ToDo,
        priority,
        String::new(),
        vec![],
        deps.iter().map(|d| (*d).to_string()).collect(),
        false,
    )
    .expect("add");
}

#[test]
fn snapshot_reports_counts_next_workstreams_and_progress() {
    let mut e = PlanEngine::new("magistr-test".into());
    add(&mut e, "base", 100, &[]);
    add(&mut e, "gui", 200, &["base"]);
    add(&mut e, "cli", 300, &[]);
    add(&mut e, "old", 400, &[]);
    e.set_feature("base", Some("Engine".into())).expect("g");
    e.set_feature("gui", Some("Desktop".into())).expect("g");
    e.set_feature("cli", Some("Engine".into())).expect("g");
    e.set_status("base", TaskStatus::InProgress).expect("s");
    e.retire_task("old", "superseded".into()).expect("o");

    let s = NativePlanSnapshot::from_tasks(&e.plan().project_id, &e.plan().tasks);
    assert_eq!(s.project_id, "magistr-test");
    assert_eq!(s.count("to_do"), 2);
    assert_eq!(s.count("in_progress"), 1);
    assert_eq!(s.count("retired"), 1);
    assert_eq!(s.count("done"), 0);
    assert_eq!(s.in_progress, vec!["base".to_string()]);
    // gui is blocked on base (in progress); cli is the lowest ready.
    assert_eq!(s.next_ready.as_deref(), Some("cli"));
    let engine = s
        .workstreams
        .current
        .get("Engine")
        .expect("Engine workstream");
    assert_eq!(engine.counts.total, 2);
    assert_eq!(engine.counts.in_progress, 1);
    assert_eq!(engine.counts.to_do, 1);
    assert_eq!(engine.readiness.ready, 1);
    assert_eq!(engine.next_ready.as_deref(), Some("cli"));
    let desktop = &s.workstreams.current["Desktop"];
    assert_eq!(desktop.counts.to_do, 1);
    assert_eq!(desktop.readiness.dependent, 1);
    assert_eq!(desktop.readiness.waiting, 0);
    assert_eq!(s.workstreams.unassigned_history.retired, 1);
    assert_eq!(s.progress.total_tasks, 3);
    assert_eq!(s.progress.total_phases, 2);
}

#[test]
fn snapshot_round_trips_through_json() {
    let mut e = PlanEngine::new("p".into());
    add(&mut e, "a", 1, &[]);
    let s = NativePlanSnapshot::from_tasks(&e.plan().project_id, &e.plan().tasks);
    let json = serde_json::to_string(&s).expect("ser");
    let back: NativePlanSnapshot = serde_json::from_str(&json).expect("de");
    assert_eq!(back, s);
}
