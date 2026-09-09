use std::path::PathBuf;

use super::*;
use crate::agent::multi::topology::AgentId;

#[test]
fn no_conflicts_initially() {
    let detector = ConflictDetector::new();
    assert!(detector.detect_conflicts().is_empty());
    assert_eq!(detector.tracked_file_count(), 0);
}

#[test]
fn single_agent_no_conflict() {
    let mut detector = ConflictDetector::new();
    let agent = AgentId::new(1);

    detector.claim_file(agent, PathBuf::from("src/main.rs"));
    detector.claim_file(agent, PathBuf::from("src/lib.rs"));

    assert!(detector.detect_conflicts().is_empty());
    assert_eq!(detector.tracked_file_count(), 2);
    assert!(!detector.has_conflict(&PathBuf::from("src/main.rs")));
}

#[test]
fn two_agents_same_file_is_conflict() {
    let mut detector = ConflictDetector::new();
    let a1 = AgentId::new(1);
    let a2 = AgentId::new(2);
    let file = PathBuf::from("src/shared.rs");

    detector.claim_file(a1, file.clone());
    detector.claim_file(a2, file.clone());

    let conflicts = detector.detect_conflicts();
    assert_eq!(conflicts.len(), 1);
    assert_eq!(conflicts[0].file, file);
    assert_eq!(conflicts[0].agents.len(), 2);
    assert!(detector.has_conflict(&file));
}

#[test]
fn release_file_resolves_conflict() {
    let mut detector = ConflictDetector::new();
    let a1 = AgentId::new(1);
    let a2 = AgentId::new(2);
    let file = PathBuf::from("src/shared.rs");

    detector.claim_file(a1, file.clone());
    detector.claim_file(a2, file.clone());
    assert!(detector.has_conflict(&file));

    detector.release_file(a1, &file);
    assert!(!detector.has_conflict(&file));
    assert_eq!(detector.tracked_file_count(), 1);
}

#[test]
fn release_all_clears_agent_claims() {
    let mut detector = ConflictDetector::new();
    let a1 = AgentId::new(1);
    let a2 = AgentId::new(2);

    detector.claim_file(a1, PathBuf::from("a.rs"));
    detector.claim_file(a1, PathBuf::from("b.rs"));
    detector.claim_file(a2, PathBuf::from("b.rs"));

    detector.release_all(a1);
    assert!(!detector.has_conflict(&PathBuf::from("b.rs")));
    // a.rs had only a1's claim, so it's removed entirely
    assert_eq!(detector.tracked_file_count(), 1);
}

#[test]
fn agents_for_file() {
    let mut detector = ConflictDetector::new();
    let a1 = AgentId::new(1);
    let a2 = AgentId::new(2);
    let file = PathBuf::from("src/main.rs");

    detector.claim_file(a1, file.clone());
    detector.claim_file(a2, file.clone());

    let agents = detector.agents_for_file(&file);
    assert_eq!(agents.len(), 2);
    assert!(agents.contains(&a1));
    assert!(agents.contains(&a2));

    let empty = detector.agents_for_file(&PathBuf::from("nonexistent.rs"));
    assert!(empty.is_empty());
}

#[test]
fn conflict_resolution_display() {
    assert_eq!(ConflictResolution::Serialize.to_string(), "serialize");
    assert_eq!(ConflictResolution::Merge.to_string(), "merge");
    assert_eq!(
        ConflictResolution::LastWriteWins.to_string(),
        "last-write-wins"
    );
}

#[test]
fn default_resolution_strategy() {
    let detector = ConflictDetector::new();
    assert!(detector.default_resolution().is_none());

    let detector = ConflictDetector::with_default_resolution(ConflictResolution::Merge);
    assert_eq!(
        detector.default_resolution(),
        Some(ConflictResolution::Merge)
    );
}

#[test]
fn record_edit_returns_none_for_first_claim() {
    let mut detector = ConflictDetector::new();
    let a1 = AgentId::new(1);
    let conflict = detector.record_edit(a1, PathBuf::from("src/main.rs"));
    assert!(conflict.is_none());
}

#[test]
fn record_edit_returns_conflict_on_second_agent() {
    let mut detector = ConflictDetector::new();
    let a1 = AgentId::new(1);
    let a2 = AgentId::new(2);
    let file = PathBuf::from("src/shared.rs");

    assert!(detector.record_edit(a1, file.clone()).is_none());
    let conflict = detector.record_edit(a2, file.clone());
    assert!(conflict.is_some());

    let c = conflict.expect("should have conflict");
    assert_eq!(c.file, file);
    assert_eq!(c.agents.len(), 2);
    assert!(c.agents.contains(&a1));
    assert!(c.agents.contains(&a2));
}

#[test]
fn record_edit_same_agent_twice_no_conflict() {
    let mut detector = ConflictDetector::new();
    let a1 = AgentId::new(1);
    let file = PathBuf::from("src/mine.rs");

    assert!(detector.record_edit(a1, file.clone()).is_none());
    assert!(detector.record_edit(a1, file).is_none());
}
