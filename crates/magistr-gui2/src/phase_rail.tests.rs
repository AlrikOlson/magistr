use super::*;
use crate::medium::Medium;

/// Done phases wear the check in the success hue, the live one the
/// accent at weight 600, the rest the faint ink — and the rail is
/// read-only.
#[test]
fn the_rail_reads_the_loop() {
    let lex = Lexicon::of(Medium::Night);
    let rail = PhaseRail::el(&["research", "plan", "execute", "verify"], 2, &lex);
    let mut runs = Vec::new();
    rail.each(&mut |_, element| {
        assert!(element.id.is_none() && !element.focusable);
        if let crate::element::Kind::Text { voice, content } = &element.kind {
            runs.push((content.clone(), element.fill, voice.weight));
        }
    });
    assert_eq!(
        runs,
        [
            ("research \u{2713}".to_owned(), Some(lex.green), 400),
            ("plan \u{2713}".to_owned(), Some(lex.green), 400),
            ("execute".to_owned(), Some(lex.mauve), 600),
            ("verify".to_owned(), Some(lex.ov0), 400),
        ]
    );
}
