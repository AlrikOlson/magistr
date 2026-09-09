use super::*;
use crate::element_layout::ElementLayout;
use crate::text::TextEngine;

/// The thumb sits left when off and right when on — placed, not
/// implied: the layout puts it at the volume's own 2.5px insets.
#[test]
fn the_thumb_crosses_the_track_with_the_state() {
    let lex = Lexicon::of(crate::medium::Medium::Night);
    let mut text = TextEngine::embedded();
    let off = Switch::el(false, ElementId(1), &lex);
    let layout = ElementLayout::compute(&off, 32.0, 19.0, &mut text).expect("layout");
    assert_eq!(
        layout.rect(1),
        [2.5, 3.5, 14.0, 14.0],
        "the shadow, 1px low"
    );
    assert_eq!(layout.rect(2), [2.5, 2.5, 14.0, 14.0], "the thumb");
    let on = Switch::el(true, ElementId(1), &lex);
    let layout = ElementLayout::compute(&on, 32.0, 19.0, &mut text).expect("layout");
    assert_eq!(layout.rect(2), [32.0 - 2.5 - 14.0, 2.5, 14.0, 14.0]);
}

/// The track wears the accent only while on.
#[test]
fn the_track_wears_the_accent_only_on() {
    let lex = Lexicon::of(crate::medium::Medium::Night);
    assert_eq!(Switch::el(true, ElementId(1), &lex).fill, Some(lex.mauve));
    assert_eq!(Switch::el(false, ElementId(1), &lex).fill, Some(lex.s1));
}
