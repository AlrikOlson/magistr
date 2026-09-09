use super::*;
use crate::medium::Medium;

fn lex() -> Lexicon {
    Lexicon::of(Medium::Night)
}

/// law, carried by the type and pinned here: a data pill is read, never clicked — no
/// element of any face carries an id, takes focus, or draws dashed.
#[test]
fn a_data_pill_is_structurally_inert() {
    let lex = lex();
    for pill in [
        DataPill::live("Executing · 04:12", &lex),
        DataPill::ok("gates ✓✓✓", &lex),
        DataPill::num("1.84 / 12.00", &lex),
        DataPill::tag("task:design-canvas", &lex),
    ] {
        pill.each(&mut |_, element| {
            assert!(element.id.is_none(), "a data pill never wears an id");
            assert!(!element.focusable, "and never takes focus");
            assert!(!element.dashed, "and never dashes — that is the slot's");
            assert!(
                element.hover_fill.is_none() && element.press_fill.is_none(),
                "and never answers the pointer with a face"
            );
        });
    }
}

/// The live face carries the status dot; the others carry none.
#[test]
fn only_the_live_face_wears_the_dot() {
    let lex = lex();
    let dots = |pill: &Element| {
        let mut n = 0;
        pill.each(&mut |_, element| {
            if element.radius == 3.0 {
                n += 1;
            }
        });
        n
    };
    assert_eq!(dots(&DataPill::live("Executing", &lex)), 1);
    assert_eq!(dots(&DataPill::ok("gates ✓", &lex)), 0);
    assert_eq!(dots(&DataPill::num("1.84", &lex)), 0);
}

#[test]
fn the_alarm_chip_wears_the_hue_on_ink_ring_and_dot() {
    let lex = lex();
    let chip = DataPill::alarm("2 blocking", lex.red, &lex);
    // The count's ink and the dot's fill are the hue, and nothing else
    // on the chip is filled at all.
    let mut filled = Vec::new();
    chip.each(&mut |_, el| {
        if let Some(fill) = el.fill {
            filled.push((matches!(el.kind, crate::element::Kind::Text { .. }), fill));
        }
    });
    assert_eq!(
        filled,
        vec![(false, lex.red), (true, lex.red)],
        "the dot, then the count"
    );
}
