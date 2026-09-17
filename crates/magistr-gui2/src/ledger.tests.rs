use super::*;
use crate::medium::Medium;

/// Every line sits on the 21px double-leading pitch, values carry
/// their own inks, and the block is read-only.
#[test]
fn the_ledger_keeps_its_pitch_and_inks() {
    let lex = Lexicon::of(Medium::Night);
    let block = Ledger::el(
        "receipts",
        &[
            ("commit", "a3f9c21", lex.sb1),
            ("gates", "\u{2713}\u{2713}\u{2713}", lex.green),
            ("cost", "2.07 USD", lex.sb0),
        ],
        &lex,
    );
    let mut rows = 0;
    let mut value_inks = Vec::new();
    block.each(&mut |_, element| {
        assert!(element.id.is_none() && !element.focusable);
        if element.height == Some(21.0) {
            rows += 1;
        }
        if let crate::element::Kind::Text { content, .. } = &element.kind
            && (content == "a3f9c21" || content.starts_with('\u{2713}') || content == "2.07 USD")
        {
            value_inks.push(element.fill);
        }
    });
    assert_eq!(rows, 4, "the cap line and three receipt lines");
    assert_eq!(
        value_inks,
        [Some(lex.sb1), Some(lex.green), Some(lex.sb0)],
        "each value in its own ink"
    );
}
