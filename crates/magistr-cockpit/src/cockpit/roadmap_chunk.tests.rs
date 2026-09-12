use serde_json::json;

use super::*;
use crate::harness::fixtures;

#[test]
fn a_full_record_reads_every_field() {
    let chunk = RoadmapChunk::from_chunk(&fixtures::chunk_from(json!({
        "id": "auth",
        "name": "Auth",
        "title": "Authenticate users",
        "status": "pending",
        "priority": 200,
        "group": "Identity",
        "deps": ["base"],
        "description": "who are you",
        "acceptance": ["logs in", "logs out"],
        "cross_refs": ["think:7"],
        "reprioritize": {"suggested_priority": 100, "reason": "sooner"},
        "status_proposal": {"suggested_status": "in_progress", "reason": "started"},
        "title_proposal": {"suggested_title": "Better", "reason": "clearer"},
    })));
    assert_eq!(chunk.name, "Auth");
    assert_eq!(chunk.band(), "high");
    assert_eq!(chunk.group.as_deref(), Some("Identity"));
    assert_eq!(chunk.deps, vec!["base"]);
    assert_eq!(chunk.acceptance.len(), 2);
    assert_eq!(chunk.cross_refs, vec!["think:7"]);
    let kinds: Vec<ProposalKind> = chunk.proposals.iter().map(|(k, _, _)| *k).collect();
    assert_eq!(
        kinds,
        vec![
            ProposalKind::Reprioritize,
            ProposalKind::Status,
            ProposalKind::Title
        ]
    );
    assert_eq!(chunk.proposals[0].1, "100", "a number reads as words");
    assert_eq!(chunk.proposals[2].2, "clearer");
}

#[test]
fn a_bare_record_names_itself_by_its_id() {
    let chunk = RoadmapChunk::from_chunk(&fixtures::chunk_from(json!({"id": "x"})));
    assert_eq!(chunk.name, "x", "the name falls back to the id");
    assert!(chunk.is_active());
    assert!(!chunk.is_in_progress());
    assert!(chunk.proposals.is_empty());
}

#[test]
fn the_bands_follow_the_named_levels() {
    let at = |priority: u32| {
        RoadmapChunk::from_chunk(&fixtures::chunk_from(
            json!({"id": "x", "priority": priority}),
        ))
        .band()
    };
    assert_eq!(at(50), "critical");
    assert_eq!(at(200), "high");
    assert_eq!(at(300), "medium");
    assert_eq!(at(400), "low");
}

#[test]
fn the_detail_rows_count_the_title_the_prose_the_bullets_and_the_links() {
    let chunk = RoadmapChunk::from_chunk(&fixtures::chunk_from(json!({
        "id": "c1",
        "title": "a claim",
        "description": "one\n\ntwo",
        "acceptance": ["a", "b"],
        "deps": ["d1"],
        "cross_refs": ["think:1"],
    })));
    // title + two prose lines (the empty one drops) + two bullets + deps + refs
    assert_eq!(chunk.detail_rows(), 7);
}

#[test]
fn a_bare_chunk_details_to_its_title_alone() {
    let chunk = RoadmapChunk::from_chunk(&fixtures::chunk_from(json!({ "id": "c1" })));
    assert_eq!(chunk.detail_rows(), 1);
}

/// The store spells "stuck" as two fields: a chunk in progress that carries
/// a blocker. The row spells it as the one word the sky and the list use.
#[test]
fn work_in_progress_behind_a_blocker_reads_as_stuck() {
    let stuck = RoadmapChunk::from_chunk(&fixtures::chunk_from(json!({
        "id": "s", "status": "in_progress",
        "blocked_by": {"kind": "awaiting_human", "reason": "needs a decision"},
    })));
    assert_eq!(stuck.status, "stuck");
    assert!(stuck.is_active());
    assert!(!stuck.is_in_progress(), "stuck is not plain progress");
    let pending = RoadmapChunk::from_chunk(&fixtures::chunk_from(json!({
        "id": "p", "status": "pending",
        "blocked_by": {"kind": "external", "reason": "upstream"},
    })));
    assert_eq!(
        pending.status, "pending",
        "a blocker on pending work is not stuck"
    );
    let done =
        RoadmapChunk::from_chunk(&fixtures::chunk_from(json!({"id": "d", "status": "done"})));
    assert_eq!(done.status, "done");
}

#[test]
fn the_updated_stamp_is_read_as_seconds_and_a_blank_one_as_none() {
    let stamped = RoadmapChunk::from_chunk(&fixtures::chunk_from(json!({
        "id": "s", "updated_at": "2026-08-30T12:00:00Z",
    })));
    assert_eq!(stamped.updated_secs, Some(1_788_091_200));
    assert_eq!(fixtures::chunk("u").updated_at, "");
    assert_eq!(
        RoadmapChunk::from_chunk(&fixtures::chunk("u")).updated_secs,
        None
    );
}
