use super::*;

/// The field mid-search: chord in the accent, query in the text ink
/// with the caret, the count faint at the far edge — focusable on its
/// id, at the panel radius.
#[test]
fn the_field_carries_chord_query_and_count() {
    let lex = Lexicon::of(crate::medium::Medium::Night);
    let field = SearchField::el(
        "\u{2318}\u{21e7}F",
        "contrast floor",
        "6 results",
        ElementId(41),
        &lex,
    );
    assert_eq!(field.id, Some(ElementId(41)));
    assert!(field.focusable);
    assert_eq!(field.radius, 11.0);
    let mut runs = Vec::new();
    field.each(&mut |_, element| {
        if let crate::element::Kind::Text { content, .. } = &element.kind {
            runs.push((content.clone(), element.fill));
        }
    });
    assert_eq!(
        runs,
        [
            ("\u{2318}\u{21e7}F".to_owned(), Some(lex.mauve)),
            ("contrast floor".to_owned(), Some(lex.tx)),
            ("6 results".to_owned(), Some(lex.ov0)),
        ]
    );
}
