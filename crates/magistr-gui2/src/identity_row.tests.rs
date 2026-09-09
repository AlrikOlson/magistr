use super::*;
use crate::element::Kind;
use crate::medium::Medium;

fn parts(row: &Element) -> (usize, Vec<String>) {
    let mut marks = 0;
    let mut texts = Vec::new();
    row.each(&mut |_, element| match &element.kind {
        Kind::Mark { .. } => marks += 1,
        Kind::Text { content, .. } => texts.push(content.clone()),
        Kind::Surface | Kind::Picture { .. } | Kind::Grid { .. } => {}
    });
    (marks, texts)
}

/// Sigil · name · path, in that order, every run the row's own.
#[test]
fn the_row_is_sigil_name_path_and_owns_its_runs() {
    let lex = Lexicon::of(Medium::Night);
    let row = IdentityRow::el(
        Some(Mark::Sigil("magistr")),
        "magistr",
        Some("~/Code/magistr"),
        &lex,
    );
    assert_eq!(
        parts(&row),
        (1, vec!["magistr".to_owned(), "~/Code/magistr".to_owned()])
    );
    for run in row.runs() {
        assert_eq!(run.owner, Some(Component::IdentityRow), "{}", run.text);
    }
}

/// A project with no sigil and no known path is still named — the
/// row never infers either.
#[test]
fn a_bare_name_still_has_a_row() {
    let lex = Lexicon::of(Medium::Day);
    let row = IdentityRow::el(None, "tidewatch", Some(""), &lex);
    assert_eq!(parts(&row), (0, vec!["tidewatch".to_owned()]));
}
