use super::*;
use crate::medium::Medium;
use crate::mode_badge::ModeBadge;

fn lex() -> Lexicon {
    Lexicon::of(Medium::Night)
}

/// The key column names a kind: the icon mark and the tracked word,
/// at the volume's fixed 58px, in the faint ink.
#[test]
fn the_key_column_names_a_kind() {
    let lex = lex();
    let row = ReceiptRow::value(
        (Icon::Commit, "commit"),
        "a3f9c21",
        Some("View"),
        true,
        lex.base,
        &lex,
    );
    let mut key_width = None;
    let mut marks = 0;
    let mut runs = Vec::new();
    row.each(&mut |_, element| {
        assert!(element.id.is_none() && !element.focusable);
        // The key column is a floor of 58, not a fixed width: a long
        // key grows it the way the browser's `min-width:auto` does.
        if element.min_width == Some(58.0) {
            key_width = element.min_width;
        }
        if matches!(element.kind, crate::element::Kind::Mark { .. }) {
            marks += 1;
        }
        if let crate::element::Kind::Text { content, .. } = &element.kind {
            runs.push((content.clone(), element.fill));
        }
    });
    assert_eq!(key_width, Some(58.0));
    assert_eq!(marks, 1);
    assert_eq!(
        runs,
        [
            ("commit".to_owned(), Some(lex.ov0)),
            ("a3f9c21".to_owned(), Some(lex.sb1)),
            ("View".to_owned(), Some(lex.mauve)),
        ]
    );
}

/// The gates receipt carries its lights in order, lit and dark.
#[test]
fn the_gates_receipt_carries_its_lights() {
    let lex = lex();
    let row = ReceiptRow::leds(
        (Icon::Gate, "gates"),
        &[true, true, false],
        Some("Log"),
        false,
        lex.base,
        &lex,
    );
    let mut lights = Vec::new();
    row.each(&mut |_, element| {
        if element.radius == 2.0 && element.width == Some(9.0) {
            lights.push(element.fill);
        }
    });
    assert_eq!(lights, [Some(lex.green), Some(lex.green), Some(lex.s1)]);
}

/// A rule row has no key column and ends in its badge.
#[test]
fn a_rule_row_ends_in_its_badge() {
    let lex = lex();
    let row = ReceiptRow::rule(
        "git push",
        ModeBadge::off(lex.base, &lex),
        true,
        lex.base,
        &lex,
    );
    let mut runs = Vec::new();
    let mut key_width = None;
    row.each(&mut |_, element| {
        // The key column is a floor of 58, not a fixed width: a long
        // key grows it the way the browser's `min-width:auto` does.
        if element.min_width == Some(58.0) {
            key_width = element.min_width;
        }
        if let crate::element::Kind::Text { content, .. } = &element.kind {
            runs.push(content.clone());
        }
    });
    assert_eq!(key_width, None, "no key column on a rule");
    assert_eq!(runs, ["git push", "OFF"]);
}
