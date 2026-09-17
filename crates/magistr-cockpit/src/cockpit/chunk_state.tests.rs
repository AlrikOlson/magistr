use serde_json::json;

use super::*;
use crate::harness::fixtures;

fn state_of(record: serde_json::Value) -> ChunkState {
    ChunkState::of(&fixtures::chunk_from(record))
}

/// Every store status names its own state, and the one the store spells
/// as two fields — in progress with a blocker — is Stuck.
#[test]
fn every_status_reads_as_one_state_and_a_blocked_in_progress_chunk_is_stuck() {
    assert_eq!(
        state_of(json!({"id": "b", "status": "backlog"})),
        ChunkState::Backlog
    );
    assert_eq!(
        state_of(json!({"id": "p", "status": "pending"})),
        ChunkState::Pending
    );
    assert_eq!(
        state_of(json!({"id": "i", "status": "in_progress"})),
        ChunkState::InProgress
    );
    assert_eq!(
        state_of(json!({"id": "k", "status": "blocked"})),
        ChunkState::Blocked
    );
    assert_eq!(
        state_of(json!({"id": "d", "status": "done"})),
        ChunkState::Done
    );
    assert_eq!(
        state_of(json!({"id": "o", "status": "obsoleted"})),
        ChunkState::Obsoleted
    );
    assert_eq!(
        state_of(json!({
            "id": "s", "status": "in_progress",
            "blocked_by": {"kind": "awaiting_human", "reason": "needs a decision"},
        })),
        ChunkState::Stuck
    );
    assert_eq!(
        state_of(json!({
            "id": "q", "status": "pending",
            "blocked_by": {"kind": "external", "reason": "upstream"},
        })),
        ChunkState::Pending,
        "a blocker on pending work is not stuck"
    );
}

/// Active is everything but obsoleted; working is in progress with or
/// without a blocker; done is done.
#[test]
fn the_predicates_partition_the_vocabulary() {
    let inactive: Vec<_> = ChunkState::ALL
        .into_iter()
        .filter(|state| !state.is_active())
        .collect();
    assert_eq!(inactive, vec![ChunkState::Obsoleted]);
    let working: Vec<_> = ChunkState::ALL
        .into_iter()
        .filter(|state| state.is_working())
        .collect();
    assert_eq!(working, vec![ChunkState::InProgress, ChunkState::Stuck]);
    let done: Vec<_> = ChunkState::ALL
        .into_iter()
        .filter(|state| state.is_done())
        .collect();
    assert_eq!(done, vec![ChunkState::Done]);
}

/// The seven words are the roadmap's own spelling, each distinct.
#[test]
fn the_words_are_the_roadmaps_spelling() {
    let words: Vec<&str> = ChunkState::ALL.iter().map(|state| state.word()).collect();
    assert_eq!(
        words,
        [
            "backlog",
            "pending",
            "in_progress",
            "stuck",
            "blocked",
            "done",
            "obsoleted"
        ]
    );
    let mut seen = std::collections::HashSet::new();
    for state in ChunkState::ALL {
        assert!(seen.insert(state), "{} appears twice", state.word());
    }
}
