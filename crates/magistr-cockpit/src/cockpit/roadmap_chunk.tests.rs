use serde_json::json;

use super::*;

#[test]
fn a_full_payload_parses_every_field() {
    let chunk = RoadmapChunk::from_payload(&json!({
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
    }))
    .expect("id present");
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
fn a_bare_payload_needs_only_the_id() {
    let chunk = RoadmapChunk::from_payload(&json!({"id": "x"})).expect("id present");
    assert_eq!(chunk.name, "x", "the name falls back to the id");
    assert!(chunk.is_active());
    assert!(!chunk.is_in_progress());
    assert!(chunk.proposals.is_empty());
    assert!(RoadmapChunk::from_payload(&json!({"title": "no id"})).is_none());
}

#[test]
fn the_bands_follow_the_named_levels() {
    let at = |priority: u32| {
        RoadmapChunk::from_payload(&json!({"id": "x", "priority": priority}))
            .expect("id")
            .band()
    };
    assert_eq!(at(50), "critical");
    assert_eq!(at(200), "high");
    assert_eq!(at(300), "medium");
    assert_eq!(at(400), "low");
}

#[test]
fn the_detail_rows_count_the_title_the_prose_the_bullets_and_the_links() {
    let chunk = RoadmapChunk::from_payload(&json!({
        "id": "c1",
        "title": "a claim",
        "description": "one\n\ntwo",
        "acceptance": ["a", "b"],
        "deps": ["d1"],
        "cross_refs": ["think:1"],
    }))
    .expect("chunk");
    // title + two prose lines (the empty one drops) + two bullets + deps + refs
    assert_eq!(chunk.detail_rows(), 7);
}

#[test]
fn a_bare_chunk_details_to_its_title_alone() {
    let chunk = RoadmapChunk::from_payload(&json!({ "id": "c1" })).expect("chunk");
    assert_eq!(chunk.detail_rows(), 1);
}
