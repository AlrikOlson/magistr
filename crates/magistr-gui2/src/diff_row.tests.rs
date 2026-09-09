use super::*;
use crate::medium::Medium;

/// The three faces: added on the success wash in the text ink,
/// deleted on the danger wash in the subdued ink, context on no wash
/// in the faint ink — all read-only.
#[test]
fn the_three_faces_wear_the_volumes_washes() {
    let lex = Lexicon::of(Medium::Night);
    let add = DiffRow::add("+ new", lex.base, &lex);
    let del = DiffRow::del("- old", lex.base, &lex);
    let ctx = DiffRow::ctx("  same", &lex);
    assert!(add.fill.is_some() && del.fill.is_some());
    assert_ne!(add.fill, del.fill, "two different washes");
    assert!(ctx.fill.is_none(), "context has no wash");
    for row in [add, del, ctx] {
        row.each(&mut |_, element| {
            assert!(element.id.is_none() && !element.focusable && !element.dashed);
        });
    }
}

/// The code rides the diff voice on its own 1.75 leading.
#[test]
fn the_code_rides_the_diff_voice() {
    let lex = Lexicon::of(Medium::Day);
    DiffRow::ctx("receipt #241", &lex).each(&mut |_, element| {
        if let crate::element::Kind::Text { voice, .. } = &element.kind {
            assert_eq!(*voice, Voice::DIFFLINE);
            assert!((voice.line_height - 1.75).abs() < f32::EPSILON);
        }
    });
}
