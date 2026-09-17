use super::*;
use crate::medium::Medium;

/// The chosen segment is the filled one — exactly one — and every
/// segment answers on its own sequential id.
#[test]
fn one_segment_is_filled_and_each_answers_its_own_id() {
    let lex = Lexicon::of(Medium::Night);
    let group = Segments::el(
        &["The sky", "The chart", "Follow OS"],
        2,
        ElementId(30),
        &lex,
    );
    let filled: Vec<bool> = group
        .children
        .iter()
        .map(|segment| segment.fill == Some(lex.mauve))
        .collect();
    assert_eq!(
        filled,
        vec![false, false, true],
        "the chosen one filled — and only it"
    );
    let mut ids = Vec::new();
    group.each(&mut |_, element| ids.extend(element.id));
    assert_eq!(ids, vec![ElementId(30), ElementId(31), ElementId(32)]);
}

/// Flat, never raised: no segment and no shell carries an elevation or
/// a gradient — a choice is not an action.
#[test]
fn a_segment_group_is_flat() {
    let lex = Lexicon::of(Medium::Night);
    let group = Segments::el(&["A", "B"], 0, ElementId(1), &lex);
    group.each(&mut |_, element| {
        assert!(element.gradient.is_none());
        assert!(element.elevation.is_flat());
    });
}
