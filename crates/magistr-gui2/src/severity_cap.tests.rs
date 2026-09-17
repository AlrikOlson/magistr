use magistr_cockpit::theme::Severity;

use super::*;
use crate::medium::Medium;

/// The cap is law in miniature: the icon marks a kind and rides the same hue as the
/// word — one mark, one run, read-only.
#[test]
fn the_cap_is_a_kind_mark_and_its_word_in_one_hue() {
    let lex = Lexicon::of(Medium::Night);
    let cap = SeverityCap::el(Severity::Attention, Some(Icon::Report), "Review", &lex);
    let hue = SeverityInk::of(Severity::Attention, &lex).cap;
    let mut marks = 0;
    let mut inks = Vec::new();
    cap.each(&mut |_, element| {
        assert!(element.id.is_none() && !element.focusable);
        match &element.kind {
            crate::element::Kind::Mark { ink, .. } => {
                marks += 1;
                assert_eq!(*ink, Lexicon::ink_bytes(hue), "the mark wears the hue");
            }
            crate::element::Kind::Text { content, .. } => {
                inks.push((content.clone(), element.fill));
            }
            crate::element::Kind::Surface
            | crate::element::Kind::Picture { .. }
            | crate::element::Kind::Grid { .. } => {}
        }
    });
    assert_eq!(marks, 1, "one kind mark");
    assert_eq!(inks, [(" Review".to_owned(), Some(hue))]);
}

/// Every band caps in its own hue, and Quiet caps in the faint ink.
#[test]
fn every_band_caps_in_its_own_hue() {
    for medium in Medium::ALL {
        let lex = Lexicon::of(medium);
        for severity in Severity::ALL {
            let expected = SeverityInk::of(severity, &lex).cap;
            let cap = SeverityCap::el(severity, Some(Icon::Alert), "Stuck", &lex);
            let mut seen = None;
            cap.each(&mut |_, element| {
                if matches!(element.kind, crate::element::Kind::Text { .. }) {
                    seen = element.fill;
                }
            });
            assert_eq!(seen, Some(expected), "{severity:?} in {medium}");
        }
    }
}
