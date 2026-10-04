//! The terminal golden suite: the cockpit's scenes rasterized
//! *through* the terminal renderer — every `shot_*` still under
//! `tests/golden/screens`, the kitty-profile stills beside them, and every
//! `shot_motion_*` filmstrip under `tests/golden/motion`.
//!
//! A property of the pair — the cockpit *and* the terminal renderer —
//! which is why it lives in this crate's integration tests and not beside
//! `view.rs` in magistr-cockpit: the scenes come from
//! [`magistr_cockpit::harness::Scenes`], the pixels from
//! [`magistr_tui::screenshot`]. The native stills are the same discipline
//! in magistr-gui2's own tree. Re-bless an intended change with
//! `MAGISTR_GOLDEN_UPDATE=1 cargo nextest run -p magistr-tui -E 'test(shot_)'`.

use std::path::Path;
use std::time::Duration;

use magistr_application::sessions::AgentEvent;
use magistr_cockpit::cockpit::view::SPARK_COLS;
use magistr_cockpit::cockpit::{
    Eased, FrameClock, Model, Motion, Msg, Size, StageTab, update, view,
};
use magistr_cockpit::golden::Golden;
use magistr_cockpit::harness::Scenes;
use magistr_cockpit::input::{KeyCode, KeyEvent};
use magistr_cockpit::layout::{LayoutTree, Node, Resolved};
use magistr_cockpit::render::{Buffer, paint};
use magistr_cockpit::theme::{Theme, ThemeMode};
use magistr_cockpit::turn::Viewport;

/// This crate's goldens of `kind`: `tests/golden/<kind>` beside the
/// manifest. The stills and the filmstrips live apart because a strip is
/// many frames wide; the gate's sweep for unreviewed candidates covers
/// the whole `tests/golden` tree, so both are refused alike.
fn golden(kind: &str) -> Golden {
    Golden::new(
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests")
            .join("golden")
            .join(kind),
    )
}

fn screens() -> Golden {
    golden("screens")
}

fn motion() -> Golden {
    golden("motion")
}

#[test]
fn shot_pixel_composer() {
    use magistr_tui::screenshot::Profile;
    type Scene = (&'static str, fn(Size) -> Model);
    let scenes: [Scene; 4] = [
        ("composer-empty", Model::new),
        ("composer-draft", |size| {
            let mut model = Model::new(size);
            update(
                &mut model,
                Msg::Paste(
                    "Please simplify the retry flow.\nKeep the existing worker recovery tests."
                        .into(),
                ),
            );
            model
        }),
        ("composer-long", |size| {
            let mut model = Model::new(size);
            update(
                &mut model,
                Msg::Paste("A long message with café, 漢字 and e\u{301}. ".repeat(30)),
            );
            model
        }),
        ("composer-streaming", Scenes::streaming),
    ];
    let failures: Vec<_> = scenes
        .into_iter()
        .flat_map(|(name, make)| render_shots(name, make, Profile::Kitty, true))
        .collect();
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

fn clock() -> FrameClock {
    FrameClock::new(0, Duration::ZERO)
}
#[test]
fn shot_rich_selected_record() {
    shots_on(
        "record",
        Scenes::rich_record,
        magistr_tui::screenshot::Profile::Cells,
        false,
    );
    shots_on(
        "record",
        Scenes::rich_record,
        magistr_tui::screenshot::Profile::Kitty,
        false,
    );
}

fn layout(root: &Node, size: Size) -> Resolved {
    LayoutTree::new()
        .compute(root, size.cols, size.rows)
        .expect("layout succeeds")
}

fn agent(m: &mut Model, event: AgentEvent) {
    update(m, Msg::Agent(event));
}

/// `make`'s surface as four goldens — Mocha and Latte at 80x24 and
/// 120x40 — checked against tests/golden/screens. Every mismatch is
/// reported, not just the first, so one run names every image to look
/// at.
fn shots(surface: &str, make: fn(Size) -> Model) {
    shots_on(surface, make, magistr_tui::screenshot::Profile::Cells, true);
}

/// [`shots`] of a scene the harness must not settle further: one the
/// ticks would carry past what it shows — a kill being held completes
/// on them — and which has settled itself to the step it means.
fn shots_held(surface: &str, make: fn(Size) -> Model) {
    shots_on(
        surface,
        make,
        magistr_tui::screenshot::Profile::Cells,
        false,
    );
}

/// [`shots`] on a terminal matching `profile`. The kitty profile's
/// stills carry `-kitty` and sit beside the cell ones: both are shipped
/// paths, and a still of one says nothing about the other.
fn shots_on(
    surface: &str,
    make: fn(Size) -> Model,
    profile: magistr_tui::screenshot::Profile,
    settle: bool,
) {
    let failures = render_shots(surface, make, profile, settle);
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

/// Collect every candidate in a related group before reporting failures.
fn render_shots(
    surface: &str,
    make: fn(Size) -> Model,
    profile: magistr_tui::screenshot::Profile,
    settle: bool,
) -> Vec<String> {
    use magistr_tui::screenshot::frame;
    let golden = screens();
    let mut failures = Vec::new();
    for (mode, theme) in [("mocha", ThemeMode::Dark), ("latte", ThemeMode::Light)] {
        let sizes: &[(u16, u16)] = if surface.starts_with("inspector-") {
            &[(80, 24), (100, 30), (120, 40), (132, 42)]
        } else {
            &[(80, 24), (120, 40)]
        };
        for &(cols, rows) in sizes {
            let mut model = make(Size::new(cols, rows)).with_theme(Theme::forced(theme));
            if profile == magistr_tui::screenshot::Profile::Kitty {
                use magistr_cockpit::cockpit::Widget;
                use magistr_cockpit::input::{Button, MouseEvent, MouseKind};
                if matches!(surface, "launcher" | "command-palette")
                    || surface.starts_with("inspector-")
                    || surface == "reasoning-expanded"
                {
                    // These scenes exercise startup and shared conversation details.
                    // The separate roadmap scene exercises the complete plan page.
                    model = model.with_tab(StageTab::Turn);
                }
                update(
                    &mut model,
                    Msg::Mode(magistr_cockpit::capability::Mode::Graphics {
                        pointer_shapes: true,
                    }),
                );
                // Selecting a view or details happens after capability detection in the app.
                let target = match surface {
                    "roadmap" => Some(Widget::TabRoadmap.id()),
                    "trace" => Some(Widget::TabSession.id()),
                    "reasoning-expanded" => Some(Widget::ReasoningToggle.id()),
                    name if name.starts_with("inspector-") => Some(Widget::InspectorFields.id()),
                    _ => None,
                };
                if let Some(target) = target {
                    update(
                        &mut model,
                        Msg::Mouse {
                            event: MouseEvent::new(MouseKind::Down(Button::Left), 0, 0),
                            target: Some(target),
                        },
                    );
                    update(
                        &mut model,
                        Msg::Mouse {
                            event: MouseEvent::new(MouseKind::Up(Button::Left), 0, 0),
                            target: Some(target),
                        },
                    );
                }
            }
            if settle {
                Scenes::settle(&mut model);
            }
            let pixmap = frame(
                &model,
                &FrameClock::new(7, Duration::from_millis(1500)),
                profile,
            )
            .expect("layout succeeds");
            let name = format!("{surface}-{mode}-{cols}x{rows}{}", profile.suffix());
            if let Err(error) = golden.check(&name, &pixmap) {
                failures.push(error.to_string());
            }
        }
    }
    failures
}

fn inspected(mut model: Model, target: magistr_cockpit::WidgetId) -> Model {
    use magistr_cockpit::input::{Button, MouseEvent, MouseKind};
    update(
        &mut model,
        Msg::Mouse {
            event: MouseEvent::new(MouseKind::Down(Button::Left), 0, 0),
            target: Some(target),
        },
    );
    update(&mut model, Msg::Key(KeyEvent::new(KeyCode::Enter)));
    model
}

#[test]
fn shot_inspector_objects() {
    use magistr_cockpit::cockpit::Widget;
    use magistr_tui::screenshot::Profile;
    type Scene = (&'static str, fn(Size) -> Model);
    let scenes: [Scene; 6] = [
        ("inspector-tool", Scenes::terminal_pane),
        ("inspector-edit", Scenes::inspected_edit),
        ("inspector-chunk", |size| {
            inspected(Scenes::roadmapped(size), Widget::roadmap_row(1))
        }),
        ("inspector-signal", |size| {
            inspected(Scenes::roadmapped(size), Widget::signal_row(0))
        }),
        ("inspector-step", |size| {
            inspected(Scenes::traced(size), Widget::trace_row(0))
        }),
        ("inspector-change", |size| {
            inspected(Scenes::changed(size), Widget::change_row(0))
        }),
    ];
    let mut failures = Vec::new();
    for (name, make) in scenes {
        for profile in [Profile::Cells, Profile::Kitty] {
            failures.extend(render_shots(name, make, profile, true));
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

#[test]
fn shot_turn_and_palette_in_both_profiles() {
    use magistr_tui::screenshot::Profile;
    // Cell-profile Turn scenes have their own tests above.
    let mut failures = render_shots("streaming", Scenes::streaming, Profile::Kitty, true);
    failures.extend(render_shots(
        "terminal-pane",
        Scenes::terminal_pane,
        Profile::Kitty,
        true,
    ));
    failures.extend(render_shots(
        "permission",
        Scenes::permission,
        Profile::Kitty,
        true,
    ));
    for profile in [Profile::Cells, Profile::Kitty] {
        failures.extend(render_shots(
            "command-palette",
            Scenes::command_palette,
            profile,
            true,
        ));
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

#[test]
fn shot_hovered_tab() {
    shots("hover-tab", Scenes::hovered_tab);
}

#[test]
fn shot_hovered_button() {
    shots("hover-button", Scenes::hovered_button);
}

#[test]
fn shot_launcher() {
    shots("launcher", Scenes::launcher);
}

#[test]
fn shot_streaming_turn() {
    shots("streaming", Scenes::streaming);
}

#[test]
fn shot_permission_strip() {
    shots("permission", Scenes::permission);
}

#[test]
fn shot_roadmap() {
    shots("roadmap", Scenes::roadmapped);
}

#[test]
fn shot_changes() {
    shots("changes", Scenes::changed);
}

#[test]
fn shot_terminal_pane() {
    shots("terminal-pane", Scenes::terminal_pane);
}

#[test]
fn shot_terminal_pane_scrolled() {
    shots("terminal-pane-scrolled", Scenes::terminal_pane_scrolled);
}

/// The terminal dock: the running card's screen raised
/// over the stage above the prompt — the grip row naming the command,
/// the docked rows in the program's own colours, the feed's tail kept
/// above it.
#[test]
fn shot_terminal_dock() {
    shots("terminal-dock", Scenes::terminal_dock);
}

/// The held kill: the running card's head filling red
/// from the left while `k` is held, and the pane after the half second
/// — frozen, the strip row saying it was the human.
#[test]
fn shot_terminal_kill() {
    shots_held("terminal-kill", Scenes::terminal_kill);
}

#[test]
fn shot_terminal_killed() {
    shots("terminal-killed", Scenes::terminal_killed);
}

#[test]
fn shot_trace() {
    shots("trace", Scenes::traced);
}

#[test]
fn shot_sheet() {
    shots("sheet", Scenes::sheeted);
}

#[test]
fn shot_launcher_kitty() {
    shots_on(
        "launcher",
        Scenes::launcher,
        magistr_tui::screenshot::Profile::Kitty,
        true,
    );
}

#[test]
fn shot_roadmap_kitty() {
    shots_on(
        "roadmap",
        Scenes::roadmapped,
        magistr_tui::screenshot::Profile::Kitty,
        true,
    );
}

#[test]
fn shot_trace_kitty() {
    shots_on(
        "trace",
        Scenes::traced,
        magistr_tui::screenshot::Profile::Kitty,
        true,
    );
}

#[test]
fn shot_reasoning_expanded() {
    shots_on(
        "reasoning-expanded",
        Scenes::traced,
        magistr_tui::screenshot::Profile::Kitty,
        true,
    );
}

/// All glyphs used by the screenshot scenes have an embedded outline.
/// The DejaVu Sans Mono fallback covers symbols absent from the primary
/// face, preventing missing-glyph boxes in the golden images.
#[test]
fn shot_glyph_gaps_are_the_known_ones() {
    use magistr_tui::screenshot::Typeface;
    let face = Typeface::new();
    let surfaces: [fn(Size) -> Model; 7] = [
        Scenes::launcher,
        Scenes::streaming,
        Scenes::permission,
        Scenes::roadmapped,
        Scenes::changed,
        Scenes::traced,
        Scenes::sheeted,
    ];
    let mut missing = std::collections::BTreeSet::new();
    for make in surfaces {
        let model = make(Size::new(120, 40));
        let root = view(&model, &clock());
        let resolved = layout(&root, model.size());
        let mut buffer = Buffer::new(120, 40);
        paint(&root, &resolved, &mut buffer);
        for cell in buffer.cells() {
            if let Some(ch) = cell.symbol.chars().next()
                && !ch.is_whitespace()
                && !face.has_glyph(ch)
            {
                missing.insert(ch);
            }
        }
    }
    assert_eq!(
        missing,
        std::collections::BTreeSet::new(),
        "glyphs no embedded face can draw"
    );
}

/// The permission surface — the one whose buttons carry the pass and
/// fail roles — under deuteranopia, in both palettes: the CVD golden.
#[test]
fn shot_permission_deuteranopia() {
    use magistr_theme::CvdMode;
    use magistr_tui::screenshot::{Profile, frame};
    let golden = screens();
    let mut failures = Vec::new();
    for (mode, theme) in [("mocha", ThemeMode::Dark), ("latte", ThemeMode::Light)] {
        let mut model = Scenes::permission(Size::new(80, 24))
            .with_theme(Theme::forced(theme).with_cvd(Some(CvdMode::Deuteranopia)));
        Scenes::settle(&mut model);
        let pixmap = frame(
            &model,
            &FrameClock::new(7, Duration::from_millis(1500)),
            Profile::Cells,
        )
        .expect("layout succeeds");
        if let Err(error) = golden.check(&format!("permission-{mode}-deuteranopia-80x24"), &pixmap)
        {
            failures.push(error.to_string());
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

/// A motion as a filmstrip golden: `frames` panels of `make`'s model,
/// one animation step apart, Mocha at 80x24, under tests/golden/motion.
/// Reviewed and re-blessed exactly like a still — a diverging strip
/// writes `<name>.new.png` beside its golden, and the screenshots gate
/// refuses the candidate.
fn strip_shot(name: &str, frames: usize, make: fn() -> Model) {
    use magistr_tui::screenshot::{Profile, strip};
    let mut model = make();
    let pixmap = strip(
        &mut model,
        &FrameClock::new(7, Duration::from_millis(1500)),
        frames,
        Profile::Cells,
    )
    .expect("layout succeeds");
    motion().assert_matches(&format!("{name}-mocha-80x24"), &pixmap);
}

/// The streaming surface, on the tick the shimmer travels with —
/// settled first, so the strip carries the shimmer alone rather than the
/// arrival fade the fixture's last event started.
fn shimmering() -> Model {
    let mut m = Scenes::streaming(Size::new(80, 24)).with_theme(Theme::forced(ThemeMode::Dark));
    Scenes::settle(&mut m);
    m
}

/// The rail's progress bar the step after it was retargeted 2/8 -> 6/8,
/// so panel one is the start of the glide.
fn rail_gliding() -> Model {
    let mut m = Model::new(Size::new(80, 24)).with_theme(Theme::forced(ThemeMode::Dark));
    update(
        &mut m,
        Msg::Engine(magistr_cockpit::engine::EngineEvent::Seed(
            magistr_engine::NativeSnapshot {
                progress: magistr_cockpit::harness::fixtures::progress(2, 8),
                ..magistr_engine::NativeSnapshot::default()
            },
        )),
    );
    for _ in 0..4 {
        update(
            &mut m,
            Msg::Engine(magistr_cockpit::engine::EngineEvent::local(
                magistr_cockpit::harness::fixtures::frame(
                    "roadmap",
                    "chunk_completed",
                    serde_json::json!({"chunk": {"id": "x"}}),
                ),
            )),
        );
    }
    m
}

/// The stage the step after Tab moved it to Changes, so panel one is
/// the pulse at rest.
fn tab_pulsing() -> Model {
    let mut m = Model::new(Size::new(80, 24)).with_theme(Theme::forced(ThemeMode::Dark));
    update(
        &mut m,
        Msg::Key(KeyEvent::new(KeyCode::Tab).with(magistr_cockpit::input::Modifiers::CTRL)),
    );
    m
}

/// The dock the step a permission card arrived on, with the stage
/// settled first — so the strip carries the dock's arrival alone rather
/// than the turn rows fading in beside it.
fn dock_arriving() -> Model {
    let mut m =
        Scenes::permission_turn(Size::new(80, 24)).with_theme(Theme::forced(ThemeMode::Dark));
    Scenes::settle(&mut m);
    Scenes::request_permission(&mut m);
    m
}

/// The turn the step its rows arrived on, so panel one is the dim end of
/// the fade and the last is the stage settled.
fn rows_arriving() -> Model {
    let mut m = Scenes::streaming(Size::new(80, 24)).with_theme(Theme::forced(ThemeMode::Dark));
    // Fill the header's activity ribbon and settle it before the rows
    // land. The monitor's own sweep is a
    // once-per-session arrival, and a strip that carried both motions
    // could keep passing `every_filmstrip_carries_motion_between_its_
    // _panels` on the sparkline alone while the row fade quietly broke.
    // Past the slot's width the ribbon stops asking for a sweep at all,
    // which is the saturation guard doing its job.
    for _ in 0..usize::from(SPARK_COLS) {
        agent(&mut m, AgentEvent::Text(" ".to_owned()));
        update(&mut m, Msg::FrameDue);
    }
    Scenes::settle(&mut m);
    agent(
        &mut m,
        AgentEvent::Text("\nRecovery checks passed.".to_owned()),
    );
    m
}

/// The trace tab the step a page scrolled it, so panel one is the frame
/// held on the way — a cell surface reads the glide a step ahead, so the
/// page's own frame is already most of the way there — and the last is
/// where the page landed. The
/// body glides under the heading, which does not move — the strip is how
/// that is reviewed rather than asserted row by row.
fn trace_gliding() -> Model {
    let mut m = Scenes::long_trace(Size::new(80, 24)).with_theme(Theme::forced(ThemeMode::Dark));
    Scenes::settle(&mut m);
    update(&mut m, Msg::Key(KeyEvent::new(KeyCode::PageUp)));
    m
}

/// The Roadmap tab at the foot of the active chunk's breath, so the
/// strip carries the rise off it: the in-progress row's pen lifts on the
/// same raised cosine the pixel halo swells on. The halo
/// itself cannot be on a strip — the rasterizer has no kitty, so the
/// tech-tree is a legend here and the swell is proven by the painter's
/// own tests.
fn roadmap_glowing() -> Model {
    let mut m = Scenes::roadmapped(Size::new(80, 24)).with_theme(Theme::forced(ThemeMode::Dark));
    Scenes::settle(&mut m);
    m
}

/// The header on the step its activity monitor arrived, so the strip
/// carries the ramp drawing itself in from the left. The pixel sparkline sweeps on the same curve, but the
/// rasterizer has no kitty — so what a strip can review is the cell
/// ramp, and the pixel line is left to `pixel::sparkline`'s own tests.
fn activity_sweeping() -> Model {
    let mut m = Scenes::streaming(Size::new(80, 24)).with_theme(Theme::forced(ThemeMode::Dark));
    Scenes::settle(&mut m);
    // A burst of busy frames between two ticks: the ribbon fills the
    // slot on one step, so the whole width draws in rather than a
    // single new column.
    for n in 1..=usize::from(SPARK_COLS) {
        for _ in 0..n {
            update(&mut m, Msg::Agent(AgentEvent::Text(" ".to_owned())));
        }
        update(&mut m, Msg::FrameDue);
    }
    m
}

/// One filmstrip: its name, how many panels it takes and the model it
/// starts from.
type Filmstrip = (&'static str, usize, fn() -> Model);

/// The motions the cockpit has, by strip name and panel count: a wave
/// long enough to read, the whole of each glide, pulse, pop and fade.
const STRIPS: [Filmstrip; 9] = [
    ("shimmer", 6, shimmering),
    ("rail-fill", (Eased::STEPS + 1) as usize, rail_gliding),
    (
        "tab-pulse",
        (Motion::TRANSITION_STEPS + 1) as usize,
        tab_pulsing,
    ),
    (
        "dock-arrival",
        (Motion::TRANSITION_STEPS + 1) as usize,
        dock_arriving,
    ),
    (
        "row-arrival",
        (Motion::ARRIVAL_STEPS + 1) as usize,
        rows_arriving,
    ),
    (
        "gate-pop",
        (Motion::TRANSITION_STEPS + 1) as usize,
        Scenes::gates_popping,
    ),
    (
        "trace-scroll",
        (Viewport::GLIDE_STEPS + 1) as usize,
        trace_gliding,
    ),
    // The first quarter of the breath — the rise off the foot of the
    // wave, where the change per step is largest. A whole period would
    // be a strip two metres wide for a curve that comes back to where it
    // started.
    (
        "roadmap-glow",
        (Motion::GLOW_STEPS / 4 + 1) as usize,
        roadmap_glowing,
    ),
    (
        "activity-sweep",
        (Motion::SWEEP_STEPS + 1) as usize,
        activity_sweeping,
    ),
];

#[test]
fn shot_motion_shimmer() {
    let (name, frames, make) = STRIPS[0];
    strip_shot(name, frames, make);
}

#[test]
fn shot_motion_rail_fill() {
    let (name, frames, make) = STRIPS[1];
    strip_shot(name, frames, make);
}

#[test]
fn shot_motion_tab_pulse() {
    let (name, frames, make) = STRIPS[2];
    strip_shot(name, frames, make);
}

#[test]
fn shot_motion_dock_arrival() {
    let (name, frames, make) = STRIPS[3];
    strip_shot(name, frames, make);
}

#[test]
fn shot_motion_row_arrival() {
    let (name, frames, make) = STRIPS[4];
    strip_shot(name, frames, make);
}

#[test]
fn shot_motion_gate_pop() {
    let (name, frames, make) = STRIPS[5];
    strip_shot(name, frames, make);
}

#[test]
fn shot_motion_trace_scroll() {
    let (name, frames, make) = STRIPS[6];
    strip_shot(name, frames, make);
}

/// A filmstrip only earns its place if the motion is on it: every strip
/// has panels that differ from its first, so a motion that silently
/// stops moving fails the gate rather than re-blessing to a still row
/// of identical frames.
#[test]
fn shot_motion_roadmap_glow() {
    let (name, frames, make) = STRIPS[7];
    strip_shot(name, frames, make);
}

#[test]
fn shot_motion_activity_sweep() {
    let (name, frames, make) = STRIPS[8];
    strip_shot(name, frames, make);
}

#[test]
fn every_filmstrip_carries_motion_between_its_panels() {
    use magistr_tui::screenshot::{Profile, Typeface, frame, strip};
    let clock = FrameClock::new(7, Duration::from_millis(1500));
    for (name, frames, make) in STRIPS {
        let start = make();
        let panel = frame(&start, &clock, Profile::Cells)
            .expect("layout succeeds")
            .width();
        let mut model = start;
        let strip = strip(&mut model, &clock, frames, Profile::Cells).expect("layout succeeds");
        let width = strip.width();
        let column = |x: u32| {
            (0..strip.height())
                .map(|row| strip.pixels()[(row * width + x) as usize])
                .collect::<Vec<_>>()
        };
        let first: Vec<Vec<_>> = (0..panel).map(column).collect();
        let moved = (1..frames as u32).any(|index| {
            let left = index * (panel + Typeface::CELL_WIDTH);
            (0..panel).any(|x| column(left + x) != first[x as usize])
        });
        assert!(moved, "{name}: every panel of the strip is the same frame");
    }
}

/// Under reduced motion every filmstrip is one panel — the still frame
/// that surface draws — and no step is taken to get it. The strip is
/// how the juice chunks after this one are reviewed, so the reduced
/// path has to collapse structurally rather than by each test
/// remembering to ask.
#[test]
fn every_filmstrip_under_reduced_motion_is_the_still_frame() {
    use magistr_cockpit::theme::MotionMode;
    use magistr_tui::screenshot::{Profile, frame, strip};
    let clock = FrameClock::new(7, Duration::from_millis(1500));
    for (name, frames, make) in STRIPS {
        let reduced =
            make().with_theme(Theme::forced(ThemeMode::Dark).with_motion(MotionMode::Reduced));
        let still = frame(&reduced, &clock, Profile::Cells).expect("layout succeeds");
        let mut model = reduced;
        // Some fixtures settle themselves before the strip starts; what
        // matters is that the strip itself takes no step.
        let before = model.ticks();
        let strip = strip(&mut model, &clock, frames, Profile::Cells).expect("layout succeeds");
        assert_eq!(strip.width(), still.width(), "{name}: one panel wide");
        assert_eq!(strip.data(), still.data(), "{name}: the still frame");
        assert_eq!(model.ticks(), before, "{name}: no step was taken");
    }
}

#[test]
fn shot_loop_track_picker() {
    let failures = render_shots(
        "loop-track-picker",
        Scenes::loop_track_picker,
        magistr_tui::screenshot::Profile::Kitty,
        true,
    );
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

#[test]
fn shot_roadmap_overview() {
    shots_on(
        "roadmap-overview",
        Scenes::roadmap_overview,
        magistr_tui::screenshot::Profile::Kitty,
        true,
    );
}

#[test]
fn shot_roadmap_overview_waiting_and_large_font() {
    use magistr_cockpit::cockpit::Widget;
    use magistr_cockpit::input::{Button, MouseEvent, MouseKind};
    use magistr_cockpit::pixel::CellSize;
    use magistr_tui::screenshot::{Profile, frame};
    let mut failures = Vec::new();
    for (name, theme) in [("mocha", ThemeMode::Dark), ("latte", ThemeMode::Light)] {
        let mut model =
            Scenes::roadmap_overview(Size::new(144, 50)).with_theme(Theme::forced(theme));
        update(
            &mut model,
            Msg::Mode(magistr_cockpit::capability::Mode::Graphics {
                pointer_shapes: true,
            }),
        );
        update(
            &mut model,
            Msg::CellSize(Some(CellSize {
                width: 16,
                height: 33,
            })),
        );
        for _ in 0..3 {
            update(
                &mut model,
                Msg::Mouse {
                    event: MouseEvent::new(MouseKind::ScrollDown, 0, 0),
                    target: Some(Widget::RoadmapOverview.id()),
                },
            );
        }
        let selected = model.roadmap().overview_items()[3].0;
        update(
            &mut model,
            Msg::Mouse {
                event: MouseEvent::new(MouseKind::Down(Button::Left), 0, 0),
                target: Some(Widget::sky_star(selected)),
            },
        );
        Scenes::settle(&mut model);
        let pixmap = frame(&model, &clock(), Profile::Kitty).expect("layout");
        if let Err(error) = screens().check(
            &format!("roadmap-overview-waiting-{name}-144x50-16x33-kitty"),
            &pixmap,
        ) {
            failures.push(error.to_string());
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}
