use super::*;
use crate::medium::Medium;

fn lex() -> Lexicon {
    Lexicon::of(Medium::Night)
}

/// The three answers are a fixed vocabulary: the word and the hue come
/// as one, never as free text.
#[test]
fn the_word_and_the_hue_are_one_vocabulary() {
    let lex = lex();
    let word_and_ink = |badge: &Element| {
        let mut found = None;
        badge.each(&mut |_, element| {
            if let crate::element::Kind::Text { content, .. } = &element.kind {
                found = Some((content.clone(), element.fill));
            }
        });
        found.expect("a badge carries its word")
    };
    let ground = lex.base;
    assert_eq!(
        word_and_ink(&ModeBadge::ask(ground, &lex)),
        ("ASK".to_owned(), Some(lex.yellow))
    );
    assert_eq!(
        word_and_ink(&ModeBadge::auto(ground, &lex)),
        ("AUTO".to_owned(), Some(lex.green))
    );
    assert_eq!(
        word_and_ink(&ModeBadge::off(ground, &lex)),
        ("OFF".to_owned(), Some(lex.red))
    );
}

/// A badge is read, never clicked, and never dashes.
#[test]
fn a_mode_badge_is_structurally_inert() {
    let lex = lex();
    for badge in [
        ModeBadge::ask(lex.base, &lex),
        ModeBadge::auto(lex.base, &lex),
        ModeBadge::off(lex.base, &lex),
    ] {
        badge.each(&mut |_, element| {
            assert!(element.id.is_none() && !element.focusable && !element.dashed);
        });
    }
}
