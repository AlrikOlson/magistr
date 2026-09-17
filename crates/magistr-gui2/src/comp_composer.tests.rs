use magistr_cockpit::cockpit::{Model, Msg, Size, update};
use magistr_cockpit::input::{KeyCode, KeyEvent};

use super::*;
use crate::element::Kind;

fn runs(root: &Element) -> Vec<(String, qs_gpu::color::Srgba)> {
    let mut runs = Vec::new();
    root.each(&mut |_, element| {
        if let Kind::Text { content, .. } = &element.kind {
            runs.push((
                content.clone(),
                element.fill.expect("a run carries its ink"),
            ));
        }
    });
    runs
}

/// Empty, the pill wears the resting prompt in the faint ink; typing
/// through the one shared `update` replaces it with the typed text in
/// the full ink — the mapping is live against the real loop.
#[test]
fn typing_through_update_fills_the_pill() {
    let lex = Lexicon::of(Medium::Night);
    let mut model = Model::new(Size::new(120, 40));
    let resting = ComposerComposition::el(&model, Medium::Night);
    let steer = runs(&resting)
        .into_iter()
        .find(|(text, _)| text == "Steer\u{2026}")
        .expect("the resting prompt");
    assert_eq!(steer.1, lex.ov0, "resting is the faint ink");
    for key in ['h', 'i'] {
        update(&mut model, Msg::Key(KeyEvent::new(KeyCode::Char(key))));
    }
    assert_eq!(model.prompt().text(), "hi");
    let filled = ComposerComposition::el(&model, Medium::Night);
    let typed = runs(&filled)
        .into_iter()
        .find(|(text, _)| text == "hi")
        .expect("the typed text is on the pill");
    assert_eq!(typed.1, lex.tx, "typed is the full ink");
    assert!(
        !runs(&filled).iter().any(|(t, _)| t == "Steer\u{2026}"),
        "the placeholder stands down"
    );
}

/// The pill keeps its identity and focusability whichever face shows —
/// one control, two readings of the model.
#[test]
fn the_pill_is_the_one_focusable_control_in_both_faces() {
    let mut model = Model::new(Size::new(120, 40));
    for face in [ComposerComposition::el(&model, Medium::Night), {
        update(&mut model, Msg::Key(KeyEvent::new(KeyCode::Char('x'))));
        ComposerComposition::el(&model, Medium::Night)
    }] {
        let mut ids = Vec::new();
        face.each(&mut |_, element| {
            if element.focusable {
                ids.extend(element.id);
            }
        });
        assert_eq!(ids, vec![ComposerComposition::PILL]);
    }
}
