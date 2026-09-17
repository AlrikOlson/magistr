use magistr_application::sessions::{AgentEvent, ToolStatus, ToolVerb};
use magistr_cockpit::cockpit::{Model, Msg, Size, update};
use magistr_cockpit::harness::fixtures;

use super::*;
use crate::elevation::Elevation;
use crate::fixture::Fixture;

const CELL: (f32, f32) = (9.0, 20.0);

fn runs(root: &Element) -> Vec<(String, Srgba, Voice)> {
    let mut runs = Vec::new();
    root.each(&mut |_, element| {
        if let crate::element::Kind::Text { content, voice } = &element.kind {
            runs.push((
                content.clone(),
                element.fill.expect("a run carries ink"),
                *voice,
            ));
        }
    });
    runs
}

/// Tick past the cockpit's arrival window, so what is measured is the
/// settled feed rather than a row still fading in.
fn settle(model: &mut Model) {
    for _ in 0..=magistr_cockpit::cockpit::Motion::ARRIVAL_STEPS {
        let _ = update(model, Msg::Tick);
    }
}

fn glows(root: &Element) -> Vec<(Srgba, f32)> {
    let mut glows = Vec::new();
    root.each(&mut |_, element| glows.extend(element.glow));
    glows
}

/// A running tool card on a window wears the breathe halo off the foot of the wave and
/// nothing at all on it: the settled still is byte-identical to a feed that is not
/// working.
#[test]
fn a_running_tool_card_breathes_off_the_foot_and_rests_on_it() {
    let mut model = Fixture::working(Size::new(120, 40));
    let rest = FeedComposition::el(&model, Medium::Night, CELL);
    assert!(glows(&rest).is_empty(), "the foot: no halo");
    for _ in 0..(magistr_cockpit::cockpit::Motion::GLOW_STEPS / 2) {
        let _ = update(&mut model, Msg::Tick);
    }
    let crest = FeedComposition::el(&model, Medium::Night, CELL);
    let halos = glows(&crest);
    assert_eq!(halos.len(), 1, "one working thing, one halo");
    assert!(halos[0].0.a > 0.25);
    // A terminal never wears it.
    model.set_surface(magistr_cockpit::cockpit::Surface::Terminal);
    assert!(glows(&FeedComposition::el(&model, Medium::Night, CELL)).is_empty());
}

/// A thought being thought — the last row, no card running — lifts its
/// ink toward the text pen at the crest and is its faint self at the
/// foot; once a tool runs, the card is the working thing and the
/// thought rests.
#[test]
fn a_thought_being_thought_lifts_its_ink_until_a_card_takes_over() {
    let lex = Lexicon::of(Medium::Night);
    let mut model =
        Model::new(Size::new(120, 40)).with_surface(magistr_cockpit::cockpit::Surface::Native);
    let _ = update(
        &mut model,
        Msg::Agent(AgentEvent::Thought("Weighing the two fixes.".to_owned())),
    );
    settle(&mut model);
    let ink_of = |model: &Model| {
        runs(&FeedComposition::el(model, Medium::Night, CELL))
            .into_iter()
            .find(|(text, _, _)| text.contains("Weighing"))
            .map(|(_, ink, _)| ink)
            .expect("the thought row")
    };
    // The settle above lands off the foot (ARRIVAL_STEPS + 1 ticks), so
    // walk to the next foot first, then to the crest.
    while magistr_cockpit::cockpit::Motion::glow_phase(model.now()) > 1e-6 {
        let _ = update(&mut model, Msg::Tick);
    }
    assert_eq!(ink_of(&model), lex.ov0, "the foot is the faint pen");
    for _ in 0..(magistr_cockpit::cockpit::Motion::GLOW_STEPS / 2) {
        let _ = update(&mut model, Msg::Tick);
    }
    let lifted = ink_of(&model);
    assert_ne!(lifted, lex.ov0, "the crest lifts the ink");
    assert!(
        (lifted.r - lex.tx.r).abs() < 0.02,
        "toward the text pen: {lifted:?} vs tx {:?} (ov0 {:?})",
        lex.tx,
        lex.ov0
    );
    // A running card takes the breath: the thought rests again.
    let _ = update(
        &mut model,
        Msg::Agent(AgentEvent::ToolCall {
            id: "tc".to_owned(),
            title: "cargo check".to_owned(),
            verb: ToolVerb::Tool,
            status: ToolStatus::InProgress,
            diff: None,
            images: Vec::new(),
            record: Default::default(),
        }),
    );
    assert_eq!(ink_of(&model), lex.ov0);
}

/// The streamed turn's rows ride the composition in their own voices:
/// the wrap and the window are the model's, a code row is mono because
/// its kind says columns mean something, and a thought is the faint
/// ink.
#[test]
fn the_turn_rows_ride_in_their_kinds_voices() {
    let model = Fixture::turn(Size::new(120, 40));
    let lex = Lexicon::of(Medium::Night);
    let feed = FeedComposition::el(&model, Medium::Night, CELL);
    let runs = runs(&feed);
    assert!(!runs.is_empty(), "the turn is on the feed");
    let code = runs
        .iter()
        .find(|(text, ..)| text.contains("fn paint"))
        .expect("the fenced block is on the feed");
    assert!(code.2.is_mono(), "a code row is mono");
    let thought = runs
        .iter()
        .find(|(text, ..)| text.contains("Read the roadmap"))
        .expect("the thought is on the feed");
    assert_eq!(thought.1, lex.ov0, "a thought is the faint ink");
}

/// A tool call floats as a card in the volume's anatomy — the verb's
/// icon and word in the verb's own hue, the subject beside it, the
/// measure at the far edge once there is one — and its status recolors
/// the verb through the one `update`: announce in progress, complete
/// it, fail it, and the same composition reads each state.
#[test]
fn a_tool_call_cards_and_its_status_recolors_the_verb() {
    let lex = Lexicon::of(Medium::Night);
    let mut model = Model::new(Size::new(120, 40));
    update(
        &mut model,
        Msg::Agent(AgentEvent::ToolCall {
            id: "tc-1".to_owned(),
            title: "Run the gates".to_owned(),
            verb: ToolVerb::Run,
            status: ToolStatus::InProgress,
            diff: None,
            images: Vec::new(),
            record: Default::default(),
        }),
    );
    // Settled past the arrival fade: this test is about what a status
    // colours the head, not about how the card comes in.
    settle(&mut model);
    let feed = FeedComposition::el(&model, Medium::Night, CELL);
    let mut carded = false;
    let mut marks = 0;
    feed.each(&mut |_, element| {
        if let crate::element::Kind::Mark { mark, .. } = &element.kind {
            marks += usize::from(matches!(
                mark,
                crate::mark::Mark::Icon(crate::icon::Icon::Run)
            ));
        }
        if element.elevation != Elevation::canvas() {
            let mut inside = Vec::new();
            element.each(&mut |_, e| {
                if let crate::element::Kind::Text { content, .. } = &e.kind {
                    inside.push(content.clone());
                }
            });
            carded |= inside.iter().any(|t| t.contains("Run the gates"));
        }
    });
    assert!(carded, "the call floats as a card");
    assert_eq!(marks, 1, "the verb's icon leads the head");
    let find = |feed: &Element, needle: &str| {
        runs(feed)
            .into_iter()
            .find(|(text, ..)| text == needle)
            .unwrap_or_else(|| panic!("{needle:?} is on the head"))
    };
    assert_eq!(find(&feed, "run").1, lex.green, "the verb wears its hue");
    assert_eq!(
        find(&feed, "Run the gates").1,
        lex.sb0,
        "the subject in the subdued ink"
    );
    assert!(
        !runs(&feed)
            .iter()
            .any(|(text, ..)| text.contains("receipt #")),
        "nothing to measure while it runs"
    );
    update(
        &mut model,
        Msg::Agent(AgentEvent::ToolCallUpdate {
            id: "tc-1".to_owned(),
            title: None,
            status: Some(ToolStatus::Completed),
            diff: None,
            duration_ms: Some(420),
            images: Vec::new(),
            record: Default::default(),
        }),
    );
    settle(&mut model);
    let feed = FeedComposition::el(&model, Medium::Night, CELL);
    let measure = find(&feed, "420 ms \u{b7} receipt #1");
    assert_eq!(measure.1, lex.ov0, "a tool's measure is the faint ink");
    assert_eq!(measure.2, Voice::LEDGER_META);
    update(
        &mut model,
        Msg::Agent(AgentEvent::ToolCallUpdate {
            id: "tc-1".to_owned(),
            title: None,
            status: Some(ToolStatus::Failed),
            diff: None,
            duration_ms: None,
            images: Vec::new(),
            record: Default::default(),
        }),
    );
    settle(&mut model);
    let feed = FeedComposition::el(&model, Medium::Night, CELL);
    assert_eq!(find(&feed, "run").1, lex.red, "a failed call's verb is red");
}

/// The collapsed edit card wears the capture's anatomy (view 05): the
/// head — `edit` in teal with its icon, the path, `+47 · receipt #241`
/// in green at the far edge — then the preview of what changed as a
/// diff line, then the receipt strip naming the rollback and the
/// checkpoint whose reach covers it. The cell painter's one summary
/// row (`✓ edit path · +1 −0`) is not on the window at all.
#[test]
fn the_collapsed_edit_card_draws_the_captures_anatomy() {
    let model = Fixture::mock("05", Size::new(120, 40)).expect("view 05 has a fixture");
    let lex = Lexicon::of(Medium::Night);
    let feed = FeedComposition::el(&model, Medium::Night, CELL);
    let runs = runs(&feed);
    let find = |needle: &str| {
        runs.iter()
            .find(|(text, ..)| text == needle)
            .unwrap_or_else(|| {
                panic!(
                    "{needle:?} is on the feed: {:?}",
                    runs.iter().map(|(text, ..)| text).collect::<Vec<_>>()
                )
            })
    };
    assert_eq!(find("edit").1, lex.teal, "the verb in its hue");
    assert_eq!(find("cockpit/surface.rs").1, lex.sb0, "the path");
    let measure = find("+47 \u{b7} receipt #241");
    assert_eq!(measure.1, lex.green);
    assert_eq!(measure.2, Voice::LEDGER_META);
    let preview = find("+ pub enum Surface { Terminal, Native }");
    assert_eq!(preview.2, Voice::DIFFLINE, "the preview is a diff line");
    let strip = find("  rollback available \u{b7} inside cp-4\u{2019}s reach");
    assert_eq!(strip.1, lex.ov0, "the strip is a context line");
    assert!(
        !runs.iter().any(|(text, ..)| text.contains("+1 \u{2212}0")),
        "the grid's summary row stays on the grid"
    );
    let mut icons = 0;
    feed.each(&mut |_, element| {
        if let crate::element::Kind::Mark { mark, .. } = &element.kind {
            icons += usize::from(matches!(
                mark,
                crate::mark::Mark::Icon(crate::icon::Icon::Edit)
            ));
        }
    });
    assert_eq!(icons, 1, "the edit icon leads the head");
}

/// A paragraph is one run: the model holds it as the rows its mono
/// column count cut, and the window joins them back so the shaper that
/// draws the volume's sans breaks it at the column's measure — where
/// the capture breaks it — rather than where a terminal would.
#[test]
fn a_paragraph_is_one_run_for_the_shaper_to_break() {
    let model = Fixture::mock("05", Size::new(120, 40)).expect("view 05 has a fixture");
    let rows = model
        .turn()
        .rows()
        .iter()
        .filter(|row| row.text.contains("vocabulary") || row.text.contains("registry"))
        .count();
    assert!(rows >= 2, "the model still cuts the paragraph into rows");
    let feed = FeedComposition::el(&model, Medium::Night, CELL);
    let mut paragraph = None;
    feed.each(&mut |_, element| {
        if let crate::element::Kind::Text { content, .. } = &element.kind
            && content.starts_with("The vocabulary")
        {
            paragraph = Some((content.clone(), element.nowrap));
        }
    });
    let (text, nowrap) = paragraph.expect("the paragraph is on the feed");
    assert!(
        text.ends_with("answers the same questions."),
        "one run holds the whole paragraph: {text:?}"
    );
    assert!(!nowrap, "the shaper breaks it, so no nowrap");
}

/// An image block reserves its whole footprint at the block's head, in
/// the surface's own pixels — the placement is the model's, the pixel
/// size the renderer's.
#[test]
fn an_image_reserves_its_footprint_at_its_head() {
    let model = Fixture::imaged(Size::new(120, 40), 100, 40);
    let (_, _, (cols, rows)) = (0..model.turn().len())
        .find_map(|row| model.turn().image_at_row(row))
        .expect("the image is in the turn");
    let feed = FeedComposition::el(&model, Medium::Night, CELL);
    let mut reserved = false;
    feed.each(&mut |_, element| {
        reserved |= element.width == Some(f32::from(cols) * CELL.0)
            && element.height == Some(f32::from(rows) * CELL.1);
    });
    assert!(
        reserved,
        "the footprint is reserved at the placement's size"
    );
}

/// A row that just arrived comes in from the dim pen and settles at its
/// own ink over the cockpit's arrival window — the same transient the
/// cell painter fades on (`Motion::row_arrival`), so the two surfaces
/// cannot disagree about when a row is new. At the end of the ramp the
/// composition is exactly the settled one, byte for byte.
#[test]
fn an_arriving_row_fades_in_from_the_dim_pen_and_settles_exactly() {
    use magistr_cockpit::cockpit::Motion;

    let mut model = Model::new(Size::new(120, 40));
    let _ = update(
        &mut model,
        Msg::Agent(AgentEvent::Text("arriving\n".to_owned())),
    );
    let lex = Lexicon::of(Medium::Night);
    let ink_of = |model: &Model| {
        runs(&FeedComposition::el(model, Medium::Night, CELL))
            .into_iter()
            .find(|(text, ..)| text.contains("arriving"))
            .expect("the row is on the feed")
            .1
    };

    // On the fold that brought it in, the row is dimmer than its pen.
    let arriving = ink_of(&model);
    assert_ne!(
        arriving, lex.tx,
        "a row the last fold added does not snap in at full ink"
    );
    let toward_dim = crate::oklab::mix(lex.tx, lex.sb0, 0.5);
    assert!(
        (arriving.r - lex.tx.r).abs() > f32::EPSILON
            || (arriving.g - lex.tx.g).abs() > f32::EPSILON,
        "it comes in from the dim pen: {arriving:?} vs {toward_dim:?}"
    );

    // Ticked past the arrival window it is exactly its own pen again.
    for _ in 0..=Motion::ARRIVAL_STEPS {
        let _ = update(&mut model, Msg::Tick);
    }
    assert_eq!(
        ink_of(&model),
        lex.tx,
        "a settled row is exactly the row it always was"
    );
}

/// Reduced motion is structural: the row is painted settled on the very
/// frame it arrives, with no ramp to sit through.
#[test]
fn reduced_motion_paints_an_arriving_row_settled() {
    use magistr_cockpit::theme::{MotionMode, Theme, ThemeMode};

    let mut model = Model::new(Size::new(120, 40))
        .with_theme(Theme::forced(ThemeMode::Dark).with_motion(MotionMode::Reduced));
    let _ = update(
        &mut model,
        Msg::Agent(AgentEvent::Text("arriving\n".to_owned())),
    );
    let lex = Lexicon::of(Medium::Night);
    let ink = runs(&FeedComposition::el(&model, Medium::Night, CELL))
        .into_iter()
        .find(|(text, ..)| text.contains("arriving"))
        .expect("the row is on the feed")
        .1;
    assert_eq!(ink, lex.tx, "reduced motion draws the final frame");
}

/// Every card head is the card's hit target: it wears its row's id in the HEADS space,
/// takes focus and walks under the arrows, and the root resolves it to the cockpit's
/// own stage-row widget — so the window opens the card through the toggle the terminal
/// already has, and the two share one expansion state.
#[test]
fn every_card_head_is_a_hit_target_routed_to_the_stage_row() {
    use crate::comp_root::RootComposition;
    use magistr_cockpit::cockpit::Widget;
    let model = Fixture::mock("05", Size::new(120, 40)).expect("view 05 has a fixture");
    let head_row = model
        .turn()
        .lines()
        .iter()
        .find(|line| matches!(line.kind(), LineKind::Edit { .. }))
        .map(Line::row_start)
        .expect("the edit card");
    let feed = FeedComposition::el(&model, Medium::Night, CELL);
    let id = ElementId(FeedComposition::HEADS + head_row as u64);
    let mut head = None;
    feed.each(&mut |_, element| {
        if element.id == Some(id) {
            head = Some((element.focusable, element.walkable, element.label.clone()));
        }
    });
    assert_eq!(
        head,
        Some((true, true, Some("edit cockpit/surface.rs".to_owned()))),
        "the head wears the id, takes focus, walks, and is named"
    );
    assert_eq!(
        RootComposition::target_of(&model, id),
        Some(Widget::stage_row(head_row - model.top_row()))
    );
}

/// A click on a Run card's head, folded through the one `update`,
/// opens it in place into its command and output pane with the
/// receipt line naming the record's cut; a second click folds it. The
/// state is the line's own `expanded` — nothing the window keeps.
#[test]
fn a_click_on_a_run_cards_head_opens_its_detail_in_place() {
    use magistr_application::sessions::ToolRecord;
    use magistr_cockpit::cockpit::Widget;
    use magistr_cockpit::input::{Button, MouseEvent, MouseKind};
    let mut model = Model::new(Size::new(120, 40));
    let mut record = ToolRecord {
        input: Some(magistr_application::sessions::ToolPayload::Json(
            serde_json::json!({"command": "cargo nextest run --workspace"}).to_string(),
        )),
        output: Some(magistr_application::sessions::ToolPayload::Json(
            serde_json::json!("348 passed\n0 failed\n").to_string(),
        )),
        ..Default::default()
    };
    record.truncated_from = Some(200_000);
    update(
        &mut model,
        Msg::Agent(AgentEvent::ToolCall {
            id: "tc-run".to_owned(),
            title: "Run the gates".to_owned(),
            verb: ToolVerb::Run,
            status: ToolStatus::Completed,
            diff: None,
            images: Vec::new(),
            record: Box::new(record),
        }),
    );
    settle(&mut model);
    let texts = |model: &Model| {
        runs(&FeedComposition::el(model, Medium::Night, CELL))
            .into_iter()
            .map(|(text, ..)| text)
            .collect::<Vec<_>>()
    };
    let closed = texts(&model);
    assert!(!closed.iter().any(|t| t == "348 passed"), "{closed:?}");
    let head = model.turn().lines()[0].row_start();
    let click = |model: &mut Model| {
        for kind in [MouseKind::Down(Button::Left), MouseKind::Up(Button::Left)] {
            update(
                model,
                Msg::Mouse {
                    event: MouseEvent::new(kind, 0, 0),
                    target: Some(Widget::stage_row(head - model.top_row())),
                },
            );
        }
    };
    click(&mut model);
    assert!(matches!(
        model.turn().lines()[0].kind(),
        LineKind::Tool { expanded: true, .. }
    ));
    let open = texts(&model);
    for needle in [
        "command",
        "cargo nextest run --workspace",
        "output",
        "348 passed",
        "0 failed",
        "receipt #1 \u{b7} cut at 64 KiB \u{b7} 196 KiB recorded",
    ] {
        assert!(open.iter().any(|t| t == needle), "{needle:?} in {open:?}");
    }
    assert!(
        !open.iter().any(|t| t.starts_with("  Run the gates")),
        "the grid's wrapped title rows stay on the grid"
    );
    click(&mut model);
    assert!(matches!(
        model.turn().lines()[0].kind(),
        LineKind::Tool {
            expanded: false,
            ..
        }
    ));
    assert_eq!(texts(&model), closed, "folded: the card it was");
}

/// A recorded check cards on the window — `check` in the gate's ink
/// with the gate icon, the name, `exit 101 · 900 ms` — and opens into
/// the gate's own diagnostic rows under its command.
#[test]
fn a_check_cards_and_opens_into_its_diagnostics() {
    use magistr_cockpit::engine::EngineEvent;
    let lex = Lexicon::of(Medium::Night);
    let mut model = Model::new(Size::new(120, 40));
    update(
        &mut model,
        Msg::Engine(EngineEvent::local(fixtures::frame(
            "ship",
            "check_recorded",
            serde_json::json!({"check": {
                "name": "lint", "command": "cargo clippy", "passed": false,
                "exit_code": 101, "duration_ms": 900,
                "details": "error: unused variable\n  --> src/main.rs:3:9",
            }}),
        ))),
    );
    settle(&mut model);
    let feed = FeedComposition::el(&model, Medium::Night, CELL);
    let find = |feed: &Element, needle: &str| {
        runs(feed)
            .into_iter()
            .find(|(text, ..)| text == needle)
            .unwrap_or_else(|| panic!("{needle:?} is on the card"))
    };
    assert_eq!(
        find(&feed, "check").1,
        lex.red,
        "a failed check's verb is red"
    );
    assert_eq!(find(&feed, "lint").1, lex.sb0);
    assert_eq!(find(&feed, "exit 101 \u{b7} 900 ms").2, Voice::LEDGER_META);
    let mut gate_icons = 0;
    feed.each(&mut |_, element| {
        if let crate::element::Kind::Mark { mark, .. } = &element.kind {
            gate_icons += usize::from(matches!(
                mark,
                crate::mark::Mark::Icon(crate::icon::Icon::Gate)
            ));
        }
    });
    assert_eq!(gate_icons, 1, "the gate icon leads the head");
    assert!(
        !runs(&feed)
            .iter()
            .any(|(t, ..)| t.contains("unused variable")),
        "collapsed: the rows wait"
    );
    let head = model.turn().lines()[0].row_start();
    assert!(model.turn_mut().toggle_at_row(head));
    let feed = FeedComposition::el(&model, Medium::Night, CELL);
    assert_eq!(find(&feed, "cargo clippy").1, lex.tx);
    assert_eq!(find(&feed, "error: unused variable").1, lex.red);
    assert_eq!(find(&feed, "  --> src/main.rs:3:9").1, lex.sb0);
    assert_eq!(
        find(&feed, "failed \u{b7} exit 101 \u{b7} 900 ms").1,
        lex.ov0,
        "the receipt line"
    );
}

/// A run card whose call embeds a terminal opens into the screen the terminal streamed
/// — as cells, never as text rows: the caps and the command are runs, the program's
/// rows are the Cells leaf, whose label reads them back for a screen reader.
#[test]
fn a_run_card_with_a_terminal_opens_into_the_screens_cells() {
    use magistr_application::sessions::ToolRecord;
    use magistr_cockpit::input::KeyCode;
    let mut model = Model::new(Size::new(120, 40));
    update(
        &mut model,
        Msg::Agent(AgentEvent::ToolCall {
            id: "tc-run".to_owned(),
            title: "cargo test".to_owned(),
            verb: ToolVerb::Run,
            status: ToolStatus::InProgress,
            diff: None,
            images: Vec::new(),
            record: Box::new(ToolRecord {
                terminal: Some("term-1".to_owned()),
                ..Default::default()
            }),
        }),
    );
    update(
        &mut model,
        Msg::Agent(AgentEvent::Terminal {
            terminal_id: "term-1".to_owned(),
            bytes: b"\x1b[32m   Compiling\x1b[0m foo\r\n   Finished\r\n".to_vec(),
        }),
    );
    update(
        &mut model,
        Msg::Key(magistr_cockpit::input::KeyEvent::new(KeyCode::Enter)),
    );
    settle(&mut model);
    let feed = FeedComposition::el(&model, Medium::Night, CELL);
    let texts: Vec<String> = runs(&feed).into_iter().map(|(text, ..)| text).collect();
    for needle in ["command", "cargo test", "output"] {
        assert!(texts.iter().any(|t| t == needle), "{needle:?} in {texts:?}");
    }
    for absent in ["no output", "   Compiling foo", "   Finished"] {
        assert!(
            !texts.iter().any(|t| t == absent),
            "the screen is cells, not runs: {absent:?} in {texts:?}"
        );
    }
    let mut labels = Vec::new();
    feed.each(&mut |_, element| {
        if let crate::element::Kind::Grid { rows, .. } = &element.kind {
            assert_eq!(rows.len(), magistr_cockpit::cockpit::Term::PANE_ROWS);
            labels.push(element.label.clone().unwrap_or_default());
        }
    });
    assert_eq!(labels.len(), 1, "one Cells leaf under the open card");
    let (compiling, finished) = (
        labels[0].find("   Compiling foo\n").expect("the first row"),
        labels[0].find("   Finished").expect("the second row"),
    );
    assert!(
        compiling < finished,
        "the rows are the label, behind the rail: {:?}",
        labels[0]
    );
}

/// The held kill: while `k` is held past the dead zone the run card's head wears a red
/// band filled to the hold's fraction, behind its words; done, the band is gone and the
/// head's measure and the receipt line both say `killed by you` in red.
#[test]
fn the_held_kill_fills_the_head_and_the_kill_names_the_human() {
    let lex = Lexicon::of(Medium::Night);
    let killing = Fixture::pane_killing(Size::new(120, 40));
    let fraction = killing.hold_fraction("term-1").expect("held");
    assert!(fraction > 0.0 && fraction < 1.0, "{fraction}");
    let root = FeedComposition::el(&killing, Medium::Night, (9.0, 20.0));
    let mut bands = Vec::new();
    root.each(&mut |_, element| {
        if element
            .fill
            .is_some_and(|fill| fill != lex.red && fill.r > fill.g && fill.r > fill.b)
            && (element.grow - fraction).abs() < 1e-5
        {
            bands.push(element.grow);
        }
    });
    assert_eq!(bands.len(), 1, "one band, filled to the hold's fraction");
    assert!(
        !runs(&root)
            .iter()
            .any(|(text, ..)| text.contains("killed by you")),
        "nothing is killed yet"
    );

    let killed = Fixture::pane_killed(Size::new(120, 40));
    assert!(
        killed
            .term("term-1")
            .is_some_and(magistr_cockpit::cockpit::Term::is_killed)
    );
    let root = FeedComposition::el(&killed, Medium::Night, (9.0, 20.0));
    let words: Vec<(String, Srgba, Voice)> = runs(&root)
        .into_iter()
        .filter(|(text, ..)| text.contains("killed by you"))
        .collect();
    assert_eq!(
        words.len(),
        3,
        "the head's measure, the strip's word and the receipt line: {words:?}"
    );
    // The head's measure and the strip's word wear the failure hue;
    // the receipt line keeps its own meta ink, the words leading it.
    assert_eq!(
        words.iter().filter(|(_, ink, _)| *ink == lex.red).count(),
        2,
        "{words:?}"
    );
    assert!(
        words
            .iter()
            .any(|(text, ink, _)| text.starts_with("killed by you \u{b7}") && *ink == lex.ov0),
        "{words:?}"
    );
    let mut bands = 0;
    root.each(&mut |_, element| {
        if element.grow > 0.0 && element.grow < 1.0 {
            bands += 1;
        }
    });
    assert_eq!(bands, 0, "the band is gone once the hold is done");
}
