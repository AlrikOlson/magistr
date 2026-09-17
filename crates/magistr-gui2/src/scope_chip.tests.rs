use super::*;
use crate::element::Kind;
use crate::medium::Medium;

fn ink(root: &Element) -> Option<Srgba> {
    let mut ink = None;
    root.each(&mut |_, element| {
        if matches!(element.kind, Kind::Text { .. }) {
            ink = element.fill;
        }
    });
    ink
}

#[test]
fn the_picked_chip_is_washed_and_ringless_in_the_text_ink_the_rest_ringed_and_subtle() {
    let lex = Lexicon::of(Medium::Night);
    let picked = ScopeChip::el("everything", true, lex.base, ElementId(7), &lex);
    assert!(picked.fill.is_some());
    assert!(picked.stroke.is_none(), "the picked chip's ring is gone");
    assert_eq!(ink(&picked), Some(lex.tx));
    let idle = ScopeChip::el("reports", false, lex.base, ElementId(8), &lex);
    assert!(idle.fill.is_none());
    assert!(idle.stroke.is_some());
    assert_eq!(ink(&idle), Some(lex.sb0));
    assert!(idle.hover_fill.is_some(), "an idle chip raises on hover");
    assert_eq!(idle.id, Some(ElementId(8)));
    assert!(idle.focusable);
}

#[test]
fn a_chip_keeps_its_border_box_whether_or_not_the_ring_is_drawn() {
    let lex = Lexicon::of(Medium::Day);
    let picked = ScopeChip::el("trace", true, lex.base, ElementId(1), &lex);
    let idle = ScopeChip::el("trace", false, lex.base, ElementId(2), &lex);
    assert_eq!(picked.pad, idle.pad);
    assert_eq!(picked.pad, [12.0, 4.0, 12.0, 4.0]);
}
