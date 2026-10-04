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
        error.to_lowercase().contains("foundation") && error.contains("unavailable"),
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
    assert_eq!(engine.group_status("Delivery")["ready_count"], 2);
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
    assert_eq!(engine.group_status("Delivery")["ready_count"], 1);
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
