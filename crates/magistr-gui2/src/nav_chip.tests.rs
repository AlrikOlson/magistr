use super::*;
use crate::medium::Medium;

/// A nav chip is the interactive counterpart of the data pill: it
/// wears an id, takes focus, and answers hover and press with the
/// raise and the dive.
#[test]
fn a_nav_chip_answers_the_pointer_and_holds_focus() {
    let lex = Lexicon::of(Medium::Night);
    let chip = NavChip::el("gui2-2a · execute", ElementId(7), &lex);
    assert_eq!(chip.id, Some(ElementId(7)));
    assert!(chip.focusable);
    assert!(chip.focus_ring.is_some());
    assert!(chip.hover_fill.is_some(), "the raise");
    assert!(chip.press_fill.is_some(), "the dive");
    assert!(!chip.dashed, "nothing clickable dashes");
}
