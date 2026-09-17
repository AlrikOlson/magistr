//! The runtime end to end over the fake terminal: the
//! placeholder cockpit runs headlessly through scripted keys, clicks,
//! resizes and the quit, and what comes out — the model, the byte stream —
//! is checked against what went in.

use std::time::Duration;

use magistr_cockpit::channel::Arrivals;
use magistr_cockpit::channel::Inbox;
use magistr_cockpit::cockpit::{
    Animation, FrameClock, LoopEvent, Model, Msg, Size, Widget, update, view,
};
use magistr_cockpit::hit::HitRegistry;
use magistr_cockpit::input::{Button, KeyCode, KeyEvent, Modifiers, MouseEvent, MouseKind};
use magistr_cockpit::job::{JobId, JobOutput};
use magistr_cockpit::layout::LayoutTree;
use magistr_tui::graphics::Env;
use magistr_tui::harness::{Capture, FakeAgent, FakeTerminal, Script};
use magistr_tui::runtime::{Options, Runtime};
use magistr_tui::term::Session;

const MS: fn(u64) -> Duration = Duration::from_millis;
const SIZE: Size = Size::new(80, 24);
const BSU: &[u8] = b"\x1b[?2026h";
const CLEAR: &[u8] = b"\x1b[2J";

/// The exact production layout supplies only coordinates: raw terminal bytes
/// still pass through capability negotiation, decoding, hit testing and update.
#[tokio::test]
async fn pixel_mouse_submits_at_drawn_bounds_for_varied_metrics_and_resize() {
    use magistr_cockpit::cockpit::Cmd;
    use magistr_tui::screenshot::{Profile, layout};
    for (width, height) in [(7, 17), (13, 29), (20, 40)] {
        let initial = Size::new(100, 40);
        let resized = Size::new(110, 44);
        let mut expected = Model::new(resized);
        update(
            &mut expected,
            Msg::CellSize(Some(magistr_cockpit::pixel::CellSize { width, height })),
        );
        update(&mut expected, Msg::Paste("Check Harbor".into()));
        let resolved = layout(
            &expected,
            &FrameClock::new(0, Duration::ZERO),
            Profile::Kitty,
        )
        .expect("production layout");
        let button = resolved.px(Widget::PromptAction.id()).expect("send button");
        let x = (button.x + button.width * 0.55).floor() as u32;
        let y = (button.y + button.height * 0.5).floor() as u32;
        let startup =
            format!("\x1b_Gi=31;OK\x1b\\\x1b[6;{height};{width}t\x1b[?1016;2$y\x1b[?62;4c");
        let click = format!("\x1b[<0;{};{}M\x1b[<0;{};{}m", x + 1, y + 1, x + 1, y + 1);
        let terminal = FakeTerminal::new(
            initial,
            Script::new()
                .bytes(MS(0), startup)
                .resize(MS(80), resized)
                .bytes(MS(80), format!("\x1b[6;{height};{width}t"))
                .bytes(MS(180), click)
                .close(MS(300)),
        );
        let capture = terminal.capture();
        let mut runtime = Runtime::new(terminal, options());
        let mut commands = runtime.commands().expect("commands");
        let mut model = Model::new(initial);
        update(&mut model, Msg::Paste("Check Harbor".into()));
        let model = runtime.run(model).await.expect("run");
        let mut submitted = Vec::new();
        while let Some(command) = commands.try_next() {
            if let Cmd::Prompt(text) = command {
                submitted.push(text);
            }
        }
        assert_eq!(
            submitted,
            ["Check Harbor"],
            "{width}×{height} pointer at {x},{y}"
        );
        assert!(model.prompt().is_empty());
        assert_eq!(model.size(), resized);
        assert_eq!(capture.count(b"\x1b[?1016h"), 1);
        assert!(
            capture.count(b"\x1b[16t") >= 2,
            "resize refreshes pixel metrics"
        );
        assert!(
            capture.bytes().ends_with(&leave_sequence()),
            "pixel mouse mode restored on exit"
        );
        let click = model
            .recent()
            .find_map(|message| match message {
                Msg::Mouse { event, target } if event.px == Some((x, y)) => Some((event, target)),
                _ => None,
            })
            .expect("physical report reached the model");
        assert_eq!(*click.1, Some(Widget::PromptAction.id()));
        assert_eq!(
            (click.0.col, click.0.row),
            (
                (x / u32::from(width)) as u16,
                (y / u32::from(height)) as u16
            )
        );
    }
}

#[tokio::test]
async fn graphics_startup_preserves_an_explicit_roadmap_entrypoint() {
    use magistr_cockpit::cockpit::{Pane, Selected, StageTab};
    use magistr_cockpit::engine::EngineEvent;
    use magistr_cockpit::harness::fixtures;
    let size = Size::new(144, 50);
    for status in [None, Some("in_progress"), Some("pending")] {
        for explicit in [false, true] {
            let terminal =
                FakeTerminal::new(size, Script::new().startup_replies(MS(0)).close(MS(120)));
            let mut model = Model::new(size);
            if let Some(status) = status {
                update(
                    &mut model,
                    Msg::Engine(EngineEvent::Seed(magistr_engine::NativeSnapshot {
                        chunks: vec![
                            fixtures::chunk_from(serde_json::json!({
                                "id": "recovery", "name": "Recovery checks",
                                "title": "Check delivery recovery", "status": "pending", "priority": 200
                            })),
                            fixtures::chunk_from(serde_json::json!({
                                "id": "retry", "name": "Saved retry limits",
                                "title": "Save retry budgets with receipts", "status": status, "priority": 100
                            })),
                        ],
                        steps: [3, 18, 144].into_iter().map(fixtures::step).collect(),
                        ..Default::default()
                    })),
                );
            }
            if explicit {
                model = model.with_tab(StageTab::Roadmap);
            }
            assert!(model.selected().is_none());
            let model = Runtime::new(terminal, options()).run(model).await.unwrap();
            assert!(model.has_pixels());
            assert_eq!(
                model.tab(),
                if explicit {
                    StageTab::Roadmap
                } else {
                    StageTab::Turn
                }
            );
            assert_eq!(model.roadmap_graph_px().is_some(), explicit);
            if status.is_some() {
                assert_eq!(model.pane(), Pane::Stage);
                if explicit {
                    assert_eq!(model.selected(), Some(&Selected::Chunk("retry".into())));
                    assert_eq!(
                        model.roadmap().selected().unwrap().title,
                        "Save retry budgets with receipts"
                    );
                } else {
                    assert_eq!(model.selected(), Some(&Selected::Step(144)));
                }
            } else {
                assert!(model.selected().is_none());
            }
        }
    }
}

#[tokio::test]
async fn an_early_loop_stop_survives_graphics_startup_with_its_reason_and_draft() {
    use magistr_cockpit::engine::EngineEvent;
    use magistr_cockpit::harness::fixtures;
    use magistr_cockpit::layout::Node;
    use magistr_tui::screenshot::{Profile, layout};

    fn nodes<'a>(node: &'a Node, out: &mut Vec<&'a Node>) {
        out.push(node);
        for child in node.child_nodes() {
            nodes(child, out);
        }
    }

    let size = Size::new(144, 50);
    for failed in [false, true] {
        let reason = if failed {
            "The agent could not start. Check the connection and try again."
        } else {
            "No ready work. Open Roadmap to review what is waiting."
        };
        let terminal =
            FakeTerminal::new(size, Script::new().startup_replies(MS(80)).close(MS(260)));
        let capture = terminal.capture();
        let runtime = Runtime::new(terminal, options());
        let mut model = Model::new(size);
        update(
            &mut model,
            Msg::Engine(EngineEvent::Seed(magistr_engine::NativeSnapshot {
                objective: Some(fixtures::objective_from(serde_json::json!({
                    "description": "Save delivery receipts", "status": "completed"
                }))),
                ..Default::default()
            })),
        );
        update(&mut model, Msg::Paste("Keep this unsent message".into()));
        assert!(model.rail().ship().is_shipped());
        // The real driver can stop before it creates an iteration. Queue that
        // news before the delayed terminal probe; no Started event is sent.
        if failed {
            runtime
                .inbox()
                .send(Msg::Loop(LoopEvent::Failed(reason.into())));
        }
        runtime
            .inbox()
            .send(Msg::Loop(LoopEvent::Ended(reason.into())));
        let model = runtime
            .run(model)
            .await
            .expect("early stop is not a runtime failure");
        assert!(model.has_pixels());
        assert!(model.loop_status().is_none());
        assert_eq!(model.prompt().text(), "Keep this unsent message");

        let clock = FrameClock::new(0, Duration::ZERO);
        let root = view(&model, &clock);
        let resolved = layout(&model, &clock, Profile::Kitty).expect("actual graphics layout");
        let mut leaves = Vec::new();
        nodes(&root, &mut leaves);
        let visible: Vec<_> = leaves
            .into_iter()
            .zip(resolved.placed())
            .filter_map(|(node, placed)| {
                node.text_metrics()
                    .filter(|_| placed.px.width > 0.0 && placed.px.height > 0.0)
                    .map(|text| (text.text(), placed))
            })
            .collect();
        let expected = format!("Work stopped: {reason}");
        let (_, reason_place) = visible
            .iter()
            .find(|(text, _)| *text == expected)
            .expect("the stop explanation is visible in the conversation");
        assert!(
            reason_place.px.height >= f32::from(model.cell_size().height),
            "the explanation has a complete line"
        );
        let status = if failed { "Work failed" } else { "Stopped" };
        assert!(visible.iter().any(|(text, _)| *text == status));
        assert!(!visible.iter().any(|(text, _)| *text == "Completed"));

        // Decode what the runtime actually wrote, before restoring the terminal,
        // and inspect the explanation's exact physical box for rendered glyphs.
        let exit = capture.writes().last().expect("terminal restore").0;
        let cell = model.cell_size();
        let rect = reason_place.px;
        let mut colors = std::collections::HashSet::new();
        for image in capture.pixels_before(exit) {
            let left = u32::from(image.col) * u32::from(cell.width);
            let top = u32::from(image.row) * u32::from(cell.height);
            for y in rect.y.ceil() as u32..rect.bottom().floor() as u32 {
                if y < top || y >= top + image.height {
                    continue;
                }
                for x in rect.x.ceil() as u32..rect.right().floor() as u32 {
                    if x < left || x >= left + image.width {
                        continue;
                    }
                    let at = (((y - top) * image.width + x - left) * 4) as usize;
                    colors.insert(image.rgba[at..at + 4].to_vec());
                }
            }
        }
        assert!(
            colors.len() > 8,
            "the terminal received the explanation's ink"
        );
    }
}

#[tokio::test]
async fn physical_roadmap_click_and_zoom_reach_the_live_graph_after_resize() {
    use magistr_cockpit::cockpit::{FlightRoadmapGraph, Selected, StageTab};
    use magistr_cockpit::harness::Scenes;
    use magistr_tui::screenshot::{Profile, layout};
    for (width, height) in [(10, 20), (13, 29)] {
        let initial = Size::new(120, 40);
        let resized = Size::new(144, 50);
        let mut expected = Scenes::roadmapped(resized).with_tab(StageTab::Turn);
        update(
            &mut expected,
            Msg::Mode(magistr_cockpit::capability::Mode::Graphics {
                pointer_shapes: true,
            }),
        );
        update(
            &mut expected,
            Msg::CellSize(Some(magistr_cockpit::pixel::CellSize { width, height })),
        );
        let placed = layout(
            &expected,
            &FrameClock::new(0, Duration::ZERO),
            Profile::Kitty,
        )
        .unwrap();
        let open = placed.px(Widget::TabRoadmap.id()).unwrap();
        let open_x = (open.x + open.width / 2.0) as u32;
        let open_y = (open.y + open.height / 2.0) as u32;
        update(
            &mut expected,
            Msg::Mouse {
                event: MouseEvent::new(MouseKind::Down(Button::Left), 0, 0),
                target: Some(Widget::TabRoadmap.id()),
            },
        );
        let placed = layout(
            &expected,
            &FrameClock::new(0, Duration::ZERO),
            Profile::Kitty,
        )
        .unwrap();
        let rect = placed.px(Widget::RoadmapGraph.id()).unwrap();
        let nodes = FlightRoadmapGraph::nodes(&expected, rect.width, rect.height);
        let point = &nodes[1];
        let x = (rect.x + point.x).round() as u32;
        let y = (rect.y + point.y).round() as u32;
        let startup =
            format!("\x1b_Gi=31;OK\x1b\\\x1b[6;{height};{width}t\x1b[?1016;2$y\x1b[?62;4c");
        let click = format!("\x1b[<0;{};{}M\x1b[<0;{};{}m", x + 1, y + 1, x + 1, y + 1);
        let wheel = format!("\x1b[<64;{};{}M", x + 1, y + 1);
        let open = format!(
            "\x1b[<0;{};{}M\x1b[<0;{};{}m",
            open_x + 1,
            open_y + 1,
            open_x + 1,
            open_y + 1
        );
        let terminal = FakeTerminal::new(
            initial,
            Script::new()
                .bytes(MS(0), startup)
                .resize(MS(80), resized)
                .bytes(MS(80), format!("\x1b[6;{height};{width}t"))
                .bytes(MS(180), open)
                .bytes(MS(260), click)
                .bytes(MS(320), wheel)
                .close(MS(480)),
        );
        let capture = terminal.capture();
        let runtime = Runtime::new(terminal, options());
        let model = runtime
            .run(Scenes::roadmapped(initial).with_tab(StageTab::Turn))
            .await
            .unwrap();
        assert_eq!(model.tab(), StageTab::Roadmap);
        assert_eq!(
            model.selected(),
            Some(&Selected::Chunk("tui2-2-rail".into()))
        );
        assert!(
            model.roadmap().zoom() > 1.0,
            "wheel at the drawn node zooms the graph"
        );
        let actual = model
            .roadmap_graph_px()
            .expect("runtime reported physical graph bounds");
        assert_eq!(actual, rect);
        assert_eq!(capture.count(b"\x1b[?1016h"), 1);
        assert!(model.recent().any(|message| matches!(message,
            Msg::Mouse { event, target } if event.px == Some((x, y)) && *target == Some(Widget::sky_star(1)))));
    }
}

#[tokio::test]
async fn unsupported_pixel_mouse_keeps_cell_reports_and_flat_sessions_never_enable_it() {
    let (model, capture) = run(Script::new().startup_replies(MS(0)).close(MS(80))).await;
    assert!(model.has_pixels());
    assert_eq!(
        capture.count(b"\x1b[?1016h"),
        0,
        "graphics alone cannot enable pixel reports"
    );
    let terminal = FakeTerminal::new(
        SIZE,
        Script::new()
            .bytes(MS(0), b"\x1b[?1016;2$y\x1b[?62;4c".as_slice())
            .close(MS(80)),
    );
    let capture = terminal.capture();
    Runtime::new(
        terminal,
        Options::new(Env::new("xterm-kitty", false, Some("text"))).probe_timeout(MS(500)),
    )
    .run(Model::new(SIZE))
    .await
    .expect("plain run");
    assert_eq!(capture.count(b"\x1b[?1016h"), 0);
}

#[tokio::test]
async fn graph_points_and_latest_use_real_terminal_actions_with_sparse_decisions() {
    use magistr_cockpit::cockpit::{Cmd, EngineCmd, FlightRecorder, Selected};
    use magistr_cockpit::engine::EngineEvent;
    use magistr_cockpit::harness::fixtures;
    use magistr_tui::screenshot::{Profile, layout};
    let size = Size::new(144, 50);
    let mut base = Model::new(size);
    update(
        &mut base,
        Msg::Engine(EngineEvent::Seed(magistr_engine::NativeSnapshot {
            steps: [3, 18, 144].into_iter().map(fixtures::step).collect(),
            ..Default::default()
        })),
    );
    update(&mut base, Msg::Paste("Keep this draft".into()));
    let closed =
        layout(&base, &FrameClock::new(0, Duration::ZERO), Profile::Kitty).expect("closed layout");
    let toggle = closed
        .px(Widget::ReasoningToggle.id())
        .expect("Show reasoning");
    let toggle = (
        (toggle.x + toggle.width / 2.0) as u16,
        (toggle.y + toggle.height / 2.0) as u16,
    );
    let mut selected = base.clone();
    update(
        &mut selected,
        Msg::Mouse {
            event: MouseEvent::new(MouseKind::Down(Button::Left), 0, 0),
            target: Some(Widget::ReasoningToggle.id()),
        },
    );
    update(
        &mut selected,
        Msg::Mouse {
            event: MouseEvent::new(MouseKind::Down(Button::Left), 0, 0),
            target: Some(Widget::trace_row(1)),
        },
    );
    update(
        &mut selected,
        Msg::Mouse {
            event: MouseEvent::new(MouseKind::Up(Button::Left), 0, 0),
            target: Some(Widget::trace_row(1)),
        },
    );
    let resolved = layout(
        &selected,
        &FrameClock::new(0, Duration::ZERO),
        Profile::Kitty,
    )
    .expect("layout");
    let graph = resolved.px(Widget::TraceGraph.id()).expect("graph");
    let point = FlightRecorder::graph(&selected, graph.width).points(graph.width, graph.height)[1];
    let point = (
        (graph.x + point.0).round() as u16,
        (graph.y + point.1).round() as u16,
    );
    let latest = resolved.px(Widget::LatestDecision.id()).expect("latest");
    let latest = (
        (latest.x + latest.width / 2.0) as u16,
        (latest.y + latest.height / 2.0) as u16,
    );
    for return_latest in [false, true] {
        let mut script = Script::new()
            .bytes(
                MS(0),
                b"\x1b_Gi=31;OK\x1b\\\x1b[6;20;10t\x1b[?1016;2$y\x1b[?62;4c".as_slice(),
            )
            .click(MS(60), toggle.0, toggle.1)
            .click(MS(140), point.0, point.1);
        if return_latest {
            script = script.click(MS(360), latest.0, latest.1);
        }
        let terminal = FakeTerminal::new(size, script.close(MS(500)));
        let mut runtime = Runtime::new(terminal, options());
        let mut commands = runtime.commands().expect("commands");
        magistr_tui::harness::FakeEngines::new().frame(MS(220), "think", "step_appended", serde_json::json!({"step":{"step_number":901,"purpose":"verify","thought":"Harbor checks passed"}})).play(runtime.inbox());
        let model = runtime.run(base.clone()).await.expect("run");
        assert_eq!(
            model.selected(),
            Some(&Selected::Step(if return_latest { 901 } else { 18 }))
        );
        assert_eq!(model.prompt().text(), "Keep this draft");
        assert!(
            model.viewport().is_following(),
            "record selection keeps conversation position"
        );
        while let Some(command) = commands.try_next() {
            assert!(
                !matches!(
                    command,
                    Cmd::Engine(EngineCmd::Checkpoint) | Cmd::Checkpoint(_)
                ),
                "return to latest cannot save a checkpoint: {command:?}"
            );
        }
    }
}

/// Where the 20 clicks land: two on the header, three per phase, three on
/// the stage, three on the footer (rects from the 80×24 layout snapshot).
const CLICKS: [(u16, u16); 20] = [
    (0, 0),
    (79, 0),
    (0, 1),
    (4, 1),
    (7, 1),
    (11, 1),
    (13, 1),
    (14, 1),
    (18, 1),
    (21, 1),
    (24, 1),
    (28, 1),
    (30, 1),
    (33, 1),
    (0, 2),
    (40, 12),
    (79, 22),
    (0, 23),
    (40, 23),
    (79, 23),
];

fn options() -> Options {
    Options::new(Env::new("xterm-kitty", false, None)).probe_timeout(MS(500))
}

async fn run(script: Script) -> (Model, Capture) {
    run_with(script, |_| {}).await
}

/// `run` with `producers` attached to the runtime's inbox first — what
/// the animation tests need, since only an agent makes the turn stream.
async fn run_with(script: Script, producers: impl FnOnce(Inbox)) -> (Model, Capture) {
    let terminal = FakeTerminal::new(SIZE, script);
    let capture = terminal.capture();
    let runtime = Runtime::new(terminal, options());
    producers(runtime.inbox());
    let model = runtime
        .run(Model::new(SIZE).with_tab(magistr_cockpit::cockpit::StageTab::Turn))
        .await
        .expect("the run succeeds");
    (model, capture)
}

/// kitty's OSC 22 pointer-shape strings: the hand, and the empty name
/// that hands the shape back to the terminal.
const POINTER_HAND: &[u8] = b"\x1b]22;pointer\x1b\\";
const POINTER_RESET: &[u8] = b"\x1b]22;\x1b\\";

/// The rect of `id` in the placeholder cockpit laid out at [`SIZE`].
fn widget_rect(id: magistr_cockpit::widget_id::WidgetId) -> magistr_cockpit::layout::Rect {
    // The kitty probe puts the cockpit on its pixel arrangement — one
    // gutter cell around everything — so the rect is read off that
    // arrangement, where the pointer will actually find the widget.
    let mut model = Model::new(SIZE).with_tab(magistr_cockpit::cockpit::StageTab::Turn);
    update(
        &mut model,
        Msg::Mode(magistr_cockpit::capability::Mode::Graphics {
            pointer_shapes: true,
        }),
    );
    let rect = magistr_tui::screenshot::layout(
        &model,
        &FrameClock::new(0, Duration::ZERO),
        magistr_tui::screenshot::Profile::Kitty,
    )
    .expect("pixel layout")
    .rect(id)
    .expect("the widget is laid out");
    assert!(!rect.is_empty(), "{id:?} covers cells");
    rect
}

/// on a kitty terminal the pointer becomes a hand over a tab,
/// stays one while it moves *within* the tab (one sequence, not one per
/// frame), and is handed back when it leaves for the header.
#[tokio::test]
async fn the_pointer_is_a_hand_over_commands_and_is_handed_back_when_it_leaves() {
    let tab = widget_rect(Widget::Commands.id());
    let script = Script::new()
        .startup_replies(MS(0))
        .mouse_move(MS(20), tab.x + 1, tab.y + tab.height / 2)
        .mouse_move(MS(60), tab.x + tab.width - 2, tab.y + tab.height / 2)
        .mouse_move(MS(100), 0, 0)
        .text(MS(140), "\x04");
    let (model, capture) = run(script).await;
    assert!(model.has_pointer_shapes(), "the probe found kitty");
    assert_eq!(
        capture.count(POINTER_HAND),
        1,
        "one hand for the whole stay on the tab"
    );
    assert_eq!(
        capture.count(POINTER_RESET),
        1,
        "handed back once, when the pointer left the tab"
    );
}

/// Quitting with the pointer still on a button hands the shape back on
/// the way out, so the terminal is never left drawing a hand.
#[tokio::test]
async fn the_pointer_is_handed_back_at_the_exit() {
    let tab = widget_rect(Widget::Commands.id());
    let script = Script::new()
        .startup_replies(MS(0))
        .mouse_move(MS(20), tab.x + 1, tab.y + tab.height / 2)
        .text(MS(60), "\x04");
    let (model, capture) = run(script).await;
    assert!(model.quit_requested());
    assert_eq!(capture.count(POINTER_HAND), 1);
    assert_eq!(capture.count(POINTER_RESET), 1, "handed back at the exit");
}

/// A terminal that never answered the kitty query is never sent a
/// pointer-shape string — it would print it.
#[tokio::test]
async fn a_plain_terminal_is_never_sent_a_pointer_shape() {
    let tab = widget_rect(Widget::Commands.id());
    let script = Script::new()
        .plain_replies(MS(0))
        .mouse_move(MS(20), tab.x + 1, tab.y + tab.height / 2)
        .mouse_move(MS(60), 0, 0)
        .text(MS(100), "\x04");
    let (model, capture) = run(script).await;
    assert!(!model.has_pointer_shapes());
    assert_eq!(capture.count(b"\x1b]22;"), 0, "no OSC 22 at all");
}

fn leave_sequence() -> Vec<u8> {
    let mut out = Vec::new();
    Session::cockpit().leave(&mut out);
    out
}

/// The hit registry of the placeholder laid out at `SIZE`.
fn registry() -> HitRegistry {
    let root = view(
        &Model::new(SIZE).with_tab(magistr_cockpit::cockpit::StageTab::Turn),
        &FrameClock::new(0, Duration::ZERO),
    );
    let resolved = LayoutTree::new()
        .compute(&root, SIZE.cols, SIZE.rows)
        .expect("layout");
    let mut hits = HitRegistry::new();
    hits.register_layout(&resolved, 0);
    hits
}

#[tokio::test]
async fn fifty_keys_and_twenty_clicks_reach_the_model_in_order_with_the_right_targets() {
    let keys: String = (0..50)
        .map(|i| char::from(b'a' + u8::try_from(i % 16).expect("small")))
        .collect();
    let mut script = Script::new()
        .startup_replies(MS(0))
        .text(MS(10), &keys[..25]);
    for (i, &(col, row)) in CLICKS.iter().enumerate() {
        script = script.click(MS(30 + 2 * i as u64), col, row);
    }
    // The keys went into the prompt line; ctrl-U clears it so ctrl-D quits.
    let script = script.text(MS(90), &keys[25..]).text(MS(150), "\x15\x04");
    let (model, capture) = run(script).await;

    // Every input arrived: 50 keys + 20 × (press, release) + ctrl-U + ctrl-D.
    assert_eq!(model.inputs(), 50 + 40 + 2);
    assert!(model.quit_requested());

    // The ribbon holds the last inputs exactly as a replay through
    // `update` with the hit registry's own targets would.
    let hits = registry();
    let mut expected = Model::new(SIZE).with_tab(magistr_cockpit::cockpit::StageTab::Turn);
    for c in keys[..25].chars() {
        update(&mut expected, Msg::Key(KeyEvent::new(KeyCode::Char(c))));
    }
    for &(col, row) in &CLICKS {
        let target = hits.hit(col, row);
        assert!(target.is_some(), "({col},{row}) hits a widget");
        for kind in [MouseKind::Down(Button::Left), MouseKind::Up(Button::Left)] {
            update(
                &mut expected,
                Msg::Mouse {
                    event: MouseEvent::new(kind, col, row),
                    target,
                },
            );
        }
    }
    for c in keys[25..].chars() {
        update(&mut expected, Msg::Key(KeyEvent::new(KeyCode::Char(c))));
    }
    update(
        &mut expected,
        Msg::Key(KeyEvent::new(KeyCode::Char('u')).with(Modifiers::CTRL)),
    );
    update(
        &mut expected,
        Msg::Key(KeyEvent::new(KeyCode::Char('d')).with(Modifiers::CTRL)),
    );
    let got: Vec<&Msg> = model.recent().collect();
    let want: Vec<&Msg> = expected.recent().collect();
    assert_eq!(got, want);
    assert_eq!(got.len(), 8);

    // The clicks resolved to every kind of widget, not just one.
    let mut targets: Vec<Widget> = CLICKS
        .iter()
        .filter_map(|&(c, r)| hits.hit(c, r).and_then(Widget::from_id))
        .collect();
    targets.dedup();
    assert!(targets.len() >= 7, "{targets:?}");

    // The startup replies reached the model.
    assert!(model.reports_key_releases());
    assert_eq!(model.cell_pixels(), Some((10, 20)));
    assert!(model.background().is_some());

    // The stream: enter first, at least one frame, the leave sequence last.
    let bytes = capture.bytes();
    assert!(bytes.starts_with(b"\x1b[?1049h"));
    assert!(capture.count(BSU) >= 1);
    assert_eq!(capture.count(BSU) as u64, model.frames());
    assert!(
        bytes.ends_with(&leave_sequence()),
        "ends with the leave sequence"
    );
}

#[tokio::test]
async fn a_burst_of_a_hundred_keys_costs_at_most_two_frames() {
    let burst: String = "abcdefghij".repeat(10);
    let (model, capture) = run(Script::new()
        .startup_replies(MS(0))
        .text(MS(30), &burst)
        .text(MS(200), "\x15\x04"))
    .await;
    assert_eq!(model.inputs(), 102, "the burst, ctrl-U and q");
    let frames = model.frames();
    assert!(frames >= 1, "the first frame is painted");
    assert!(
        frames <= 3,
        "{frames} frames: the initial one plus at most two for the burst"
    );
    assert_eq!(capture.count(BSU) as u64, frames);
}

#[tokio::test]
async fn an_idle_cockpit_paints_nothing() {
    let (model, capture) = run(Script::new()
        .startup_replies(MS(0))
        .text(MS(30), "k")
        .text(MS(1030), "\x15\x04"))
    .await;
    // A full second with one key: the first frame and one for the key,
    // and nothing at all in between. Anything painting on a clock would
    // have added a couple of dozen more.
    assert!(model.frames() <= 2, "{} frames", model.frames());
    assert_eq!(model.ticks(), 0, "nothing animates, so nothing ticks");
    assert_eq!(capture.count(BSU) as u64, model.frames());
}

/// The idle guarantee, on the one surface that grew a periodic
/// animation: the Roadmap tab with a chunk in progress. The glow's wave
/// is a pure function of the tick, but it never *declares* one — so with
/// no focus change and nothing else moving, a full second on the tab
/// costs the same nothing an idle Turn tab does.
#[tokio::test]
async fn an_idle_roadmap_tab_paints_nothing_while_its_glow_holds() {
    let (model, capture) = run(Script::new()
        .startup_replies(MS(0))
        // Ctrl-Tab from Turn past Changes and Trace to the tech-tree.
        .text(MS(30), "\x1b[9;5u\x1b[9;5u")
        .text(MS(1030), "\x15\x04"))
    .await;
    assert_eq!(
        model.tab(),
        magistr_cockpit::cockpit::StageTab::Roadmap,
        "three tabs land on the tech-tree"
    );
    // The first frame, the tab switches, and the transition they started
    // — then a whole quiet second with the glow simply held where it was.
    let transition = u128::from(magistr_cockpit::cockpit::Motion::TRANSITION_STEPS)
        * Model::TICK_PERIOD.as_millis();
    let budget =
        3 + transition.div_ceil(magistr_tui::runtime::Tier::Interactive.period().as_millis());
    assert!(
        u128::from(model.frames()) <= budget,
        "{} frames",
        model.frames()
    );
    let writes = capture.writes();
    let start = writes[0].0;
    assert!(
        !writes
            .iter()
            .any(|(at, _)| (start + MS(500)..start + MS(1000)).contains(at)),
        "the settled roadmap produces no writes"
    );
    assert!(
        model.ticks() <= magistr_cockpit::cockpit::Motion::TRANSITION_STEPS + 1,
        "{} steps: the tab's own pulse and nothing more",
        model.ticks()
    );
    assert_eq!(capture.count(BSU) as u64, model.frames());
}

/// The other half of the same guarantee: a
/// cockpit that IS animating paints at the rate the core asked for —
/// once per `Model::TICK_PERIOD` — without a single message arriving
/// after the one that started the turn.
#[tokio::test]
async fn a_spinning_cockpit_paints_at_the_rate_the_core_asked_for() {
    let quiet = MS(960);
    let start = MS(30);
    let (model, capture) = run_with(
        Script::new()
            .startup_replies(MS(0))
            .text(start + quiet, "\x04"),
        |inbox| {
            FakeAgent::new().text(start, "thinking").play(inbox);
        },
    )
    .await;

    // One agent event, then silence — every frame after it is the
    // spinner's own.
    assert_eq!(model.agent_events(), 1);
    let want = quiet.as_millis() / Model::TICK_PERIOD.as_millis();
    let ticks = u128::from(model.ticks());
    assert!(
        (want / 2..=want + 2).contains(&ticks),
        "{ticks} steps over {quiet:?} at {:?} a step (wanted about {want})",
        Model::TICK_PERIOD
    );
    assert!(
        model.frames() >= model.ticks(),
        "every step is shown: {} frames for {} steps",
        model.frames(),
        model.ticks()
    );
    assert_eq!(capture.count(BSU) as u64, model.frames());
}

/// Real wheel bytes must produce intermediate pixel frames between the model's
/// 60ms ticks. Run in release so debug rasterization cannot mask the cadence.
#[tokio::test]
#[ignore = "release-only pixel scrolling and idle budget"]
#[allow(
    clippy::assertions_on_constants,
    reason = "this ignored timing test must reject an explicit debug-mode run"
)]
async fn pixel_scroll_samples_between_ticks_and_stops_after_settling() {
    use magistr_application::sessions::{AgentEvent, TurnEnd};
    use magistr_cockpit::theme::{MotionMode, Theme};
    assert!(!cfg!(debug_assertions), "run with --release");
    for (size, motion) in [
        (Size::new(144, 50), MotionMode::Full),
        (Size::new(128, 40), MotionMode::Full),
        (Size::new(128, 40), MotionMode::Reduced),
    ] {
        let mut model = Model::new(size).with_theme(Theme::default().with_motion(motion));
        let prose = (0..100)
            .map(|n| {
                format!("Harbor delivery {n}: Keep each customer's retry budget separate.\n\n")
            })
            .collect::<String>();
        update(&mut model, Msg::Agent(AgentEvent::Text(prose)));
        update(
            &mut model,
            Msg::Agent(AgentEvent::TurnEnded(TurnEnd::EndTurn)),
        );
        let terminal = FakeTerminal::new(
            size,
            Script::new()
                .startup_replies(MS(0))
                .scroll_up(MS(800), 20, 10)
                .scroll_up(MS(840), 20, 10)
                .scroll_up(MS(1500), 20, 10)
                .close(MS(2200)),
        );
        let capture = terminal.capture();
        let model = Runtime::new(terminal, options())
            .run(model)
            .await
            .expect("scroll");
        assert!(
            !model.viewport().is_following(),
            "wheel reached the conversation"
        );
        let writes = capture.writes();
        let started = writes[0].0;
        let bytes = capture.bytes();
        let mut offset = 0;
        let mut scroll_frames = Vec::new();
        let mut restarted_frames = Vec::new();
        let mut idle_uploads = 0;
        for (at, len) in writes {
            let frame = &bytes[offset..offset + len];
            offset += len;
            let uploads = frame.windows(6).any(|part| part == b"\x1b_Ga=t");
            if uploads && (started + MS(800)..started + MS(1000)).contains(&at) {
                scroll_frames.push(at);
            }
            if uploads && (started + MS(1500)..started + MS(1700)).contains(&at) {
                restarted_frames.push(at);
            }
            if uploads
                && ((started + MS(1200)..started + MS(1400)).contains(&at)
                    || (started + MS(1850)..started + MS(2100)).contains(&at))
            {
                idle_uploads += 1;
            }
        }
        assert_eq!(idle_uploads, 0, "settled scroll uploads nothing");
        assert_eq!(
            model.now().fract(),
            0.0,
            "a finished gesture leaves a whole model tick"
        );
        if motion.is_full() {
            assert!(
                restarted_frames.len() >= 5,
                "a later gesture also samples smoothly"
            );
            assert!(
                scroll_frames.len() >= 5,
                "intermediate positions: {scroll_frames:?}"
            );
            let mut gaps = scroll_frames
                .windows(2)
                .map(|pair| pair[1].duration_since(pair[0]))
                .collect::<Vec<_>>();
            gaps.sort();
            let median = gaps[gaps.len() / 2];
            eprintln!(
                "{}×{} scroll: {} changed frames, median gap {median:?}",
                size.cols * 10,
                size.rows * 20,
                scroll_frames.len()
            );
            assert!(
                median < MS(30),
                "scroll remains above 33 changed frames/s: {median:?}"
            );
        } else {
            assert_eq!(
                scroll_frames.len(),
                2,
                "reduced motion jumps once for each wheel event"
            );
            assert_eq!(restarted_frames.len(), 1);
            assert_eq!(model.ticks(), 0);
        }
    }
}

/// The screen after the first `count` writes.
fn screen_after_writes(capture: &Capture, count: usize) -> String {
    let bytes = capture.bytes();
    let end: usize = capture
        .writes()
        .iter()
        .take(count)
        .map(|(_, len)| len)
        .sum();
    let mut parser = vt100::Parser::new(SIZE.rows, SIZE.cols, 0);
    parser.process(&bytes[..end]);
    parser.screen().contents()
}

/// A message that reached the inbox before a tick is in the frame that
/// tick paints — the one-frame promise, sampled against a machine slower
/// than the tick. The loop's `select!` polls its branches in a random
/// order; a message that arrives while a frame is being written is in
/// the channel when the loop comes back, and meets a tick that is
/// already due — and without the drain ahead of the paint the tick wins
/// about half the time and the next frame goes out without the message.
/// The 70 ms write against the 60 ms spinner tick makes nearly every
/// arrival that case, the way a loaded machine did for the permission
/// strip in `session_harness`; forty words make the coin flip a
/// certainty (20 of 40 missed with the drain removed). The attribution
/// is exact, not a timestamp tolerance: a word is asserted only when its
/// stamp falls inside a write's span, and then in the frame right after
/// that write. Multi-threaded so the agent's timeline can post while
/// the loop's thread is inside the write.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_message_already_in_the_inbox_is_in_the_frame_the_tick_paints() {
    let arrivals = Arrivals::new();
    let start = MS(30);
    let words: Vec<String> = (0..40).map(|i| format!("w{i:02} ")).collect();
    let last = start + MS(65 * 41) + MS(400);
    let terminal = FakeTerminal::new(
        SIZE,
        Script::new().startup_replies(MS(0)).text(last, "\x04"),
    )
    .slow_writes(MS(70));
    let capture = terminal.capture();
    // The VT oracle observes the fallback; graphics latency has its own wire test.
    let runtime = Runtime::new(
        terminal,
        Options::new(Env::new("xterm-kitty", false, Some("text"))).probe_timeout(MS(500)),
    );
    let mut agent = FakeAgent::new().text(start, "thinking ");
    for (i, word) in words.iter().enumerate() {
        agent = agent.text(start + MS(65 * (i as u64 + 1)), word);
    }
    agent.play(runtime.inbox().stamped(arrivals.clone()));
    let model = runtime
        .run(Model::new(SIZE).with_tab(magistr_cockpit::cockpit::StageTab::Turn))
        .await
        .expect("the run succeeds");
    assert_eq!(model.agent_events(), 41);
    let texts = arrivals.texts();
    assert_eq!(texts.len(), 41);
    let spans = capture.spans();
    let mut sampled = 0;
    let mut late = Vec::new();
    for (when, text) in &texts[1..] {
        // The write in flight when the word arrived, if any: the word was
        // queued before the loop returned from it.
        let Some(during) = spans
            .iter()
            .position(|(began, ended, _)| began <= when && when <= ended)
        else {
            continue;
        };
        if during + 1 >= spans.len() {
            continue;
        }
        sampled += 1;
        if !screen_after_writes(&capture, during + 2).contains(text.trim_end()) {
            late.push(text.trim_end().to_owned());
        }
    }
    assert!(
        sampled >= 25,
        "{sampled} of 40 words arrived during a write"
    );
    assert!(
        late.is_empty(),
        "{} of {sampled} words already in the inbox missed the next frame: {late:?}",
        late.len()
    );
}

/// And it stops: the turn ends, `Cmd::Still` takes the tick back, and the
/// second that follows costs nothing.
#[tokio::test]
async fn the_tick_stops_with_the_turn_and_the_quiet_second_after_is_free() {
    let (model, capture) = run_with(
        Script::new().startup_replies(MS(0)).text(MS(1200), "\x04"),
        |inbox| {
            FakeAgent::new()
                .text(MS(30), "thinking")
                .turn_ended(MS(200))
                .play(inbox);
        },
    )
    .await;

    assert_eq!(model.agent_events(), 2, "the delta and the turn end");
    // 170 ms of streaming is one spinner step; the last row's arrival
    // fades for three more and the header's activity monitor
    // draws itself in over six, which is the
    // longest of them and so the one that sets the tail. That sweep is
    // a once-per-session arrival — past the slot's width the ribbon
    // stops asking for one — and the quiet second after it adds none,
    // which is what this test is really here to pin.
    assert!(model.ticks() <= 9, "{} steps", model.ticks());
    // Pixel frames sample between the same bounded model ticks. After the
    // final sweep the runtime must park completely, including empty frames.
    let budget = 3
        + (9 * Model::TICK_PERIOD.as_millis())
            .div_ceil(magistr_tui::runtime::Tier::Interactive.period().as_millis());
    assert!(
        u128::from(model.frames()) <= budget,
        "{} frames",
        model.frames()
    );
    let writes = capture.writes();
    let start = writes[0].0;
    assert!(
        !writes
            .iter()
            .any(|(at, _)| (start + MS(800)..start + MS(1150)).contains(at)),
        "the ended turn produces no idle writes"
    );
    assert_eq!(capture.count(BSU) as u64, model.frames());
}

/// With no producer updates, graphics still shows active work while the
/// plain-terminal fallback retains its clock-only budget.
#[tokio::test]
async fn quiet_work_animates_on_graphics_and_keeps_plain_terminals_at_clock_rate() {
    let quiet = MS(1500);
    for graphics in [false, true] {
        let startup = if graphics {
            Script::new().startup_replies(MS(0))
        } else {
            Script::new().plain_replies(MS(0))
        };
        let (model, capture) = run_with(startup.text(MS(30) + quiet, "\x04"), |inbox| {
            tokio::spawn(async move {
                tokio::time::sleep(MS(30)).await;
                inbox.send(Msg::Loop(LoopEvent::Started {
                    iteration: 1,
                    max: 4,
                }));
            });
        })
        .await;
        assert_eq!(model.has_pixels(), graphics);
        let period = if graphics {
            Model::TICK_PERIOD
        } else {
            Animation::Clock.period()
        };
        let want = quiet.as_millis() / period.as_millis();
        let ticks = u128::from(model.ticks());
        assert!(
            (want / 2..=want + 2).contains(&ticks),
            "{ticks} ticks over {quiet:?} at {period:?}, graphics={graphics}"
        );
        assert_eq!(model.has_activity_motion(), graphics);
        assert_eq!(capture.count(BSU) as u64, model.frames());
    }
}

#[tokio::test]
async fn a_resize_storm_is_drained_to_one_layout() {
    let mut script = Script::new().startup_replies(MS(0));
    for i in 0..50 {
        let size = if i % 2 == 0 {
            Size::new(100 + i, 30)
        } else {
            Size::new(60, 20 + i)
        };
        script = script.resize(MS(30), size);
    }
    let script = script.text(MS(300), "\x04");
    let (model, capture) = run(script).await;
    assert_eq!(model.size(), Size::new(60, 20 + 49));
    let frames = model.frames();
    assert!(
        frames <= 3,
        "{frames} frames: the initial one plus at most two for the storm"
    );
    // Every frame after a resize clears; the storm produced one or two.
    assert!(capture.count(CLEAR) <= 3, "{} clears", capture.count(CLEAR));
    let bytes = capture.bytes();
    assert!(bytes.ends_with(&leave_sequence()));
}

#[tokio::test]
async fn a_closed_input_ends_the_run_and_a_plain_terminal_gets_no_graphics() {
    let (model, capture) = run(Script::new()
        .plain_replies(MS(0))
        .text(MS(20), "x")
        .close(MS(60)))
    .await;
    assert!(!model.quit_requested());
    assert_eq!(model.inputs(), 1);
    assert!(!model.reports_key_releases());
    let bytes = capture.bytes();
    assert!(bytes.ends_with(&leave_sequence()));
    assert_eq!(
        capture.count(b"\x1b_G"),
        1,
        "only the probe's query is a kitty command"
    );
}

#[tokio::test]
async fn a_terminal_that_never_answers_still_starts_after_the_probe_timeout() {
    let began = std::time::Instant::now();
    let (model, capture) = run(Script::new().text(MS(600), "\x04")).await;
    assert!(began.elapsed() >= MS(500));
    assert!(model.quit_requested());
    assert!(capture.count(BSU) >= 1);
}

/// A run whose terminal is on the OSC 9;4 allowlist, with
/// `producers` on the inbox — every other harness leaves progress off,
/// which is what keeps their captures byte-exact.
async fn run_reporting_progress(script: Script, producers: impl FnOnce(Inbox)) -> (Model, Capture) {
    let terminal = FakeTerminal::new(SIZE, script);
    let capture = terminal.capture();
    let options = Options::new(Env::new("xterm-ghostty", false, None).with_progress(true))
        .probe_timeout(MS(500));
    let runtime = Runtime::new(terminal, options);
    producers(runtime.inbox());
    let model = runtime
        .run(Model::new(SIZE).with_tab(magistr_cockpit::cockpit::StageTab::Turn))
        .await
        .expect("the run succeeds");
    (model, capture)
}

/// Every OSC 9;4 in `bytes`, in order, as text.
fn progress_reports(bytes: &[u8]) -> Vec<String> {
    let text = String::from_utf8_lossy(bytes).into_owned();
    text.match_indices("\x1b]9;4;")
        .filter_map(|(at, _)| {
            let rest = &text[at..];
            rest.find("\x1b\\").map(|end| rest[..end].to_owned())
        })
        .collect()
}

/// Post the loop's whole arc on `inbox`, one step every 40 ms: the
/// iteration starts, a tick of elapsed time that changes nothing, a gate
/// runs, the gate fails, the loop ends.
fn loop_arc(inbox: Inbox) {
    tokio::spawn(async move {
        let steps = [
            Msg::Loop(LoopEvent::Started {
                iteration: 1,
                max: 4,
            }),
            Msg::Loop(LoopEvent::Elapsed(MS(1000))),
            Msg::Loop(LoopEvent::GateStarted {
                name: "check".to_owned(),
                job: JobId::new(1),
            }),
            Msg::Job {
                id: JobId::new(1),
                output: JobOutput::Failed("gate red".to_owned()),
            },
            Msg::Loop(LoopEvent::Ended("aborted".to_owned())),
        ];
        for step in steps {
            tokio::time::sleep(MS(40)).await;
            if !inbox.send(step) {
                return;
            }
        }
    });
}

#[tokio::test]
async fn the_loops_progress_reaches_the_terminal_once_per_change_of_state() {
    let script = Script::new().startup_replies(MS(0)).text(MS(300), "\x04");
    let (model, capture) = run_reporting_progress(script, loop_arc).await;
    assert!(model.quit_requested());
    let bytes = capture.bytes();
    assert_eq!(
        progress_reports(&bytes),
        vec![
            "\x1b]9;4;1;25", // iteration 1 of 4
            "\x1b]9;4;3;0",  // the gate runs: no fraction to give
            "\x1b]9;4;2;25", // the gate failed: the same fraction, in error
            "\x1b]9;4;0;0",  // the loop ended
            "\x1b]9;4;0;0",  // and leave clears it again
        ],
        "the elapsed tick changed no state, so it said nothing"
    );
    let text = String::from_utf8_lossy(&bytes);
    for (at, _) in text.match_indices("\x1b]9;4;1;25") {
        let before = &text[..at];
        assert!(
            before.rfind("\x1b[?2026h") > before.rfind("\x1b[?2026l"),
            "the report rides inside the frame's synchronized bracket"
        );
    }
    assert!(bytes.ends_with(&leave_sequence_with_progress()));
}

#[tokio::test]
async fn a_terminal_off_the_allowlist_is_never_told_about_the_loop() {
    let script = Script::new().startup_replies(MS(0)).text(MS(300), "\x04");
    let (model, capture) = run_with(script, loop_arc).await;
    assert!(model.loop_status().is_some(), "the loop still ran");
    assert_eq!(
        capture.count(b"\x1b]9;4;"),
        0,
        "a terminal that would print the string hears nothing"
    );
    assert!(capture.bytes().ends_with(&leave_sequence()));
}

fn leave_sequence_with_progress() -> Vec<u8> {
    let mut out = Vec::new();
    Session::cockpit().with_progress(true).leave(&mut out);
    out
}

/// A wide graphics screen keeps recorded work beside the conversation.
const WIDE: Size = Size::new(180, 50);

/// Actual terminal clicks open Records; scrolling there leaves the conversation anchored.
#[tokio::test]
async fn opening_and_scrolling_records_keeps_the_conversation_in_place() {
    let baseline = Model::new(WIDE);
    let layout = magistr_tui::screenshot::layout(
        &baseline,
        &FrameClock::new(0, Duration::ZERO),
        magistr_tui::screenshot::Profile::Kitty,
    )
    .unwrap();
    let records = layout.px(Widget::TabSession.id()).expect("Records control");
    let col = ((records.x + records.width * 0.5) / 10.0) as u16;
    let row = ((records.y + records.height * 0.5) / 20.0) as u16;
    let script = Script::new()
        .startup_replies(MS(0))
        // Well past the arrival transients the seed sets off, so the
        // cockpit is idle when the notch lands — and well past the
        // notch's own glide before the quit, so the model ends on the
        // window the scroll landed on rather than a frame still easing
        // towards it.
        .click(MS(300), col, row)
        .scroll_up(MS(500), 150, 20)
        .text(MS(1000), "\x04");
    let terminal = FakeTerminal::new(WIDE, script);
    let runtime = Runtime::new(terminal, options());
    let inbox = runtime.inbox();
    let steps = (1..=60)
        .map(|n| {
            magistr_cockpit::harness::fixtures::step_from(
                serde_json::json!({"step_number": n, "purpose": "open"}),
            )
        })
        .collect();
    inbox.send(Msg::Engine(magistr_cockpit::engine::EngineEvent::Seed(
        magistr_engine::NativeSnapshot {
            steps,
            ..magistr_engine::NativeSnapshot::default()
        },
    )));
    let lines: String = (0..60).map(|n| format!("line {n}\n")).collect();
    inbox.send(Msg::Agent(magistr_application::sessions::AgentEvent::Text(
        lines,
    )));
    inbox.send(Msg::Agent(
        magistr_application::sessions::AgentEvent::TurnEnded(
            magistr_application::sessions::TurnEnd::EndTurn,
        ),
    ));
    let model = runtime
        .run(Model::new(WIDE))
        .await
        .expect("the run succeeds");

    assert_eq!(
        model.undock_rect(),
        None,
        "no extra permanent Session column"
    );
    assert_eq!(model.undocked_tab(), None);
    assert_eq!(model.tab(), magistr_cockpit::cockpit::StageTab::Session);
    assert_eq!(model.inputs(), 4, "the click press/release, wheel and quit");
    let (total, height) = model.session_window();
    assert!(total > height, "the feed is longer than the column");
    assert_eq!(
        model.session_top(),
        total - height - 3,
        "the feed took the notch"
    );
    assert!(
        model.viewport().is_following(),
        "and the turn never moved off its tail"
    );
}
