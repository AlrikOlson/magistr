use super::*;
use crate::medium::Medium;

fn lex() -> Lexicon {
    Lexicon::of(Medium::Night)
}

/// The row's anatomy: a dot, the name, the state chip, the meta — and
/// only the divided face carries the hairline.
#[test]
fn the_row_carries_dot_name_chip_and_meta() {
    let lex = lex();
    let row = StatusRow::el(
        lex.mauve,
        "gui2-2a",
        ("Executing", None),
        "04:12",
        true,
        lex.base,
        &lex,
    );
    let mut runs = Vec::new();
    let mut dots = 0;
    let mut hairlines = 0;
    row.each(&mut |_, element| {
        assert!(element.id.is_none() && !element.focusable && !element.dashed);
        if let crate::element::Kind::Text { content, .. } = &element.kind {
            runs.push(content.clone());
        }
        if element.radius == 3.0 && element.fill == Some(lex.mauve) {
            dots += 1;
        }
        if element.height == Some(1.0) {
            hairlines += 1;
        }
    });
    assert_eq!(runs, ["gui2-2a", "Executing", "04:12"]);
    assert_eq!(dots, 1);
    assert_eq!(hairlines, 1);

    let last = StatusRow::el(
        lex.red,
        "e6b",
        ("Blocked", Some(lex.red)),
        "Requires agent",
        false,
        lex.base,
        &lex,
    );
    let mut hairlines = 0;
    last.each(&mut |_, element| {
        if element.height == Some(1.0) {
            hairlines += 1;
        }
    });
    assert_eq!(hairlines, 0, "the last row wears no rule");
}

/// A signalled state tints its chip's ink and ring; a quiet one wears
/// the subdued ink on the hairline ring.
#[test]
fn a_signalled_state_tints_the_chip() {
    let lex = lex();
    let chip_ink = |row: &Element| {
        let mut ink = None;
        row.each(&mut |_, element| {
            if let crate::element::Kind::Text { content, .. } = &element.kind
                && (content == "Blocked" || content == "Executing")
            {
                ink = element.fill;
            }
        });
        ink
    };
    let blocked = StatusRow::el(
        lex.red,
        "e6b",
        ("Blocked", Some(lex.red)),
        "meta",
        false,
        lex.base,
        &lex,
    );
    let executing = StatusRow::el(
        lex.mauve,
        "gui2-2a",
        ("Executing", None),
        "meta",
        false,
        lex.base,
        &lex,
    );
    assert_eq!(chip_ink(&blocked), Some(lex.red));
    assert_eq!(chip_ink(&executing), Some(lex.sb0));
}
