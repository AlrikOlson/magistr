use super::*;
use crate::medium::Medium;

/// The active item lights its wash, its mark and its label; an idle
/// one stays quiet — and both are navigation, answering on their id.
#[test]
fn active_lights_wash_mark_and_label() {
    let lex = Lexicon::of(Medium::Night);
    let on = SettingsNav::el(
        Icon::Image,
        "Appearance",
        true,
        lex.base,
        ElementId(60),
        &lex,
    );
    let off = SettingsNav::el(
        Icon::Session,
        "Motion",
        false,
        lex.base,
        ElementId(61),
        &lex,
    );
    assert!(on.fill.is_some() && off.fill.is_none());
    assert_eq!(on.id, Some(ElementId(60)));
    assert!(on.focusable && off.focusable);
    let anatomy = |item: &Element| {
        let mut mark_ink = None;
        let mut label = None;
        item.each(&mut |_, element| match &element.kind {
            crate::element::Kind::Mark { ink, .. } => mark_ink = Some(*ink),
            crate::element::Kind::Text { voice, .. } => label = Some((element.fill, voice.weight)),
            crate::element::Kind::Surface
            | crate::element::Kind::Picture { .. }
            | crate::element::Kind::Grid { .. } => {}
        });
        (mark_ink.expect("a mark"), label.expect("a label"))
    };
    let (on_mark, (on_ink, on_weight)) = anatomy(&on);
    let (off_mark, (off_ink, off_weight)) = anatomy(&off);
    assert_eq!(on_mark, Lexicon::ink_bytes(lex.mauve));
    assert_eq!(off_mark, Lexicon::ink_bytes(lex.ov0));
    assert_eq!((on_ink, on_weight), (Some(lex.tx), 550));
    assert_eq!((off_ink, off_weight), (Some(lex.sb1), 400));
}
