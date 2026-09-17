use super::*;

/// The composer is the field a person writes into: it takes focus on
/// its id, wears the stadium hairline, and stands its caret in the
/// accent.
#[test]
fn the_composer_takes_focus_and_stands_its_caret() {
    let lex = Lexicon::of(crate::medium::Medium::Night);
    let field = Composer::el(
        "\u{203a}",
        "allow, but stop if\u{2026}",
        ElementId(40),
        &lex,
    );
    assert_eq!(field.id, Some(ElementId(40)));
    assert!(field.focusable);
    assert_eq!(field.radius, 999.0, "the stadium");
    let mut carets = 0;
    let mut runs = Vec::new();
    field.each(&mut |_, element| {
        if element.width == Some(1.5) && element.fill == Some(lex.mauve) {
            carets += 1;
        }
        if let crate::element::Kind::Text { content, .. } = &element.kind {
            runs.push((content.clone(), element.fill));
        }
    });
    assert_eq!(carets, 1, "one standing caret");
    assert_eq!(
        runs,
        [
            ("\u{203a}".to_owned(), Some(lex.mauve)),
            ("allow, but stop if\u{2026}".to_owned(), Some(lex.ov0)),
        ]
    );
}
