use super::*;

#[test]
fn nothing_read_is_unavailable_and_a_snapshot_makes_it_current() {
    let mut projection = Projection::default();
    assert_eq!(projection.freshness(), Freshness::Unavailable);
    assert_eq!(projection.last_delivery(), None);
    projection.seeded();
    assert_eq!(projection.freshness(), Freshness::Current);
}

#[test]
fn in_order_events_are_folded_and_a_first_event_alone_makes_the_view_current() {
    let mut projection = Projection::default();
    assert!(projection.delivered(Delivery::First));
    assert_eq!(projection.freshness(), Freshness::Current);
    assert!(projection.delivered(Delivery::InOrder));
    assert!(projection.delivered(Delivery::Local));
    assert!(projection.delivered(Delivery::Unsequenced));
    assert_eq!(projection.freshness(), Freshness::Current);
    assert_eq!(projection.last_delivery(), Some(Delivery::Unsequenced));
}

#[test]
fn a_duplicate_is_not_folded_and_leaves_the_freshness_alone() {
    let mut projection = Projection::default();
    projection.seeded();
    assert!(!projection.delivered(Delivery::Duplicate));
    assert_eq!(projection.freshness(), Freshness::Current);
    assert_eq!(projection.last_delivery(), Some(Delivery::Duplicate));
}

#[test]
fn a_gap_or_a_new_owner_is_folded_but_marks_the_view_stale_until_a_snapshot() {
    let mut projection = Projection::default();
    projection.seeded();
    assert!(projection.delivered(Delivery::Gap { missed: 3 }));
    assert_eq!(projection.freshness(), Freshness::Stale);
    // In-order events after a gap do not repair it: only the store can.
    assert!(projection.delivered(Delivery::InOrder));
    assert_eq!(projection.freshness(), Freshness::Stale);
    projection.seeded();
    assert_eq!(projection.freshness(), Freshness::Current);
    assert!(projection.delivered(Delivery::NewEpoch));
    assert_eq!(projection.freshness(), Freshness::Stale);
}

#[test]
fn a_hangup_makes_a_read_view_stale_and_a_reattachment_alone_does_not_restore_it() {
    let mut projection = Projection::default();
    // Before anything was read there is nothing to go stale.
    projection.feed(FeedStatus::Disconnected);
    assert_eq!(projection.freshness(), Freshness::Unavailable);
    projection.seeded();
    projection.feed(FeedStatus::Waiting);
    projection.feed(FeedStatus::Connected { reconnect: false });
    assert_eq!(projection.freshness(), Freshness::Current);
    projection.feed(FeedStatus::Disconnected);
    assert_eq!(projection.freshness(), Freshness::Stale);
    projection.feed(FeedStatus::Connected { reconnect: true });
    assert_eq!(
        projection.freshness(),
        Freshness::Stale,
        "reattaching promises nothing; the snapshot it asks for does"
    );
    projection.seeded();
    assert_eq!(projection.freshness(), Freshness::Current);
}
