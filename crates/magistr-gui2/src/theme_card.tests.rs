use super::*;
use crate::element::Kind;
use crate::medium::Medium;

/// Chosen is said by ring-and-check: the 2px accent ring and the badge
/// with its mark appear together, and only then.
#[test]
fn chosen_is_ring_and_check() {
    let lex = Lexicon::of(Medium::Night);
    let chosen = ThemeCard::el(
        "The sky",
        true,
        ThemeCard::sky_preview(&lex),
        ElementId(40),
        &lex,
    );
    let plain = ThemeCard::el(
        "The sky",
        false,
        ThemeCard::sky_preview(&lex),
        ElementId(41),
        &lex,
    );
    let marks = |card: &Element| {
        let mut n = 0;
        card.each(&mut |_, element| {
            if matches!(element.kind, Kind::Mark { .. }) {
                n += 1;
            }
        });
        n
    };
    assert_eq!(marks(&chosen), 1, "the check");
    assert_eq!(marks(&plain), 0);
    // The ring is a child over the media (an inset box-shadow paints
    // over the background), never the preview's own stroke.
    let ring = |card: &Element| {
        let preview = &card.children[0];
        assert!(preview.stroke.is_none(), "the media carries no stroke");
        preview.children.iter().find_map(|child| child.stroke)
    };
    assert_eq!(ring(&chosen), Some((lex.mauve, 2.0)), "the accent ring");
    assert_eq!(
        ring(&plain),
        Some((lex.hairc, 1.0)),
        "the hairline rest ring"
    );
}

/// A visual choice is interactive: the card answers on its id.
#[test]
fn the_card_answers_on_its_id() {
    let lex = Lexicon::of(Medium::Night);
    let card = ThemeCard::el(
        "The chart",
        false,
        ThemeCard::sky_preview(&lex),
        ElementId(50),
        &lex,
    );
    assert_eq!(card.id, Some(ElementId(50)));
    assert!(card.focusable);
}
