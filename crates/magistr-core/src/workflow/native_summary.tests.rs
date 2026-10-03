use super::*;
use crate::workflow::iteration::tests::{complete_native_chunk, native_engines as engines};
use magistr_engine::NativeEngines;
use magistr_engine::native::ChunkStatus;

fn add(engines: &NativeEngines, id: &str, priority: u32, deps: &[&str]) {
    engines
        .roadmap_add_chunk(
            id.into(),
            format!("Chunk {id}"),
            String::new(),
            ChunkStatus::Pending,
            priority,
            String::new(),
            vec![],
            deps.iter().map(|d| (*d).to_string()).collect(),
            None,
            None,
        )
        .expect("add chunk");
}

#[test]
fn counts_chunks_and_names_the_next_ready_one() {
    let tmp = tempfile::tempdir().unwrap();
    let e = engines(&tmp);
    add(&e, "first", 100, &[]);
    add(&e, "second", 200, &["first"]);

    let s = native_spec_summary(&e);
    assert_eq!((s.total, s.checked, s.unchecked), (2, 0, 1));
    assert_eq!(s.unchecked_items, vec!["first — Chunk first".to_string()]);
    assert!(!s.all_done());

    e.roadmap_start_chunk("first").unwrap();
    complete_native_chunk(&e, "first");
    let s = native_spec_summary(&e);
    assert_eq!((s.total, s.checked), (2, 1));
    assert_eq!(s.unchecked_items, vec!["second — Chunk second".to_string()]);
}

#[test]
fn resumes_one_active_item_before_higher_priority_pending_work_without_writes() {
    let tmp = tempfile::tempdir().unwrap();
    let e = engines(&tmp);
    add(&e, "unfinished", 200, &[]);
    add(&e, "urgent", 1, &[]);
    e.roadmap_start_chunk("unfinished").unwrap();
    let before = e.snapshot();
    let summary = native_work_summary(&e, None, None).unwrap();
    assert_eq!(
        (summary.total, summary.checked, summary.unchecked),
        (2, 0, 1)
    );
    assert_eq!(
        summary.unchecked_items,
        ["unfinished — Chunk unfinished (in progress; resume this work)"]
    );
    assert_eq!(
        native_work_chunk(&e, None, None).unwrap().unwrap().id,
        "unfinished"
    );
    assert_eq!(
        e.snapshot(),
        before,
        "selection must not rewrite any native state"
    );
    assert_eq!(
        native_spec_summary(&e).unchecked_items,
        ["urgent — Chunk urgent"]
    );
}

#[test]
fn multiple_active_items_need_a_saved_target_or_matching_unfinished_objective() {
    let tmp = tempfile::tempdir().unwrap();
    let e = engines(&tmp);
    add(&e, "first", 500, &[]);
    add(&e, "second", 1, &[]);
    add(&e, "pending", 0, &[]);
    e.roadmap_start_chunk("first").unwrap();
    e.roadmap_start_chunk("second").unwrap();
    let error = native_work_chunk(&e, None, None).unwrap_err().to_string();
    assert!(
        error.contains("Chunk first") && error.contains("Chunk second"),
        "{error}"
    );
    assert!(error.contains("Choose"), "{error}");
    assert_eq!(
        native_work_chunk(&e, Some("first"), None)
            .unwrap()
            .unwrap()
            .id,
        "first"
    );

    e.ship_set_objective(
        "Finish the second item".into(),
        vec![],
        vec![],
        "chunk:second".into(),
    )
    .unwrap();
    assert_eq!(
        native_work_chunk(&e, None, None).unwrap().unwrap().id,
        "second"
    );
    e.ship_add_task(
        "resume".into(),
        "Continue the second item".into(),
        magistr_engine::native::TaskType::Implement,
        None,
    )
    .unwrap();
    e.ship_start_task("resume").unwrap();
    assert_eq!(e.ship_objective().unwrap().status, ObjectiveStatus::Active);
    assert_eq!(
        native_work_chunk(&e, None, None).unwrap().unwrap().id,
        "second"
    );
    let before = e.snapshot();
    assert_eq!(
        native_work_chunk(&e, Some("first"), None)
            .unwrap()
            .unwrap()
            .id,
        "first"
    );
    assert_eq!(e.snapshot(), before);
}

#[test]
fn saved_pending_target_stays_bound_and_unfinished_objectives_are_preserved() {
    let tmp = tempfile::tempdir().unwrap();
    let e = engines(&tmp);
    add(&e, "chosen", 500, &[]);
    add(&e, "urgent", 1, &[]);
    assert_eq!(
        native_work_chunk(&e, Some("chosen"), None)
            .unwrap()
            .unwrap()
            .id,
        "chosen"
    );
    e.ship_set_objective("Current research".into(), vec![], vec![], "research".into())
        .unwrap();
    let before = e.snapshot();
    assert_eq!(
        native_work_chunk(&e, Some("chosen"), None)
            .unwrap()
            .unwrap()
            .id,
        "chosen"
    );
    assert_eq!(e.snapshot(), before);
}

#[test]
fn unbound_objectives_do_not_imply_a_conflicting_chunk() {
    for scope in ["", "general", " General "] {
        let tmp = tempfile::tempdir().unwrap();
        let e = engines(&tmp);
        add(&e, "only", 1, &[]);
        e.roadmap_start_chunk("only").unwrap();
        e.ship_set_objective(
            "Continue existing work".into(),
            vec![],
            vec![],
            scope.into(),
        )
        .unwrap();
        let before = e.snapshot();
        assert_eq!(
            native_work_chunk(&e, None, None).unwrap().unwrap().id,
            "only"
        );
        assert_eq!(e.snapshot(), before, "unbound objectives remain intact");
    }
}

#[test]
fn an_active_blocker_is_reported_instead_of_skipping_to_pending_work() {
    let tmp = tempfile::tempdir().unwrap();
    let e = engines(&tmp);
    add(&e, "unfinished", 200, &[]);
    add(&e, "ready", 1, &[]);
    e.roadmap_start_chunk("unfinished").unwrap();
    e.roadmap_set_blocked_by(
        "unfinished",
        magistr_engine::native::BlockerKind::AwaitingHuman,
        "Waiting for the recovery recording.".into(),
        None,
    )
    .unwrap();
    let before = e.snapshot();
    for preferred in [None, Some("unfinished")] {
        let error = native_work_chunk(&e, preferred, None)
            .unwrap_err()
            .to_string();
        assert!(error.contains("Chunk unfinished"), "{error}");
        assert!(
            error.contains("Waiting for the recovery recording"),
            "{error}"
        );
    }
    assert_eq!(e.snapshot(), before);
}

#[test]
fn saved_blocked_or_deferred_items_are_not_silently_replaced() {
    for status in [ChunkStatus::Blocked, ChunkStatus::Backlog] {
        let tmp = tempfile::tempdir().unwrap();
        let e = engines(&tmp);
        add(&e, "saved", 200, &[]);
        add(&e, "ready", 1, &[]);
        e.roadmap_set_status("saved", status).unwrap();
        let before = e.snapshot();
        let error = native_work_chunk(&e, Some("saved"), None)
            .unwrap_err()
            .to_string();
        assert!(error.contains("Chunk saved"), "{error}");
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
            e.roadmap_start_chunk("other").unwrap();
            e.ship_set_objective(
                "Finish other work".into(),
                vec![],
                vec![],
                "chunk:other".into(),
            )
            .unwrap();
        }
        let before = e.snapshot();
        let error = native_work_summary(&e, Some("private-missing-id"), None)
            .unwrap_err()
            .to_string();
        assert!(
            error.contains("The work saved for this run is no longer in the roadmap"),
            "{error}"
        );
        assert!(!error.contains("private-missing-id"), "{error}");
        assert_eq!(
            e.snapshot(),
            before,
            "a missing saved target never authorizes other work"
        );
        assert_eq!(
            native_work_chunk(&e, None, None).unwrap().unwrap().id,
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
    add(&e, "recovery", 200, &["storage"]);
    e.roadmap_start_chunk("recovery").unwrap();
    let before = e.snapshot();
    let error = native_work_chunk(&e, None, None).unwrap_err().to_string();
    assert!(
        error.contains("Chunk recovery") && error.contains("Chunk storage"),
        "{error}"
    );
    assert_eq!(e.snapshot(), before);
    e.roadmap_start_chunk("storage").unwrap();
    complete_native_chunk(&e, "storage");
    assert_eq!(
        native_work_chunk(&e, None, None).unwrap().unwrap().id,
        "recovery"
    );

    let mut missing = e.roadmap_chunk("recovery").unwrap();
    missing.deps = vec!["private-missing-identifier".into()];
    let error = ensure_available(&missing, &e.roadmap_chunks())
        .unwrap_err()
        .to_string();
    assert!(error.contains("required work item is missing"), "{error}");
    assert!(!error.contains("private-missing-identifier"), "{error}");
}

#[test]
fn finished_objectives_and_saved_items_do_not_prevent_the_next_ready_item() {
    let tmp = tempfile::tempdir().unwrap();
    let e = engines(&tmp);
    add(&e, "finished", 100, &[]);
    add(&e, "next", 200, &["finished"]);
    e.roadmap_start_chunk("finished").unwrap();
    complete_native_chunk(&e, "finished");
    let before = e.snapshot();
    let summary = native_work_summary(&e, Some("finished"), None).unwrap();
    assert_eq!(summary.unchecked_items, ["next — Chunk next"]);
    assert_eq!(
        (summary.total, summary.checked, summary.unchecked),
        (2, 1, 1)
    );
    assert_eq!(e.snapshot(), before);
    e.ship_complete_task("verify", vec![]).unwrap();
    e.ship_finalize(vec![], None).unwrap();
    e.roadmap_start_chunk("next").unwrap();
    complete_native_chunk(&e, "next");
    let summary = native_work_summary(&e, Some("next"), None).unwrap();
    assert!(summary.all_done());
    assert_eq!(
        (summary.total, summary.checked, summary.unchecked),
        (2, 2, 0)
    );
}

#[test]
fn empty_and_non_actionable_roadmaps_return_no_work() {
    let tmp = tempfile::tempdir().unwrap();
    let e = engines(&tmp);
    assert!(native_work_chunk(&e, None, None).unwrap().is_none());
    let summary = native_work_summary(&e, None, None).unwrap();
    assert_eq!(
        (summary.total, summary.checked, summary.unchecked),
        (0, 0, 0)
    );
    add(&e, "later", 1, &[]);
    e.roadmap_set_status("later", ChunkStatus::Backlog).unwrap();
    assert!(native_work_chunk(&e, None, None).unwrap().is_none());
    e.roadmap_set_status("later", ChunkStatus::Obsoleted)
        .unwrap();
    assert!(
        native_work_chunk(&e, Some("later"), None)
            .unwrap()
            .is_none()
    );
}

#[test]
fn nothing_ready_reads_as_all_done() {
    let tmp = tempfile::tempdir().unwrap();
    let e = engines(&tmp);
    add(&e, "only", 100, &[]);
    e.roadmap_start_chunk("only").unwrap();
    // In progress, not done: no chunk is ready, so the loop must stop.
    let s = native_spec_summary(&e);
    assert_eq!((s.total, s.checked, s.unchecked), (1, 0, 0));
    assert!(s.all_done());
}
