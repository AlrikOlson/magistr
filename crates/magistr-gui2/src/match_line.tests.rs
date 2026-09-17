use super::*;
use crate::element::Kind;
use crate::medium::Medium;

fn runs(root: &Element, lex: &Lexicon) -> Vec<(String, u16, bool)> {
    let mut runs = Vec::new();
    root.each(&mut |_, element| {
        if let Kind::Text { content, voice } = &element.kind {
            runs.push((
                content.clone(),
                voice.weight,
                element.fill == Some(lex.mauve),
            ));
        }
    });
    runs
}

#[test]
fn a_marked_run_is_bold_in_the_accent_and_words_keep_their_trailing_space() {
    let lex = Lexicon::of(Medium::Night);
    let line = MatchLine::el(
        "clears the contrast floor here",
        &[(11, 25)],
        Voice::LEDGER_META,
        lex.ov0,
        &lex,
    );
    assert!(line.wrap, "the line wraps like an inline box");
    assert_eq!(
        runs(&line, &lex),
        vec![
            ("clears ".to_owned(), 400, false),
            ("the ".to_owned(), 400, false),
            ("contrast ".to_owned(), 700, true),
            ("floor".to_owned(), 700, true),
            (" ".to_owned(), 400, false),
            ("here".to_owned(), 400, false),
        ]
    );
}

#[test]
fn a_mark_inside_a_word_splits_the_word_into_runs_that_never_break_apart() {
    let lex = Lexicon::of(Medium::Day);
    let line = MatchLine::el(
        "ensure_contrast now",
        &[(7, 15)],
        Voice::CHIP,
        lex.sb0,
        &lex,
    );
    assert_eq!(
        runs(&line, &lex),
        vec![
            ("ensure_".to_owned(), 400, false),
            ("contrast".to_owned(), 700, true),
            (" ".to_owned(), 400, false),
            ("now".to_owned(), 400, false),
        ]
    );
    // The word's runs share one unbreakable row: two children of the
    // line, not four.
    assert_eq!(line.children.len(), 2);
    assert_eq!(line.children[0].children.len(), 3);
}

#[test]
fn no_marks_is_a_plain_line_and_out_of_range_marks_are_clipped() {
    let lex = Lexicon::of(Medium::Night);
    let plain = MatchLine::el("a b", &[], Voice::CHIP, lex.sb0, &lex);
    assert_eq!(runs(&plain, &lex).len(), 2);
    let clipped = MatchLine::el("a b", &[(2, 40)], Voice::CHIP, lex.sb0, &lex);
    assert_eq!(
        runs(&clipped, &lex),
        vec![("a ".to_owned(), 400, false), ("b".to_owned(), 700, true)]
    );
}
