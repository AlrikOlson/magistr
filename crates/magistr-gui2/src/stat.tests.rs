use super::*;
use crate::medium::Medium;

/// The count sits in its fixed slot so the tail never breathes, and
/// the whole stat is read-only.
#[test]
fn the_count_sits_in_its_slot() {
    let lex = Lexicon::of(Medium::Night);
    let stat = Stat::el("211", "306", &lex);
    let mut slot_width = None;
    let mut runs = Vec::new();
    stat.each(&mut |_, element| {
        assert!(element.id.is_none() && !element.focusable);
        if let crate::element::Kind::Text { content, .. } = &element.kind {
            runs.push(content.clone());
        } else if element.width == Some(30.0) {
            slot_width = element.width;
        }
    });
    assert_eq!(slot_width, Some(30.0), "the volume's own 30px slot");
    assert_eq!(runs, ["211", " / 306"]);
}

/// Both runs are the stat voice in the second subdued ink — one
/// component, one voice, no naked halves.
#[test]
fn one_voice_one_ink() {
    let lex = Lexicon::of(Medium::Day);
    Stat::el("7", "12", &lex).each(&mut |_, element| {
        if let crate::element::Kind::Text { voice, .. } = &element.kind {
            assert_eq!(*voice, Voice::STAT);
            assert_eq!(element.fill, Some(lex.sb1));
        }
    });
}
