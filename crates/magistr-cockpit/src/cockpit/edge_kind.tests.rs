use super::*;

/// An edge is walked only when the chunk it leaves has shipped.
#[test]
fn only_a_shipped_dependency_draws_a_solid_edge() {
    use crate::cockpit::ChunkState;
    assert_eq!(EdgeKind::of(ChunkState::Done), EdgeKind::Done);
    for state in ChunkState::ALL.into_iter().filter(|state| !state.is_done()) {
        assert_eq!(EdgeKind::of(state), EdgeKind::Ahead, "{state:?}");
    }
}

/// The dashes are the "ahead" kind and only that kind.
#[test]
fn the_dashes_are_the_road_not_yet_walked() {
    assert!(!EdgeKind::Done.is_dashed());
    assert!(EdgeKind::Ahead.is_dashed());
    assert_eq!(EdgeKind::ALL.len(), 2);
}
