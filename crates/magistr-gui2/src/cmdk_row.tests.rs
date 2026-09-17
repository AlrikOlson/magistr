use super::*;
use crate::medium::Medium;
use crate::voice::Voice;

/// Chosen wears the accent wash, idle wears nothing; both answer on
/// their id.
#[test]
fn chosen_wears_the_wash_and_idle_wears_nothing() {
    let lex = Lexicon::of(Medium::Night);
    let content = || vec![Element::text(Voice::SLOT, lex.tx, "Open think-and-ship").nowrap()];
    let chosen = CmdkRow::el(content(), true, lex.base, ElementId(50), &lex);
    let idle = CmdkRow::el(content(), false, lex.base, ElementId(51), &lex);
    assert!(chosen.fill.is_some(), "the wash");
    assert_ne!(chosen.fill, Some(lex.base), "washed, not bare ground");
    assert!(idle.fill.is_none());
    assert_eq!(chosen.id, Some(ElementId(50)));
    assert!(chosen.focusable && idle.focusable);
}
