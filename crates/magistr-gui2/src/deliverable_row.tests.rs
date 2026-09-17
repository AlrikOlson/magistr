use super::*;
use crate::element::Kind;
use crate::medium::Medium;

fn texts(root: &Element) -> Vec<String> {
    let mut out = Vec::new();
    root.each(&mut |_, element| {
        if let Kind::Text { content, .. } = &element.kind {
            out.push(content.clone());
        }
    });
    out
}

/// The row's anatomy: a card at a row's lift, the badge with its
/// glyph in the hue, the name and the measure — and no click.
#[test]
fn the_row_is_a_lifted_card_with_a_badge_a_name_and_a_measure() {
    let lex = Lexicon::of(Medium::Night);
    let row = DeliverableRow::el(
        Icon::Recording,
        lex.mauve,
        true,
        "Session recording",
        "12 min",
        &lex,
    );
    assert_eq!(row.fill, Some(lex.mantle));
    assert_eq!(row.radius, Card::RADIUS);
    assert_eq!(texts(&row), vec!["Session recording", "\u{b7} 12 min"]);
    let mut badge = None;
    let mut glyph = None;
    row.each(&mut |_, element| {
        assert!(element.id.is_none() && !element.focusable, "you read it");
        if element.width == Some(26.0) && element.height == Some(26.0) {
            badge = Some(element.radius);
        }
        if let Kind::Mark { mark, px, ink } = &element.kind {
            glyph = Some((*mark, *px, *ink));
        }
    });
    assert_eq!(badge, Some(13.0), "the recording's badge is a disc");
    assert_eq!(
        glyph,
        Some((
            Mark::Icon(Icon::Recording),
            13,
            Lexicon::ink_bytes(lex.mauve)
        ))
    );
}

/// The filmstrips' badge is the rounded square in the teal.
#[test]
fn the_filmstrips_badge_is_a_rounded_square() {
    let lex = Lexicon::of(Medium::Day);
    let row = DeliverableRow::el(
        Icon::Filmstrip,
        lex.teal,
        false,
        "Motion filmstrips",
        "3 strips",
        &lex,
    );
    let mut badge = None;
    row.each(&mut |_, element| {
        if element.width == Some(26.0) && element.height == Some(26.0) {
            badge = Some(element.radius);
        }
    });
    assert_eq!(badge, Some(6.0));
    assert_eq!(texts(&row), vec!["Motion filmstrips", "\u{b7} 3 strips"]);
}
