use super::*;
use crate::element_layout::ElementLayout;
use crate::medium::Medium;
use crate::text::TextEngine;

/// A meter is a quantity you read — no id, no focus, no pointer face,
/// anywhere in its tree.
#[test]
fn a_meter_is_structurally_inert() {
    let lex = Lexicon::of(Medium::Night);
    let meter = BudgetMeter::el(
        "Budget",
        "1.84 / 12.00",
        0.15,
        Some(0.83),
        None,
        None,
        220.0,
        &lex,
    );
    meter.each(&mut |_, element| {
        assert!(element.id.is_none());
        assert!(!element.focusable);
        assert!(element.hover_fill.is_none());
    });
}

/// The fill takes its share of the track and the breaker tick stands
/// 2px proud of it both ways, at its own share.
#[test]
fn the_fill_and_the_tick_land_at_their_shares() {
    let lex = Lexicon::of(Medium::Night);
    let meter = BudgetMeter::el(
        "Budget",
        "1.84 / 12.00",
        0.15,
        Some(0.83),
        None,
        None,
        220.0,
        &lex,
    );
    let mut text = TextEngine::embedded();
    let layout = ElementLayout::compute(&meter, 220.0, 25.0, &mut text).expect("layout");
    // Find the track (height 4) and read its children.
    let mut track_at = None;
    meter.each(&mut |index, element| {
        if element.height == Some(4.0) {
            track_at = Some(index);
        }
    });
    let track = track_at.expect("the track exists");
    let [_, track_y, _, _] = layout.rect(track);
    let [fx, fy, fw, fh] = layout.rect(track + 1);
    assert_eq!((fx, fw), (0.0, 220.0 * 0.15), "the fill's share");
    assert_eq!((fy, fh), (track_y, 4.0), "the fill spans the track");
    let [tx, ty, tw, th] = layout.rect(track + 2);
    assert_eq!((tx, tw), (220.0 * 0.83, 2.0), "the tick's share");
    assert_eq!((ty, th), (track_y - 2.0, 8.0), "2px proud both ways");
}

/// The stop note appears only when there is one.
#[test]
fn the_note_is_optional() {
    let lex = Lexicon::of(Medium::Night);
    let with = BudgetMeter::el(
        "Budget",
        "1.84",
        0.1,
        None,
        Some("Stop at 10.00"),
        None,
        220.0,
        &lex,
    );
    let without = BudgetMeter::el("Budget", "1.84", 0.1, None, None, None, 220.0, &lex);
    assert_eq!(with.children.len(), without.children.len() + 2);
}

/// The label row sets its cap and value on one baseline the way
/// `.ml { align-items:baseline }` does: the cap's box carries the one
/// pixel its bottom-aligned 9.5px line would otherwise sit under the
/// 10.5px value's baseline.
#[test]
fn the_label_and_the_value_share_a_baseline() {
    let lex = Lexicon::of(Medium::Night);
    let meter = BudgetMeter::el(
        "Budget",
        "1.84 / 12.00",
        0.15,
        None,
        None,
        None,
        220.0,
        &lex,
    );
    let row = &meter.children[0];
    assert_eq!(row.children.len(), 2, "the cap and the value");
    let cap = &row.children[0];
    assert_eq!(cap.pad[3], BudgetMeter::LABEL_BASELINE_LIFT);
    assert_eq!(cap.pad[3], 1.0);
    assert!(
        matches!(cap.children[0].kind, crate::element::Kind::Text { .. }),
        "the cap's run sits inside the lifted box"
    );
}

/// The note is set in the cap's voice at the `.06em` the volume gives it inline, and it
/// sits on the window's own line under its 4px margin — 3px lower than the cap's line
/// box alone would put it, the way the capture draws `Stop at 10.00`.
#[test]
fn the_note_wears_its_own_tracking_on_the_windows_line() {
    let lex = Lexicon::of(Medium::Night);
    let meter = BudgetMeter::el(
        "Budget",
        "1.84 / 12.00",
        0.15,
        Some(0.83),
        Some("Stop at 10.00"),
        None,
        220.0,
        &lex,
    );
    let air = &meter.children[meter.children.len() - 2];
    assert_eq!(
        air.height,
        Some(BudgetMeter::NOTE_GAP + BudgetMeter::NOTE_BASELINE_DROP),
        "the volume's margin plus the strut's drop"
    );
    let foot = &meter.children[meter.children.len() - 1];
    let note = &foot.children[0];
    let crate::element::Kind::Text { voice, .. } = &note.kind else {
        panic!("the note is a text run");
    };
    assert_eq!(*voice, Voice::METER_NOTE);
    assert_eq!(Voice::METER_NOTE.tracking_em, 0.06);
    assert_eq!(
        Voice {
            tracking_em: Voice::CAP.tracking_em,
            ..Voice::METER_NOTE
        },
        Voice::CAP,
        "the cap's voice in every other respect"
    );
}

/// The foot line's `Raise` is the one control near the meter: it wears the id the
/// caller hands it, takes focus under a ring, sits at the foot's far edge past the
/// note, and nothing else in the tree answers a pointer.
#[test]
fn the_foot_lines_raise_is_the_one_control_and_sits_past_the_note() {
    let lex = Lexicon::of(Medium::Night);
    let meter = BudgetMeter::el(
        "Budget",
        "1.84 / 12.00",
        0.15,
        Some(0.83),
        Some("Stop at 10.00"),
        Some(BudgetMeter::RAISE),
        220.0,
        &lex,
    );
    let mut controls = Vec::new();
    meter.each(&mut |_, element| {
        if let Some(id) = element.id {
            controls.push((id, element.focusable, element.focus_ring));
        }
    });
    assert_eq!(controls, [(BudgetMeter::RAISE, true, Some(lex.mauve))]);
    let foot = &meter.children[meter.children.len() - 1];
    let words: Vec<&str> = foot
        .children
        .iter()
        .filter_map(|child| match &child.kind {
            crate::element::Kind::Text { content, .. } => Some(content.as_str()),
            _ => None,
        })
        .collect();
    assert_eq!(words, ["Stop at 10.00", "Raise"]);
    let raise = foot.children.last().expect("the verb");
    let crate::element::Kind::Text { voice, .. } = &raise.kind else {
        panic!("the verb is a text run");
    };
    assert_eq!(*voice, Voice::RAIL_VERB);
    assert_eq!(raise.fill, Some(lex.mauve), "the accent");
    // With no note the verb still has its line to stand on.
    let bare = BudgetMeter::el(
        "Budget",
        "1.84 / 12.00",
        0.15,
        None,
        None,
        Some(BudgetMeter::RAISE),
        220.0,
        &lex,
    );
    assert_eq!(bare.children.len(), meter.children.len());
}
