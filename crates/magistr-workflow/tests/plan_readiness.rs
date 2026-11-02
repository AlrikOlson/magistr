//! Start admission uses the same current records as next-work hints.
use magistr_workflow::plan::PlanEngine;
use magistr_workflow::plan::domain::{TaskStatus, WaitingOn, WaitingReason};

fn blocker(kind: WaitingReason, reason: &str) -> WaitingOn {
    WaitingOn {
        kind,
        reason: reason.into(),
        evidence: None,
        waiting_since: "2026-10-03T00:00:00Z".into(),
    }
}

fn add(engine: &mut PlanEngine, id: &str, priority: u32, deps: &[&str]) {
    engine
        .add_task(
            id.into(),
            format!("Work {id}"),
            TaskStatus::ToDo,
            priority,
            String::new(),
            vec![],
            deps.iter().map(|id| (*id).into()).collect(),
            false,
        )
        .unwrap();
    engine.set_feature(id, Some("Delivery".into())).unwrap();
}

#[test]
fn start_and_status_admission_reject_waiting_work_without_changing_records() {
    let mut engine = PlanEngine::new("project".into());
    add(&mut engine, "foundation", 20, &[]);
    add(&mut engine, "waiting", 1, &["foundation", "absent"]);
    let before = engine.plan().clone();
    let error = engine
        .start_task("waiting")
        .expect_err("dependencies must be completed");
    assert!(
        error.to_string().to_lowercase().contains("foundation")
            && error.to_string().contains("unavailable"),
        "{error}"
    );
    assert_eq!(engine.plan(), &before);
    assert!(
        engine
            .set_status("waiting", TaskStatus::InProgress)
            .is_err()
    );
    assert_eq!(engine.plan(), &before);
}

#[test]
fn blockers_and_status_proposals_cannot_bypass_start_admission() {
    let mut engine = PlanEngine::new("project".into());
    add(&mut engine, "work", 1, &[]);
    engine
        .set_waiting_on("work", blocker(WaitingReason::Person, "Access is needed."))
        .unwrap();
    engine
        .propose_status(
            "work",
            TaskStatus::InProgress,
            "Start now".into(),
            "tracker".into(),
        )
        .unwrap();
    let before = engine.plan().clone();
    assert!(engine.start_task("work").is_err());
    assert!(engine.resolve_status_proposal("work", true).is_err());
    assert_eq!(engine.plan(), &before);
    engine.clear_waiting_on("work").unwrap();
    engine.resolve_status_proposal("work", true).unwrap();
}

#[test]
fn stale_hint_is_rechecked_by_the_writer_and_ties_keep_snapshot_order() {
    let mut engine = PlanEngine::new("project".into());
    add(&mut engine, "z-first", 10, &[]);
    add(&mut engine, "a-second", 10, &[]);
    let hinted = engine.next().unwrap().id.clone();
    assert_eq!(hinted, "z-first");
    assert_eq!(engine.next_in_feature("Delivery").unwrap().id, hinted);
    assert_eq!(
        engine
            .workstream_frontier(Some("Delivery"))
            .activity
            .readiness
            .ready,
        2
    );
    engine
        .set_waiting_on(
            &hinted,
            blocker(WaitingReason::Outside, "Access was revoked."),
        )
        .unwrap();
    assert!(
        engine.start_task(&hinted).is_err(),
        "a cached hint grants no admission"
    );
    assert_eq!(engine.next().unwrap().id, "a-second");
    assert_eq!(
        engine
            .workstream_frontier(Some("Delivery"))
            .activity
            .readiness
            .ready,
        1
    );
}

#[test]
fn completed_dependencies_admit_work_and_completion_revocation_is_distinct() {
    let mut engine = PlanEngine::new("project".into());
    add(&mut engine, "foundation", 10, &[]);
    add(&mut engine, "work", 20, &["foundation"]);
    engine.start_task("foundation").unwrap();
    engine.complete_task("foundation", None).unwrap();
    engine.start_task("work").unwrap();
    engine.complete_task("work", None).unwrap();
    assert!(engine.start_task("work").is_err());
    engine.set_status("work", TaskStatus::InProgress).unwrap();
    assert_eq!(engine.plan().tasks[1].status, TaskStatus::InProgress);
}

#[test]
fn completed_work_closes_saved_focus_without_redirecting_or_mutating_it() {
    use magistr_workflow::plan::domain::{FocusMode, FocusState};
    let mut engine = PlanEngine::new("project".into());
    add(&mut engine, "foundation", 10, &[]);
    engine
        .set_feature("foundation", Some("Rust".into()))
        .unwrap();
    engine.focus_set("lane", "Rust", FocusMode::Build).unwrap();
    add(&mut engine, "desktop", 20, &[]);
    engine
        .set_feature("desktop", Some("Rust desktop".into()))
        .unwrap();
    engine.start_task("foundation").unwrap();
    engine.complete_task("foundation", None).unwrap();

    let before = engine.plan().clone();
    let catalog = engine.workstreams();
    assert_eq!(
        catalog.focus_state(engine.focus_get("lane").map(|focus| focus.feature.as_str())),
        FocusState::Closed {
            feature: "Rust".into()
        }
    );
    assert_eq!(catalog.current_names(), ["Rust desktop"]);
    assert_eq!(
        engine.workstream_history(Some("Rust")).task_ids,
        ["foundation"]
    );
    assert!(engine.workstream_frontier(Some("Rust")).items.is_empty());
    assert!(engine.next_in_feature("Rust").is_none());
    for query in ["Rust", "rust"] {
        let error = engine
            .focus_set("lane", query, FocusMode::Shape)
            .unwrap_err();
        assert!(
            error.to_string().contains("no unfinished work")
                && error.to_string().contains("Rust desktop")
        );
        assert_eq!(engine.plan(), &before);
    }

    engine
        .set_feature("foundation", Some("Archive".into()))
        .unwrap();
    assert_eq!(
        engine.workstreams().focus_state(Some("Rust")),
        FocusState::Missing {
            feature: "Rust".into()
        }
    );
    assert_eq!(engine.focus_get("lane").unwrap().feature, "Rust");
    engine
        .set_status("foundation", TaskStatus::InProgress)
        .unwrap();
    assert_eq!(
        engine.workstreams().current_names(),
        ["Archive", "Rust desktop"]
    );
    assert!(!engine.workstreams().history.contains_key("Archive"));
}

#[test]
fn current_grouping_proposals_never_enlist_or_change_terminal_records() {
    let mut engine = PlanEngine::new("project".into());
    for (id, status) in [
        ("old-one", TaskStatus::Done),
        ("old-two", TaskStatus::Retired),
        ("new-one", TaskStatus::Idea),
        ("new-two", TaskStatus::ToDo),
        ("new-finished", TaskStatus::Done),
        ("new-grouped", TaskStatus::ToDo),
    ] {
        engine
            .add_task(
                id.into(),
                format!("Work {id}"),
                status,
                10,
                String::new(),
                vec![],
                vec![],
                false,
            )
            .unwrap();
    }
    engine
        .set_feature("new-grouped", Some("Delivery".into()))
        .unwrap();
    let before = engine.plan().clone();
    let proposals = engine.propose_features_from_ids(2);
    assert_eq!(proposals.len(), 1);
    assert_eq!(proposals[0].prefix, "new");
    assert_eq!(proposals[0].task_ids, ["new-one", "new-two"]);
    assert_eq!(engine.plan(), &before, "discovery writes nothing");
    assert_eq!(
        engine.workstream_history(None).task_ids,
        ["old-one", "old-two", "new-finished"]
    );
}
