use super::*;

/// Every wire status the roadmap carries names its own state, and an
/// unknown one is still a star.
#[test]
fn every_status_names_a_state() {
    assert_eq!(StarState::of(ChunkState::Done, false), StarState::Done);
    assert_eq!(StarState::of(ChunkState::Blocked, true), StarState::Blocked);
    assert_eq!(StarState::of(ChunkState::Stuck, false), StarState::Stuck);
    assert_eq!(
        StarState::of(ChunkState::InProgress, false),
        StarState::Executing
    );
    assert_eq!(StarState::of(ChunkState::Pending, true), StarState::Ready);
    assert_eq!(
        StarState::of(ChunkState::Pending, false),
        StarState::Pending
    );
    assert_eq!(
        StarState::of(ChunkState::Backlog, false),
        StarState::Pending
    );
    // The plan keeps obsoleted history; the sky reads it as a planned star.
    assert_eq!(
        StarState::of(ChunkState::Obsoleted, false),
        StarState::Pending
    );
}

/// `ready` decides between exactly two states and nothing else: a done
/// chunk whose deps happen to be done is still done.
#[test]
fn readiness_only_separates_pending_from_ready() {
    for state in [
        ChunkState::Done,
        ChunkState::Blocked,
        ChunkState::Stuck,
        ChunkState::InProgress,
    ] {
        assert_eq!(
            StarState::of(state, true),
            StarState::of(state, false),
            "{state:?} read its readiness"
        );
    }
}

/// The seven are distinct, and `ALL` holds each exactly once.
#[test]
fn the_vocabulary_is_seven_distinct_states() {
    let mut seen = std::collections::HashSet::new();
    for state in StarState::ALL {
        assert!(seen.insert(state), "{} appears twice", state.label());
    }
    assert_eq!(seen.len(), 7);
}

/// Only the shipped star twinkles — that is the whole of "shipped stars
/// stay lit", and the reason day can refuse the motion by refusing one
/// predicate.
#[test]
fn only_the_shipped_star_twinkles() {
    let twinkling: Vec<_> = StarState::ALL
        .into_iter()
        .filter(|s| s.twinkles())
        .collect();
    assert_eq!(twinkling, vec![StarState::Done]);
}

/// The alert ring is worn by the two states that need a person, and the
/// core is what tells those two apart.
#[test]
fn the_alert_ring_is_worn_by_stuck_and_blocked_and_the_core_separates_them() {
    let alerting: Vec<_> = StarState::ALL.into_iter().filter(|s| s.alerts()).collect();
    assert_eq!(alerting, vec![StarState::Stuck, StarState::Blocked]);
    assert!(StarState::Stuck.breathes());
    assert!(!StarState::Blocked.breathes());
}

/// A state that has no core is one nothing has happened to yet.
#[test]
fn only_the_untouched_states_are_outlines() {
    let hollow: Vec<_> = StarState::ALL
        .into_iter()
        .filter(|s| !s.has_core())
        .collect();
    assert_eq!(hollow, vec![StarState::Ghost, StarState::Pending]);
}

/// No state carries two periodic motions at once — one motion per star,
/// keeping one dominant animation per screen.
#[test]
fn no_state_carries_two_periodic_motions() {
    for state in StarState::ALL {
        let motions =
            u8::from(state.twinkles()) + u8::from(state.pulses()) + u8::from(state.breathes());
        assert!(motions <= 1, "{} carries {motions} motions", state.label());
    }
}
