use super::*;
use crate::element::Kind;
use crate::medium::Medium;

/// The key is the teal glyph and word; the fact follows in the text
/// ink; the row is a card at the given measure.
#[test]
fn the_key_is_teal_and_the_fact_follows() {
    let lex = Lexicon::of(Medium::Night);
    let row = FindingRow::el(
        Icon::Stack,
        "stack",
        "Rust \u{b7} edition 2024",
        330.0,
        &lex,
    );
    assert_eq!(row.width, Some(330.0));
    let mut runs = Vec::new();
    let mut marks = 0;
    row.each(&mut |_, element| match &element.kind {
        Kind::Text { content, .. } => {
            runs.push((content.clone(), element.fill.expect("a run has its ink")));
        }
        Kind::Mark { mark, .. } => {
            assert_eq!(*mark, Mark::Icon(Icon::Stack));
            marks += 1;
        }
        _ => {}
    });
    assert_eq!(marks, 1);
    assert_eq!(runs[0], ("stack".to_owned(), lex.teal));
    assert_eq!(runs[1], ("Rust \u{b7} edition 2024".to_owned(), lex.tx));
}
