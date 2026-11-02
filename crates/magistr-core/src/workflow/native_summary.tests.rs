use super::*;
use crate::workflow::iteration::tests::{complete_native_task, native_engines as engines};
use magistr_engine::NativeEngines;
use magistr_engine::native::TaskStatus;

fn add(engines: &NativeEngines, id: &str, priority: u32, deps: &[&str]) {
    engines
        .plan_add_task(
            id.into(),
            format!("Task {id}"),
            String::new(),
            TaskStatus::ToDo,
            priority,
            String::new(),
            vec![],
            deps.iter().map(|d| (*d).to_string()).collect(),
            None,
            None,
        )
        .expect("add task");
}

#[test]
fn counts_tasks_and_names_the_next_ready_one() {
    let tmp = tempfile::tempdir().unwrap();
    let e = engines(&tmp);
    add(&e, "first", 100, &[]);
    add(&e, "second", 200, &["first"]);

    let s = native_spec_summary(&e).unwrap();
    assert_eq!((s.total, s.checked, s.unchecked), (2, 0, 2));
    assert_eq!(s.unchecked_items, vec!["first — Task first".to_string()]);
    assert!(!s.all_done());

    e.plan_start_task("first").unwrap();
    complete_native_task(&e, "first");
    let s = native_spec_summary(&e).unwrap();
    assert_eq!((s.total, s.checked), (2, 1));
    assert_eq!(s.unchecked_items, vec!["second — Task second".to_string()]);
}

#[test]
fn resumes_one_active_item_before_higher_priority_pending_work_without_writes() {
    let tmp = tempfile::tempdir().unwrap();
    let e = engines(&tmp);
    add(&e, "unfinished", 200, &[]);
    add(&e, "urgent", 1, &[]);
    e.plan_start_task("unfinished").unwrap();
    let before = e.snapshot();
    let summary = native_work_summary(&e, None, None).unwrap();
    assert_eq!(
        (summary.total, summary.checked, summary.unchecked),
        (2, 0, 2)
    );
    assert_eq!(
        summary.unchecked_items,
        ["unfinished — Task unfinished (in progress; resume this work)"]
    );
    assert_eq!(
        native_work_task(&e, None, None).unwrap().unwrap().id,
        "unfinished"
    );
    assert_eq!(
        e.snapshot(),
        before,
        "selection must not rewrite any native state"
    );
    assert_eq!(
        native_spec_summary(&e).unwrap().unchecked_items,
        ["urgent — Task urgent"]
    );
}

#[test]
fn multiple_active_items_need_a_saved_target_or_matching_unfinished_goal() {
    let tmp = tempfile::tempdir().unwrap();
    let e = engines(&tmp);
    add(&e, "first", 500, &[]);
    add(&e, "second", 1, &[]);
    add(&e, "pending", 0, &[]);
    e.plan_start_task("first").unwrap();
    e.plan_start_task("second").unwrap();
    let error = native_work_task(&e, None, None).unwrap_err().to_string();
    assert!(
        error.contains("Task first") && error.contains("Task second"),
        "{error}"
    );
    assert!(error.contains("Choose"), "{error}");
    assert_eq!(
        native_work_task(&e, Some("first"), None)
            .unwrap()
            .unwrap()
            .id,
        "first"
    );

    e.run_open(
        "Finish the second item".into(),
        vec![],
        vec![],
        "task:second".into(),
    )
    .unwrap();
    assert_eq!(
        native_work_task(&e, None, None).unwrap().unwrap().id,
        "second"
    );
    e.add_step(
        "resume".into(),
        "Continue the second item".into(),
        magistr_engine::native::StepType::Implement,
        None,
    )
    .unwrap();
    e.start_step("resume").unwrap();
    assert_eq!(e.run_goal().unwrap().unwrap().status, GoalStatus::Active);
    assert_eq!(
        native_work_task(&e, None, None).unwrap().unwrap().id,
        "second"
    );
    let before = e.snapshot();
    assert_eq!(
        native_work_task(&e, Some("first"), None)
            .unwrap()
            .unwrap()
            .id,
        "first"
    );
    assert_eq!(e.snapshot(), before);
}

#[test]
fn saved_pending_target_stays_bound_and_unfinished_goals_are_preserved() {
    let tmp = tempfile::tempdir().unwrap();
    let e = engines(&tmp);
    add(&e, "chosen", 500, &[]);
    add(&e, "urgent", 1, &[]);
    assert_eq!(
        native_work_task(&e, Some("chosen"), None)
            .unwrap()
            .unwrap()
            .id,
        "chosen"
    );
    e.run_open("Current research".into(), vec![], vec![], "research".into())
        .unwrap();
    let before = e.snapshot();
    assert_eq!(
        native_work_task(&e, Some("chosen"), None)
            .unwrap()
            .unwrap()
            .id,
        "chosen"
    );
    assert_eq!(e.snapshot(), before);
}

#[test]
fn unbound_goals_do_not_imply_a_conflicting_task() {
    for scope in ["", "general", " General "] {
        let tmp = tempfile::tempdir().unwrap();
        let e = engines(&tmp);
        add(&e, "only", 1, &[]);
        e.plan_start_task("only").unwrap();
        e.run_open(
            "Continue existing work".into(),
            vec![],
            vec![],
            scope.into(),
        )
        .unwrap();
        let before = e.snapshot();
        assert_eq!(
            native_work_task(&e, None, None).unwrap().unwrap().id,
            "only"
        );
        assert_eq!(e.snapshot(), before, "unbound goals remain intact");
    }
}

#[test]
fn an_active_blocker_is_reported_instead_of_skipping_to_pending_work() {
    let tmp = tempfile::tempdir().unwrap();
    let e = engines(&tmp);
    add(&e, "unfinished", 200, &[]);
    add(&e, "ready", 1, &[]);
    e.plan_start_task("unfinished").unwrap();
    e.plan_set_waiting_on(
        "unfinished",
        magistr_engine::native::WaitingReason::Person,
        "Waiting for the recovery recording.".into(),
        None,
    )
    .unwrap();
    let before = e.snapshot();
    for preferred in [None, Some("unfinished")] {
        let error = native_work_task(&e, preferred, None)
            .unwrap_err()
            .to_string();
        assert!(error.contains("Task unfinished"), "{error}");
        assert!(
            error.contains("Waiting for the recovery recording"),
            "{error}"
        );
    }
    assert_eq!(e.snapshot(), before);
}

#[test]
fn saved_blocked_or_deferred_items_are_not_silently_replaced() {
    for status in [TaskStatus::Waiting, TaskStatus::Idea] {
        let tmp = tempfile::tempdir().unwrap();
        let e = engines(&tmp);
        add(&e, "saved", 200, &[]);
        add(&e, "ready", 1, &[]);
        e.plan_set_status("saved", status).unwrap();
        let before = e.snapshot();
        let error = native_work_task(&e, Some("saved"), None)
            .unwrap_err()
            .to_string();
        assert!(error.contains("Task saved"), "{error}");
        assert_eq!(e.snapshot(), before);
    }
}

#[test]
fn missing_saved_work_refuses_unrelated_pending_or_active_work_without_writes() {
    for active in [false, true] {
        let tmp = tempfile::tempdir().unwrap();
        let e = engines(&tmp);
        add(&e, "other", 1, &[]);
        if active {
            e.plan_start_task("other").unwrap();
            e.run_open(
                "Finish other work".into(),
                vec![],
                vec![],
                "task:other".into(),
            )
            .unwrap();
        }
        let before = e.snapshot();
        let error = native_work_summary(&e, Some("private-missing-id"), None)
            .unwrap_err()
            .to_string();
        assert!(
            error.contains("The work saved for this run is no longer in the plan"),
            "{error}"
        );
        assert!(!error.contains("private-missing-id"), "{error}");
        assert_eq!(
            e.snapshot(),
            before,
            "a missing saved target never authorizes other work"
        );
        assert_eq!(
            native_work_task(&e, None, None).unwrap().unwrap().id,
            "other",
            "other work is available only when no saved target was requested"
        );
    }
}

#[test]
fn active_dependencies_are_checked_again_and_missing_work_is_not_invented() {
    let tmp = tempfile::tempdir().unwrap();
    let e = engines(&tmp);
    add(&e, "storage", 100, &[]);
    add(&e, "recovery", 200, &[]);
    e.plan_start_task("recovery").unwrap();
    // An authored dependency can change after work has started.
    e.plan_update_task(
        "recovery",
        None,
        None,
        None,
        None,
        Some(vec!["storage".into()]),
        None,
    )
    .unwrap();
    let before = e.snapshot();
    let error = native_work_task(&e, None, None).unwrap_err().to_string();
    assert!(
        error.contains("Task recovery") && error.contains("Task storage"),
        "{error}"
    );
    assert_eq!(e.snapshot(), before);
    e.plan_start_task("storage").unwrap();
    complete_native_task(&e, "storage");
    assert_eq!(
        native_work_task(&e, None, None).unwrap().unwrap().id,
        "recovery"
    );

    let mut missing = e.plan_task("recovery").unwrap().unwrap();
    missing.deps = vec!["private-missing-identifier".into()];
    let error = ensure_available(&missing, &e.plan_tasks().unwrap())
        .unwrap_err()
        .to_string();
    assert!(error.contains("required work item is missing"), "{error}");
    assert!(!error.contains("private-missing-identifier"), "{error}");
}

#[test]
fn finished_goals_and_saved_items_do_not_prevent_the_next_ready_item() {
    let tmp = tempfile::tempdir().unwrap();
    let e = engines(&tmp);
    add(&e, "finished", 100, &[]);
    add(&e, "next", 200, &["finished"]);
    e.plan_start_task("finished").unwrap();
    complete_native_task(&e, "finished");
    let before = e.snapshot();
    let summary = native_work_summary(&e, Some("finished"), None).unwrap();
    assert_eq!(summary.unchecked_items, ["next — Task next"]);
    assert_eq!(
        (summary.total, summary.checked, summary.unchecked),
        (2, 1, 1)
    );
    assert_eq!(e.snapshot(), before);
    e.finish_step("verify", vec![]).unwrap();
    e.run_finish(vec![], None).unwrap();
    e.plan_start_task("next").unwrap();
    complete_native_task(&e, "next");
    let summary = native_work_summary(&e, Some("next"), None).unwrap();
    assert!(summary.all_done());
    assert_eq!(
        (summary.total, summary.checked, summary.unchecked),
        (2, 2, 0)
    );
}

#[test]
fn empty_and_non_actionable_plans_return_no_work() {
    let tmp = tempfile::tempdir().unwrap();
    let e = engines(&tmp);
    assert!(native_work_task(&e, None, None).unwrap().is_none());
    let summary = native_work_summary(&e, None, None).unwrap();
    assert_eq!(
        (summary.total, summary.checked, summary.unchecked),
        (0, 0, 0)
    );
    add(&e, "later", 1, &[]);
    e.plan_set_status("later", TaskStatus::Idea).unwrap();
    assert!(native_work_task(&e, None, None).unwrap().is_none());
    e.plan_set_status("later", TaskStatus::Retired).unwrap();
    assert!(native_work_task(&e, Some("later"), None).unwrap().is_none());
}

#[test]
fn unfinished_work_is_not_reported_as_complete_when_nothing_is_ready() {
    let tmp = tempfile::tempdir().unwrap();
    let e = engines(&tmp);
    add(&e, "only", 100, &[]);
    e.plan_start_task("only").unwrap();
    // Availability and completion are independent facts.
    let s = native_spec_summary(&e).unwrap();
    assert_eq!((s.total, s.checked, s.unchecked), (1, 0, 1));
    assert!(s.unchecked_items.is_empty());
    assert!(!s.all_done());
}

#[test]
fn selecting_work_refuses_an_unreadable_execution_plan_until_recovery() {
    let tmp = tempfile::tempdir().unwrap();
    let e = engines(&tmp);
    add(&e, "work", 100, &[]);
    e.run_open(
        "Continue this work".into(),
        vec![],
        vec![],
        "task:work".into(),
    )
    .unwrap();
    assert_eq!(
        native_work_task(&e, None, None).unwrap().unwrap().id,
        "work"
    );
    let path = e
        .config()
        .persistence_config()
        .data_dir
        .join("run")
        .join("executions")
        .join(e.config().execution_context().key())
        .join("sessions")
        .join(format!("{}.json", e.project_id()));
    let valid = std::fs::read(&path).unwrap();
    std::fs::write(&path, b"{ unreadable plan").unwrap();
    assert!(native_work_task(&e, None, None).is_err());
    assert!(native_work_summary(&e, Some("work"), None).is_err());
    std::fs::write(path, valid).unwrap();
    assert_eq!(
        native_work_task(&e, None, None).unwrap().unwrap().id,
        "work"
    );
}
