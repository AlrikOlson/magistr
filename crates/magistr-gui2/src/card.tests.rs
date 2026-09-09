use magistr_cockpit::theme::Severity;

use super::*;
use crate::medium::Medium;

/// The four grounds are the volume's own, and no card face can carry
/// a stroke — the border prohibition held by type: nothing in this
/// module's output has one.
#[test]
fn the_grounds_are_the_volumes_and_no_card_wears_a_stroke() {
    for medium in Medium::ALL {
        let lex = Lexicon::of(medium);
        let glass = Lexicon::composite(
            lex.base,
            qs_gpu::color::Srgba::new(lex.mantle.r, lex.mantle.g, lex.mantle.b, 0.84),
        );
        for (card, ground) in [
            (Card::el(17.0, 14.0, &lex), lex.mantle),
            (
                Card::severity(Severity::Alert, 14.0, 11.0, &lex),
                SeverityInk::of(Severity::Alert, &lex).ground,
            ),
            (Card::overlay(lex.base, 26.0, 24.0, &lex), glass),
            (Card::popover(lex.base, 6.0, 4.0, &lex), glass),
        ] {
            assert_eq!(card.fill, Some(ground));
            assert!(card.stroke.is_none(), "a card never wears a border");
            assert_eq!(card.radius, 13.0, "the card radius");
        }
    }
}

/// The elevation scale is worn as filed: card at row, overlay and
/// popover at their own levels — depth, not a flat wash.
#[test]
fn the_scale_is_worn_as_filed() {
    let lex = Lexicon::of(Medium::Night);
    assert_eq!(Card::el(1.0, 1.0, &lex).elevation, Elevation::row());
    assert_eq!(
        Card::overlay(lex.base, 1.0, 1.0, &lex).elevation,
        Elevation::overlay()
    );
    assert_eq!(
        Card::popover(lex.base, 1.0, 1.0, &lex).elevation,
        Elevation::popover()
    );
    assert!(!Card::el(1.0, 1.0, &lex).elevation.is_flat());
}
