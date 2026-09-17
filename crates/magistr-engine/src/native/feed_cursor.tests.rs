use super::*;
use crate::native::{Delivery, EventCursor, OwnerGeneration};

fn at(epoch: u64, sequence: u64) -> Option<EventCursor> {
    Some(EventCursor {
        epoch: OwnerGeneration::admitted(epoch).expect("nonzero"),
        sequence,
    })
}

#[test]
fn the_first_sequenced_frame_is_the_baseline_and_in_order_frames_follow() {
    let mut cursor = FeedCursor::new();
    assert_eq!(cursor.last(), None);
    assert_eq!(cursor.accept(at(1, 4)), Delivery::First);
    assert_eq!(cursor.accept(at(1, 5)), Delivery::InOrder);
    assert_eq!(cursor.accept(at(1, 6)), Delivery::InOrder);
    assert_eq!(cursor.last(), at(1, 6));
}

#[test]
fn a_repeated_or_earlier_sequence_is_a_duplicate_and_does_not_move_the_cursor() {
    let mut cursor = FeedCursor::new();
    cursor.accept(at(1, 4));
    cursor.accept(at(1, 5));
    assert_eq!(cursor.accept(at(1, 5)), Delivery::Duplicate);
    assert_eq!(cursor.accept(at(1, 2)), Delivery::Duplicate);
    assert_eq!(
        cursor.last(),
        at(1, 5),
        "a duplicate leaves the cursor alone"
    );
    assert_eq!(cursor.accept(at(1, 6)), Delivery::InOrder);
}

#[test]
fn a_jump_in_the_sequence_names_how_many_frames_were_missed() {
    let mut cursor = FeedCursor::new();
    cursor.accept(at(1, 4));
    assert_eq!(cursor.accept(at(1, 7)), Delivery::Gap { missed: 2 });
    assert_eq!(
        cursor.last(),
        at(1, 7),
        "the gap is recorded, not blocked on"
    );
    assert_eq!(cursor.accept(at(1, 8)), Delivery::InOrder);
}

#[test]
fn another_owner_generation_is_a_new_epoch_whatever_its_sequence_says() {
    let mut cursor = FeedCursor::new();
    cursor.accept(at(1, 40));
    // A new publisher starts its own counter at one: not a duplicate.
    assert_eq!(cursor.accept(at(2, 1)), Delivery::NewEpoch);
    assert_eq!(cursor.last(), at(2, 1));
    assert_eq!(cursor.accept(at(2, 2)), Delivery::InOrder);
    // And a frame from the old owner after that is a change back, not a
    // late duplicate: epochs are not ordered against each other.
    assert_eq!(cursor.accept(at(1, 41)), Delivery::NewEpoch);
}

#[test]
fn an_unstamped_frame_is_unsequenced_and_leaves_the_cursor_where_it_was() {
    let mut cursor = FeedCursor::new();
    assert_eq!(cursor.accept(None), Delivery::Unsequenced);
    assert_eq!(cursor.last(), None);
    cursor.accept(at(1, 1));
    assert_eq!(cursor.accept(None), Delivery::Unsequenced);
    assert_eq!(cursor.last(), at(1, 1));
    assert_eq!(cursor.accept(at(1, 2)), Delivery::InOrder);
}

#[test]
fn only_gaps_and_new_epochs_ask_for_a_snapshot_and_only_duplicates_are_skipped() {
    let all = [
        Delivery::Local,
        Delivery::Unsequenced,
        Delivery::First,
        Delivery::InOrder,
        Delivery::Duplicate,
        Delivery::Gap { missed: 1 },
        Delivery::NewEpoch,
    ];
    let snapshots: Vec<Delivery> = all.into_iter().filter(|d| d.needs_snapshot()).collect();
    assert_eq!(
        snapshots,
        vec![Delivery::Gap { missed: 1 }, Delivery::NewEpoch]
    );
    let skipped: Vec<Delivery> = all.into_iter().filter(|d| d.is_duplicate()).collect();
    assert_eq!(skipped, vec![Delivery::Duplicate]);
}
