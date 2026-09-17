use magistr_cockpit::cockpit::Spine;
use magistr_domain::workflow::CheckpointReason;

use super::*;
use crate::element::Kind;
use crate::medium::Medium;

/// View 05's spine: 239 receipts before this window, four states with
/// the newest a manual one, two receipts since.
fn spine() -> Spine {
    let mut spine = Spine::default();
    spine.seed_receipts(239);
    let green = || {
        ["check", "test", "lint"]
            .map(|gate| (gate.to_owned(), true))
            .to_vec()
    };
    let rows = [
        spine.open_boundary(1, Vec::new()),
        spine.open_boundary(2, green()),
        spine.open_boundary(3, green()),
        spine.open(CheckpointReason::Manual, "before golden update"),
    ];
    for id in rows {
        spine.saved(&id, Some("abc123".to_owned()), true);
    }
    spine.note_receipt();
    spine.note_receipt();
    spine
}

fn texts(root: &Element) -> Vec<String> {
    let mut texts = Vec::new();
    root.each(&mut |_, element| {
        if let Kind::Text { content, .. } = &element.kind {
            texts.push(content.clone());
        }
    });
    texts
}

/// Every node's fill and pins, in tree order.
fn nodes(root: &Element) -> Vec<(Option<Srgba>, [Option<f32>; 4])> {
    let mut nodes = Vec::new();
    root.each(&mut |_, element| {
        if element.width == Some(SpineLedger::NODE) && element.height == Some(SpineLedger::NODE) {
            nodes.push((element.fill, element.pin));
        }
    });
    nodes
}

/// The head entry is the live state: `head` with `now` beside it and
/// the receipts taken past the newest checkpoint under it — the
/// spine's one counter, never a second number.
#[test]
fn the_head_entry_reads_head_now_and_the_receipts_past_the_newest_state() {
    let lex = Lexicon::of(Medium::Night);
    let texts = texts(&SpineLedger::el(&spine(), 2, None, None, &lex));
    assert_eq!(&texts[..3], ["head", "now", "2 receipts after cp-4"]);
}

/// The head entry seats the loop's verbs — the row the caller drew —
/// under its receipts line and before the first checkpoint row, the
/// way view 05's rail draws them.
#[test]
fn the_head_entry_seats_the_verbs_under_its_receipts_line() {
    let lex = Lexicon::of(Medium::Night);
    let verbs = Element::text(Voice::LEDGER_HINT, lex.sb0, "verbs").claim(Component::KeyHint);
    let texts = texts(&SpineLedger::el(&spine(), 2, Some(verbs), None, &lex));
    assert_eq!(
        &texts[..5],
        [
            "head",
            "now",
            "2 receipts after cp-4",
            "verbs",
            "cp-4 · before golden update"
        ]
    );
}

/// The rows under the head are the newest checkpoints, newest first — the capture's
/// reading order — each with `Preview` at its far edge, and a boundary row wears its
/// gates in the success hue after its label.
#[test]
fn the_rows_are_newest_first_and_a_boundary_wears_its_marks_in_green() {
    let lex = Lexicon::of(Medium::Night);
    let block = SpineLedger::el(&spine(), 2, None, None, &lex);
    let texts = texts(&block);
    assert_eq!(
        &texts[3..],
        [
            "cp-4 · before golden update",
            "Preview",
            "cp-3 · iter 3 ·",
            "✓✓✓",
            "Preview"
        ]
    );
    let mut marks = None;
    block.each(&mut |_, element| {
        if let Kind::Text { content, .. } = &element.kind
            && content == "✓✓✓"
        {
            marks = element.fill;
        }
    });
    assert_eq!(marks, Some(lex.green), "the marks are the success hue");
}

/// One receipt is one receipt.
#[test]
fn one_receipt_is_singular() {
    let lex = Lexicon::of(Medium::Night);
    let mut spine = Spine::default();
    let id = spine.open(CheckpointReason::Manual, "start");
    spine.saved(&id, None, true);
    spine.note_receipt();
    let texts = texts(&SpineLedger::el(&spine, 2, None, None, &lex));
    assert!(
        texts.contains(&"1 receipt after cp-1".to_owned()),
        "{texts:?}"
    );
}

/// Every entry has its node astride the line — the head's in the
/// accent, the rest in the second surface step — pinned off the entry
/// the way the volume positions them.
#[test]
fn every_entry_has_a_node_and_the_heads_is_the_accent() {
    let lex = Lexicon::of(Medium::Night);
    let nodes = nodes(&SpineLedger::el(&spine(), 2, None, None, &lex));
    assert_eq!(nodes.len(), 3, "head + two rows");
    assert_eq!(nodes[0].0, Some(lex.mauve));
    assert_eq!(nodes[1].0, Some(lex.s1));
    assert_eq!(nodes[2].0, Some(lex.s1));
    for (_, pin) in &nodes {
        assert_eq!(pin[0], Some(SpineLedger::NODE_LEFT));
        assert_eq!(pin[1], Some(SpineLedger::NODE_TOP));
    }
}

/// An empty spine is no block at all: the caller says "no checkpoints
/// yet" in words, and this draws no line to nowhere.
#[test]
fn an_empty_spine_is_no_block() {
    let lex = Lexicon::of(Medium::Night);
    let block = SpineLedger::el(&Spine::default(), 2, None, None, &lex);
    assert!(texts(&block).is_empty());
    assert!(nodes(&block).is_empty());
}

/// Every checkpoint row is the spine row's own control: it wears its id, takes focus
/// under the accent ring, raises onto the glow ground under the pointer, and carries
/// `Preview` in the rail's verb voice. The revealed row — by spine index — carries `·
/// Restore · Fork` too, each verb its own focusable id, and stands on the raised
/// ground; no other row does.
#[test]
fn a_row_is_a_control_and_the_revealed_row_carries_restore_and_fork() {
    let lex = Lexicon::of(Medium::Night);
    // cp-3 is spine index 2; cp-4, the newest, index 3.
    let block = SpineLedger::el(&spine(), 2, None, Some(2), &lex);
    let mut controls = Vec::new();
    block.each(&mut |_, element| {
        if let Some(id) = element.id {
            controls.push((
                id,
                element.focusable,
                element.focus_ring,
                element.hover_fill.is_some(),
                element.fill,
            ));
        }
    });
    let raise = SpineLedger::raise(&lex);
    assert_eq!(
        controls,
        [
            (
                ElementId(SpineLedger::ROWS + 3),
                true,
                Some(lex.mauve),
                true,
                None
            ),
            (
                ElementId(SpineLedger::ROWS + 2),
                true,
                Some(lex.mauve),
                true,
                Some(raise)
            ),
            // A run's fill is its ink: the verbs in the accent.
            (
                ElementId(SpineLedger::RESTORE + 2),
                true,
                Some(lex.mauve),
                false,
                Some(lex.mauve)
            ),
            (
                ElementId(SpineLedger::FORK + 2),
                true,
                Some(lex.mauve),
                false,
                Some(lex.mauve)
            ),
        ],
        "the rows, then the revealed row's two verbs"
    );
    let texts = texts(&block);
    assert_eq!(
        &texts[3..],
        [
            "cp-4 · before golden update",
            "Preview",
            "cp-3 · iter 3 ·",
            "✓✓✓",
            "Preview",
            "·",
            "Restore",
            "·",
            "Fork"
        ]
    );
    let mut verbs = Vec::new();
    block.each(&mut |_, element| {
        if let Kind::Text { content, voice, .. } = &element.kind
            && *voice == Voice::RAIL_VERB
        {
            verbs.push((content.clone(), element.fill));
        }
    });
    assert!(
        verbs.iter().all(|(_, ink)| *ink == Some(lex.mauve)),
        "every verb in the accent: {verbs:?}"
    );
    assert_eq!(
        Voice::RAIL_VERB.px,
        9.5,
        "the rail sets its verbs inline at 9.5"
    );
}
