use super::*;

/// A representative card: 71 columns of a 640 px card at a 9 px cell,
/// two-digit line numbers, a 60-character widest line.
const CARD: (usize, usize, usize) = (71, 2, 60);

#[test]
fn a_pane_with_room_for_two_whole_columns_splits() {
    let (_, gutter, widest) = CARD;
    let both = 2 * (gutter + 1 + widest) + DiffLayout::SPLIT_GAP;
    assert_eq!(
        DiffLayout::for_width(both, gutter, widest),
        DiffLayout::Split
    );
    assert_ne!(
        DiffLayout::for_width(both - 1, gutter, widest),
        DiffLayout::Split
    );
}

#[test]
fn the_volumes_own_card_is_one_column_with_its_numbers() {
    let (cols, gutter, widest) = CARD;
    assert_eq!(
        DiffLayout::for_width(cols, gutter, widest),
        DiffLayout::Gutter
    );
}

#[test]
fn a_pane_that_cannot_hold_the_numbers_keeps_the_text() {
    let (_, gutter, widest) = CARD;
    // One column short of the gutter reading.
    let tight = 2 * gutter + 1 + widest;
    assert_eq!(
        DiffLayout::for_width(tight, gutter, widest),
        DiffLayout::Compact
    );
    assert!(!DiffLayout::Compact.has_gutter());
}

#[test]
fn every_reading_that_is_not_compact_prints_its_numbers() {
    assert!(DiffLayout::Split.has_gutter());
    assert!(DiffLayout::Gutter.has_gutter());
    assert!(DiffLayout::Split.is_split());
    assert!(!DiffLayout::Gutter.is_split());
}

#[test]
fn a_pane_of_no_width_still_answers() {
    assert_eq!(DiffLayout::for_width(0, 4, 200), DiffLayout::Compact);
}
