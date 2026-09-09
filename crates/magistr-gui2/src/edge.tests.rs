use super::*;
use crate::medium::Medium;

/// Done is one solid line; ahead is the same length broken 4/4 into
/// segments that never overshoot. Both read-only.
#[test]
fn done_is_solid_and_ahead_is_broken() {
    let lex = Lexicon::of(Medium::Night);
    let done = Edge::done(120.0, &lex);
    assert_eq!(done.width, Some(120.0));
    assert_eq!(done.fill, Some(lex.edge));
    assert_eq!(done.height, Some(1.4));

    let ahead = Edge::ahead(120.0, &lex);
    assert!(ahead.fill.is_none(), "the ahead line is its dashes");
    let mut segments = 0;
    let mut laid = 0.0;
    ahead.each(&mut |_, element| {
        assert!(element.id.is_none() && !element.focusable);
        if element.fill == Some(lex.edge) {
            segments += 1;
            laid += element.width.unwrap_or(0.0);
        }
    });
    assert_eq!(segments, 15, "120px of 4/4 rhythm");
    assert!(laid <= 120.0, "dashes never overshoot the length");
}
