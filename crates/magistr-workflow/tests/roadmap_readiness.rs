//! Start admission uses the same current records as next-work hints.
use magistr_workflow::roadmap::RoadmapEngine;
use magistr_workflow::roadmap::domain::{BlockedBy, BlockerKind, ChunkStatus};

fn blocker(kind: BlockerKind, reason: &str) -> BlockedBy {
    BlockedBy {
        kind,
        reason: reason.into(),
        evidence: None,
        blocked_at: "2026-10-03T00:00:00Z".into(),
    }
}

fn add(engine: &mut RoadmapEngine, id: &str, priority: u32, deps: &[&str]) {
    engine
        .add_chunk(
            id.into(),
            format!("Work {id}"),
            ChunkStatus::Pending,
            priority,
            String::new(),
            vec![],
            deps.iter().map(|id| (*id).into()).collect(),
            false,
        )
        .unwrap();
    engine.set_group(id, Some("Delivery".into())).unwrap();
}

#[test]
fn start_and_status_admission_reject_waiting_work_without_changing_records() {
    let mut engine = RoadmapEngine::new("project".into());
    add(&mut engine, "foundation", 20, &[]);
    add(&mut engine, "waiting", 1, &["foundation", "absent"]);
    let before = engine.roadmap().clone();
    let error = engine
        .start_chunk("waiting")
        .expect_err("dependencies must be completed");
    assert!(
        error.to_string().to_lowercase().contains("foundation")
            && error.to_string().contains("unavailable"),
        "{error}"
    );
    assert_eq!(engine.roadmap(), &before);
    assert!(
        engine
            .set_status("waiting", ChunkStatus::InProgress)
            .is_err()
    );
    assert_eq!(engine.roadmap(), &before);
}

#[test]
fn blockers_and_status_proposals_cannot_bypass_start_admission() {
    let mut engine = RoadmapEngine::new("project".into());
    add(&mut engine, "work", 1, &[]);
    engine
        .set_blocked_by(
            "work",
            blocker(BlockerKind::AwaitingHuman, "Access is needed."),
        )
        .unwrap();
    engine
        .propose_status(
            "work",
            ChunkStatus::InProgress,
            "Start now".into(),
            "tracker".into(),
        )
        .unwrap();
    let before = engine.roadmap().clone();
    assert!(engine.start_chunk("work").is_err());
    assert!(engine.resolve_status_proposal("work", true).is_err());
    assert_eq!(engine.roadmap(), &before);
    engine.clear_blocked_by("work").unwrap();
    engine.resolve_status_proposal("work", true).unwrap();
}

#[test]
fn stale_hint_is_rechecked_by_the_writer_and_ties_keep_snapshot_order() {
    let mut engine = RoadmapEngine::new("project".into());
    add(&mut engine, "z-first", 10, &[]);
    add(&mut engine, "a-second", 10, &[]);
    let hinted = engine.next().unwrap().id.clone();
    assert_eq!(hinted, "z-first");
    assert_eq!(engine.next_in_group("Delivery").unwrap().id, hinted);
    assert_eq!(
        engine
            .workstream_frontier(Some("Delivery"))
            .activity
            .readiness
            .ready,
        2
    );
    engine
        .set_blocked_by(
            &hinted,
            blocker(BlockerKind::External, "Access was revoked."),
        )
        .unwrap();
    assert!(
        engine.start_chunk(&hinted).is_err(),
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
    let mut engine = RoadmapEngine::new("project".into());
    add(&mut engine, "foundation", 10, &[]);
    add(&mut engine, "work", 20, &["foundation"]);
    engine.start_chunk("foundation").unwrap();
    engine.complete_chunk("foundation", None).unwrap();
    engine.start_chunk("work").unwrap();
    engine.complete_chunk("work", None).unwrap();
    assert!(engine.start_chunk("work").is_err());
    engine.set_status("work", ChunkStatus::InProgress).unwrap();
    assert_eq!(engine.roadmap().chunks[1].status, ChunkStatus::InProgress);
}

#[test]
fn completed_work_closes_saved_focus_without_redirecting_or_mutating_it() {
    use magistr_workflow::roadmap::domain::{FocusMode, FocusState};
    let mut engine = RoadmapEngine::new("project".into());
    add(&mut engine, "foundation", 10, &[]);
    engine.set_group("foundation", Some("Rust".into())).unwrap();
    engine.focus_set("lane", "Rust", FocusMode::Build).unwrap();
    add(&mut engine, "desktop", 20, &[]);
    engine
        .set_group("desktop", Some("Rust desktop".into()))
        .unwrap();
    engine.start_chunk("foundation").unwrap();
    engine.complete_chunk("foundation", None).unwrap();

    let before = engine.roadmap().clone();
    let catalog = engine.workstreams();
    assert_eq!(
        catalog.focus_state(engine.focus_get("lane").map(|focus| focus.group.as_str())),
        FocusState::Closed {
            group: "Rust".into()
        }
    );
    assert_eq!(catalog.current_names(), ["Rust desktop"]);
    assert_eq!(
        engine.workstream_history(Some("Rust")).chunk_ids,
        ["foundation"]
    );
    assert!(engine.workstream_frontier(Some("Rust")).items.is_empty());
    assert!(engine.next_in_group("Rust").is_none());
    for query in ["Rust", "rust"] {
        let error = engine
            .focus_set("lane", query, FocusMode::Shape)
            .unwrap_err();
        assert!(
            error.to_string().contains("no unfinished work")
                && error.to_string().contains("Rust desktop")
        );
        assert_eq!(engine.roadmap(), &before);
    }

    engine
        .set_group("foundation", Some("Archive".into()))
        .unwrap();
    assert_eq!(
        engine.workstreams().focus_state(Some("Rust")),
        FocusState::Missing {
            group: "Rust".into()
        }
    );
    assert_eq!(engine.focus_get("lane").unwrap().group, "Rust");
    engine
        .set_status("foundation", ChunkStatus::InProgress)
        .unwrap();
    assert_eq!(
        engine.workstreams().current_names(),
        ["Archive", "Rust desktop"]
    );
    assert!(!engine.workstreams().history.contains_key("Archive"));
}

#[test]
fn current_grouping_proposals_never_enlist_or_change_terminal_records() {
    let mut engine = RoadmapEngine::new("project".into());
    for (id, status) in [
        ("old-one", ChunkStatus::Done),
        ("old-two", ChunkStatus::Obsoleted),
        ("new-one", ChunkStatus::Backlog),
        ("new-two", ChunkStatus::Pending),
        ("new-finished", ChunkStatus::Done),
        ("new-grouped", ChunkStatus::Pending),
    ] {
        engine
            .add_chunk(
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
        .set_group("new-grouped", Some("Delivery".into()))
        .unwrap();
    let before = engine.roadmap().clone();
    let proposals = engine.propose_groups_from_ids(2);
    assert_eq!(proposals.len(), 1);
    assert_eq!(proposals[0].prefix, "new");
    assert_eq!(proposals[0].chunk_ids, ["new-one", "new-two"]);
    assert_eq!(engine.roadmap(), &before, "discovery writes nothing");
    assert_eq!(
        engine.workstream_history(None).chunk_ids,
        ["old-one", "old-two", "new-finished"]
    );
}
