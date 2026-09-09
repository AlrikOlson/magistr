use super::*;

/// An edge is walked only when the chunk it leaves has shipped.
#[test]
fn only_a_shipped_dependency_draws_a_solid_edge() {
    assert_eq!(EdgeKind::of("done"), EdgeKind::Done);
    for status in ["pending", "in_progress", "blocked", "stuck", "backlog", ""] {
        assert_eq!(EdgeKind::of(status), EdgeKind::Ahead, "{status}");
    }
}

/// The dashes are the "ahead" kind and only that kind.
#[test]
fn the_dashes_are_the_road_not_yet_walked() {
    assert!(!EdgeKind::Done.is_dashed());
    assert!(EdgeKind::Ahead.is_dashed());
    assert_eq!(EdgeKind::ALL.len(), 2);
}
