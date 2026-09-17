use super::*;
use crate::element::Kind;

fn texts(root: &Element) -> Vec<String> {
    let mut texts = Vec::new();
    root.each(&mut |_, element| {
        if let Kind::Text { content, .. } = &element.kind {
            texts.push(content.clone());
        }
    });
    texts
}

/// The ids the row mints, in reading order.
fn ids(root: &Element) -> Vec<ElementId> {
    let mut ids = Vec::new();
    root.each(&mut |_, element| {
        if let Some(id) = element.id {
            ids.push(id);
        }
    });
    ids
}

/// four pairs, cap then verb, with the registry's own keys on the caps — a rebound key
/// cannot leave the rail lying — and the abort cap is the window's chord, never the
/// terminal's `esc`.
#[test]
fn the_row_is_the_designs_four_pairs_with_the_registrys_keys() {
    let lex = Lexicon::of(Medium::Night);
    let row = LedgerVerbs::el(false, Medium::Night, lex.base, &lex);
    assert_eq!(
        texts(&row),
        [
            "space",
            "Pause",
            "s",
            "Step",
            "c",
            "Checkpoint",
            "\u{2318}.",
            "Abort"
        ]
    );
    assert_eq!(Binding::PAUSE.keys, "space");
    assert_eq!(Binding::STEP.keys, "s");
    assert_eq!(Binding::CHECKPOINT_NOW.keys, "c");
    assert_eq!(Binding::ABORT_CHORD.keys, "\u{2318}.");
    assert_ne!(Binding::ABORT_CHORD.keys, Binding::ABORT.keys);
    assert_eq!(
        ids(&row),
        [
            LedgerVerbs::PAUSE,
            LedgerVerbs::STEP,
            LedgerVerbs::CHECKPOINT,
            LedgerVerbs::ABORT
        ]
    );
}

/// Held at a boundary, the first pair reads Resume; nothing else moves.
#[test]
fn the_pause_pair_reads_resume_while_the_loop_holds() {
    let lex = Lexicon::of(Medium::Day);
    let row = LedgerVerbs::el(true, Medium::Day, lex.base, &lex);
    assert_eq!(
        texts(&row),
        [
            "space",
            "Resume",
            "s",
            "Step",
            "c",
            "Checkpoint",
            "\u{2318}.",
            "Abort"
        ]
    );
}

/// The labels are set a size down from the class — the rail's inline
/// 9.5 — while the caps keep the kbd's own voice.
#[test]
fn the_labels_wear_the_ledger_hint_voice() {
    let lex = Lexicon::of(Medium::Night);
    let row = LedgerVerbs::el(false, Medium::Night, lex.base, &lex);
    let mut voices = Vec::new();
    row.each(&mut |_, element| {
        if let Kind::Text { voice, .. } = &element.kind {
            voices.push(voice.px);
        }
    });
    assert_eq!(voices, [9.5; 8]);
    assert_eq!(Voice::LEDGER_HINT.family, Voice::HINT.family);
}
