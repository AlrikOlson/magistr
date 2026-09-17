use super::*;
use crate::element::Kind;
use crate::medium::Medium;

fn texts(root: &Element) -> Vec<(String, qs_gpu::color::Srgba)> {
    let mut out = Vec::new();
    root.each(&mut |_, element| {
        if let Kind::Text { content, .. } = &element.kind {
            out.push((content.clone(), element.fill.expect("a run has its ink")));
        }
    });
    out
}

/// Empty, the field shows its placeholder in the faint ink; typed, the
/// words in the text ink — the mark and the caret in the accent either
/// way, and the field wears its id.
#[test]
fn the_placeholder_gives_way_to_the_typed_words() {
    let lex = Lexicon::of(Medium::Night);
    let empty = AnswerField::el("", "Type your own answer", ElementId(9), &lex);
    let runs = texts(&empty);
    assert_eq!(runs[0].0, "\u{203a}");
    assert_eq!(runs[0].1, lex.mauve);
    assert_eq!(runs[1], ("Type your own answer".to_owned(), lex.ov0));
    let typed = AnswerField::el("the harbor", "Type your own answer", ElementId(9), &lex);
    assert_eq!(texts(&typed)[1], ("the harbor".to_owned(), lex.tx));
    let mut ids = Vec::new();
    typed.each(&mut |_, element| ids.extend(element.id));
    assert_eq!(ids, vec![ElementId(9)]);
}
