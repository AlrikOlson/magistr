use super::*;
use crate::element::Kind;
use crate::medium::Medium;

fn run(root: &Element) -> (String, Srgba) {
    let mut out = None;
    root.each(&mut |_, element| {
        if let Kind::Text { content, .. } = &element.kind {
            out = Some((content.clone(), element.fill.expect("a run has its ink")));
        }
    });
    out.expect("a chip has one run")
}

/// A picked chip wears the ok face and its check; an unpicked one is
/// flat in the subdued ink; both carry the id.
#[test]
fn picked_is_green_with_a_check_and_unpicked_is_flat() {
    let lex = Lexicon::of(Medium::Day);
    let picked = PickChip::el("the public API", true, lex.base, ElementId(3), &lex);
    assert_eq!(
        run(&picked),
        ("the public API \u{2713}".to_owned(), lex.green)
    );
    let flat = PickChip::el("offline operation", false, lex.base, ElementId(4), &lex);
    assert_eq!(run(&flat), ("offline operation".to_owned(), lex.sb0));
    assert_eq!(picked.id, Some(ElementId(3)));
    assert_eq!(flat.id, Some(ElementId(4)));
    assert!(flat.focusable);
}
