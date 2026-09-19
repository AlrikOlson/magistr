use magistr_cockpit::cockpit::{LoopEvent, Model, Msg, Size, Widget, update};
use magistr_cockpit::input::{Button, MouseEvent, MouseKind};

use super::*;
use crate::element::Kind;
use crate::fixture::Fixture;
use crate::ledger_verbs::LedgerVerbs;

const CELL_W: f32 = 9.0;

fn texts(root: &Element) -> Vec<String> {
    let mut texts = Vec::new();
    root.each(&mut |_, element| {
        if let Kind::Text { content, .. } = &element.kind {
            texts.push(content.clone());
        }
    });
    texts
}

fn has(root: &Element, needle: &str) -> bool {
    texts(root).iter().any(|t| t.contains(needle))
}

/// View 05's rail, read entirely off the model: the newest checkpoints,
/// every charter rule wearing its mode's word, and the bound budget's
/// readout with its breaker.
#[test]
fn the_rail_maps_the_models_spine_charter_and_budget() {
    let model = Fixture::mock("05", Size::new(120, 40)).expect("view 05 has a fixture");
    let rail = RailComposition::el(&model, Medium::Night, CELL_W);
    assert!(has(&rail, "Checkpoints"));
    assert!(has(&rail, "before golden update"), "the newest state");
    assert!(
        has(&rail, "cp-3 · iter 3 ·") && has(&rail, "✓✓✓"),
        "a boundary row wears its iteration's gates, in the icon set's glyphs"
    );
    assert!(!has(&rail, "iter 1"), "only the newest two rows ride");
    for subject in ["Edit · magistr-tui", "Run · check, test", "git push"] {
        assert!(has(&rail, subject), "{subject} is a charter row");
    }
    for badge in ["ASK", "AUTO", "OFF"] {
        assert!(has(&rail, badge), "{badge} badge");
    }
    assert!(has(&rail, "1.84 / 12.00"), "the budget's readout");
    assert!(has(&rail, "STOP AT 10.00"), "and its breaker");
}

/// A fresh session's rail is honest: every section shows its empty
/// state, and no meter or ledger stands in as furniture.
#[test]
fn empty_sections_show_their_empty_states() {
    let model = Model::new(Size::new(120, 40));
    let rail = RailComposition::el(&model, Medium::Night, CELL_W);
    assert!(has(&rail, "No checkpoints yet"));
    assert!(has(&rail, "No rules bound"));
    assert!(has(&rail, "No budget bound"));
    assert!(!has(&rail, "/"), "no meter readout on an unbound budget");
}

/// The divider drag reshapes this composition through the one shared
/// `update`: the width is columns × advance off the model, and no
/// width state lives here.
#[test]
fn the_divider_drag_reshapes_the_rail_through_the_one_update() {
    let size = Size::new(120, 40);
    let mut model = Fixture::with_pixels(Fixture::mock("05", size).expect("fixture"));
    let before = model.rail().content_cols(size.cols);
    update(
        &mut model,
        Msg::Mouse {
            event: MouseEvent::new(MouseKind::Down(Button::Left), 90, 10),
            target: Some(Widget::RailDivider.id()),
        },
    );
    assert!(model.rail().is_dragging());
    update(
        &mut model,
        Msg::Mouse {
            event: MouseEvent::new(MouseKind::Drag(Button::Left), 80, 10),
            target: None,
        },
    );
    update(
        &mut model,
        Msg::Mouse {
            event: MouseEvent::new(MouseKind::Up(Button::Left), 80, 10),
            target: None,
        },
    );
    let after = model.rail().content_cols(size.cols);
    assert_ne!(before, after, "the drag moved the rail");
    let rail = RailComposition::el(&model, Medium::Night, CELL_W);
    let dragged = model.rail().dragged_cols(size.cols);
    assert_ne!(
        dragged, 0,
        "the drag is measured in columns off the default"
    );
    #[allow(clippy::cast_precision_loss)]
    let expected = Voice::RAIL_WIDTH + dragged as f32 * CELL_W;
    assert_eq!(
        rail.width,
        Some(expected),
        "the composition's width is the volume's 248 moved by the dragged columns times the cell advance"
    );
}

/// Untouched, the rail's width is the volume's own 248 — not the cell
/// rail's columns times the advance, which put the seam at 848 where
/// the capture's is at 852 and every left-aligned run 4px off — and
/// its first child is the seam (1px), with the column taking the rest.
#[test]
fn the_untouched_rail_is_the_volumes_width_with_its_seam_inside() {
    let model = Fixture::mock("05", Size::new(120, 40)).expect("view 05 has a fixture");
    let rail = RailComposition::el(&model, Medium::Night, CELL_W);
    assert_eq!(model.rail().dragged_cols(120), 0, "nobody dragged");
    let expected_width = Voice::RAIL_WIDTH;
    assert_eq!(expected_width, 248.0, "the volume's `width:248px`");
    let expected_column_w = expected_width - 1.0;
    assert_eq!(rail.width, Some(expected_width));
    assert_eq!(rail.children[0].width, Some(1.0), "the seam");
    assert_eq!(
        rail.children[1].width,
        Some(expected_column_w),
        "the column is the rest"
    );
}

/// The capture's anatomy: the head entry over the newest checkpoints newest-first, the
/// caps wearing their glyphs, and the charter's cap carrying the rail's mode word.
#[test]
fn the_rail_reads_the_captures_anatomy() {
    let model = Fixture::mock("05", Size::new(120, 40)).expect("view 05 has a fixture");
    let rail = RailComposition::el(&model, Medium::Night, CELL_W);
    let texts = texts(&rail);
    let at = |needle: &str| {
        texts
            .iter()
            .position(|t| t == needle)
            .unwrap_or_else(|| panic!("{needle} in {texts:?}"))
    };
    assert!(at("head") < at("now") && at("now") < at("2 receipts after cp-4"));
    assert!(
        at("2 receipts after cp-4") < at("cp-4 · before golden update"),
        "the head entry comes first"
    );
    assert!(
        at("cp-4 · before golden update") < at("cp-3 · iter 3 ·"),
        "then the newest checkpoints, newest first"
    );
    assert!(at("Charter · Draft mode") > at("✓✓✓"));
    let mut marks = Vec::new();
    rail.each(&mut |_, element| {
        if let Kind::Mark { mark, px, .. } = &element.kind {
            marks.push((format!("{mark:?}"), *px));
        }
    });
    assert_eq!(
        marks,
        [
            ("Icon(Checkpoint)".to_owned(), RailComposition::CAP_ICON_PX),
            ("Icon(Agent)".to_owned(), RailComposition::CAP_ICON_PX),
            ("Icon(Session)".to_owned(), RailComposition::CAP_ICON_PX),
        ],
        "the three caps — the bound agent's among them — wear their glyphs at the cap's own size"
    );
}

/// The ledger head carries the loop's verbs — the capture's three key-hint pairs under
/// the receipts line and over the first checkpoint row, their caps the registry's keys
/// — and the pause pair reads Resume while the loop holds a boundary.
#[test]
fn the_ledger_head_carries_the_loops_verbs_and_pause_reads_resume_while_held() {
    let mut model = Fixture::mock("05", Size::new(120, 40)).expect("view 05 has a fixture");
    let rail = RailComposition::el(&model, Medium::Night, CELL_W);
    let runs = texts(&rail);
    let at = |needle: &str| {
        runs.iter()
            .position(|t| t == needle)
            .unwrap_or_else(|| panic!("{needle} in {runs:?}"))
    };
    assert!(at("2 receipts after cp-4") < at("space"));
    assert!(at("space") < at("Pause"));
    assert!(at("Pause") < at("s") && at("s") < at("Step"));
    assert!(at("Step") < at("c") && at("c") < at("Checkpoint"));
    assert!(
        at("Checkpoint") < at("cp-4 · before golden update"),
        "under the head, over the rows"
    );
    update(
        &mut model,
        Msg::Loop(LoopEvent::Paused("gates failed".to_owned())),
    );
    let held = RailComposition::el(&model, Medium::Night, CELL_W);
    assert!(has(&held, "Resume"), "held, the pair resumes");
    assert!(!texts(&held).iter().any(|t| t == "Pause"));
}

/// A rail no session seeded a mode word for reads the bare `Charter`
/// rather than a dangling dot.
#[test]
fn an_unseeded_charter_cap_is_the_bare_word() {
    let mut model = Model::new(Size::new(120, 40));
    model
        .rail_mut()
        .set_charter(vec![magistr_cockpit::cockpit::CharterRule::new(
            "git push",
            magistr_cockpit::cockpit::RuleMode::Off,
        )]);
    let rail = RailComposition::el(&model, Medium::Night, CELL_W);
    assert!(texts(&rail).contains(&"Charter".to_owned()));
    assert!(!has(&rail, "Charter ·"));
}

/// The agent block: absent until the handshake names a model — two seconds of "no agent
/// yet" would be furniture — then the model under its mode word and the ledger line,
/// both read off the rail's own block.
#[test]
fn the_agent_block_is_drawn_once_the_handshake_names_the_model() {
    let mut model = Model::new(Size::new(120, 40));
    model.rail_mut().set_mode("Workflow mode");
    let before = RailComposition::el(&model, Medium::Night, CELL_W);
    assert!(!has(&before, "Agent"), "no block before the handshake");
    update(
        &mut model,
        Msg::Agent(magistr_application::sessions::AgentEvent::Initialized {
            session_id: "s1".to_owned(),
            model: Some("sonnet".to_owned()),
        }),
    );
    let after = RailComposition::el(&model, Medium::Night, CELL_W);
    assert!(has(&after, "Agent"), "the section's cap");
    assert!(has(&after, "sonnet · Workflow mode"), "the identity line");
    assert!(
        has(&after, "0.00 USD · 0 min · 0 requests"),
        "the ledger line"
    );
}

/// The charter's offer: a row under the rules carrying the scope, its record and
/// `Delegate` — a focusable control in the offer's id space, routed to the cockpit's
/// own widget — and no such row on a rail with nothing to offer.
#[test]
fn a_charter_offer_is_a_row_with_delegate_at_its_edge() {
    let plain = Fixture::mock("05", Size::new(120, 40)).expect("view 05");
    assert!(!has(
        &RailComposition::el(&plain, Medium::Night, CELL_W),
        "Delegate"
    ));
    let offered = Fixture::rail_offer(Size::new(120, 40));
    let rail = RailComposition::el(&offered, Medium::Day, CELL_W);
    assert!(has(&rail, "Edit · crates/magistr-tui"));
    assert!(has(&rail, "allowed 14 of 14"));
    assert!(has(&rail, "Delegate"));
    let mut controls = Vec::new();
    rail.each(&mut |_, element| {
        if let Some(id) = element.id
            && (RailComposition::OFFER..RailComposition::OFFER_END).contains(&id.0)
        {
            controls.push((id, element.focusable));
        }
    });
    assert_eq!(
        controls,
        [(ElementId(RailComposition::OFFER), true)],
        "the offer's Delegate carries the first offer id and takes focus"
    );
    assert_eq!(
        crate::comp_root::RootComposition::target_of(&offered, ElementId(RailComposition::OFFER)),
        Some(Widget::charter_offer(0))
    );
}

/// With no loop live, a session whose host starts work in place wears the two starts
/// where the loop's verbs sit — focusable, named, in the start ids — and none of the
/// loop's verbs; a live loop wears its verbs and no start.
#[test]
fn the_ledger_head_offers_the_starts_while_idle_and_the_loops_verbs_only_while_live() {
    use crate::work_controls::WorkControls;
    let ids = |rail: &Element| {
        let mut ids = Vec::new();
        rail.each(&mut |_, element| {
            if let Some(id) = element.id
                && (LedgerVerbs::PAUSE.0..=WorkControls::LOOP.0).contains(&id.0)
            {
                ids.push((id, element.focusable, element.label.clone()));
            }
        });
        ids
    };
    let idle = Fixture::rail_idle(Size::new(120, 40));
    assert!(!idle.loop_is_live());
    let rail = RailComposition::el(&idle, Medium::Night, CELL_W);
    assert_eq!(
        ids(&rail),
        [
            (WorkControls::NEXT, true, Some("Start next item".to_owned())),
            (WorkControls::LOOP, true, Some("Start loop".to_owned())),
        ]
    );
    assert!(
        has(&rail, "alt-n") && has(&rail, "alt-l"),
        "the registry's keys"
    );
    assert!(!has(&rail, "Pause") && !has(&rail, "Abort"));

    let live = Fixture::mock("05", Size::new(120, 40))
        .expect("view 05")
        .with_hosted_work();
    assert!(live.loop_is_live());
    let rail = RailComposition::el(&live, Medium::Night, CELL_W);
    let live_ids: Vec<_> = ids(&rail).into_iter().map(|(id, ..)| id).collect();
    assert_eq!(
        live_ids,
        [
            LedgerVerbs::PAUSE,
            LedgerVerbs::STEP,
            LedgerVerbs::CHECKPOINT,
            LedgerVerbs::ABORT
        ]
    );
    assert!(ids(&rail).iter().all(|(_, focusable, _)| *focusable));
    assert!(!has(&rail, "Start loop"));

    // An ended loop on a session no host starts work for: neither row.
    let mut ended = Fixture::mock("05", Size::new(120, 40)).expect("view 05");
    update(
        &mut ended,
        Msg::Loop(LoopEvent::Ended("finished".to_owned())),
    );
    assert_eq!(ids(&RailComposition::el(&ended, Medium::Night, CELL_W)), []);

    // A fresh hosted session has no checkpoint yet and still offers the starts.
    let fresh = Model::new(Size::new(120, 40)).with_hosted_work();
    let rail = RailComposition::el(&fresh, Medium::Night, CELL_W);
    assert!(has(&rail, "No checkpoints yet") && has(&rail, "Start next item"));
}
