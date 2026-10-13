use super::*;

use std::path::PathBuf;

use magistr_cockpit::cockpit::{Size, Widget, update};
use magistr_cockpit::golden::Golden;
use magistr_cockpit::input::{Button, MouseEvent, MouseKind};
use magistr_cockpit::theme::{Theme, ThemeMode};
use qs_gpu::frame::PrimKind;

use crate::fixture::Fixture;
use crate::marks::Marks;

/// Tick past the cockpit's arrival window, so a still is of the settled
/// frame rather than of rows still fading in.
fn settle(model: &mut Model) {
    for _ in 0..=magistr_cockpit::cockpit::Motion::ARRIVAL_STEPS {
        let _ = update(model, Msg::Tick);
    }
}

fn clock() -> FrameClock {
    FrameClock::new(0, std::time::Duration::ZERO)
}

/// The native goldens, beside the terminal ones in shape but in this
/// crate's own tree — a still of the GPU painter says nothing about the
/// cell painter and must not be able to re-bless one.
fn golden() -> Golden {
    Golden::new(
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("tests")
            .join("golden")
            .join("native"),
    )
}

/// A mock view's still through the whole frame — the composition root,
/// the element layout, the emitter, the sky and the pictures — at the
/// blessed captures' own size.
fn still_of(view: &str, mode: ThemeMode) -> (Pixmap, Frame, Model) {
    let mut frame = Frame::new().expect("the embedded face yields a grid");
    let cell = frame.metrics();
    let size = Fixture::capture_grid((cell.width(), cell.height()));
    let mut model = Fixture::with_pixels(
        Fixture::mock(view, size)
            .expect("the view has a fixture")
            .with_theme(Theme::forced(mode)),
    );
    let (width, height) = Fixture::CAPTURE;
    let pixmap = frame
        .still(&mut model, &clock(), width, height)
        .expect("the frame renders");
    (pixmap, frame, model)
}

/// The two media every still is rasterized in. Forced rather than read
/// off the environment: a still whose theme came from `MAGISTR_THEME`
/// would be a picture of the machine that took it.
const MEDIA: [(&str, ThemeMode); 2] = [("night", ThemeMode::Dark), ("day", ThemeMode::Light)];

fn shots(view: &str) {
    let mut failures = Vec::new();
    for (medium, mode) in MEDIA {
        let (pixmap, _, _) = still_of(view, mode);
        if std::panic::catch_unwind(|| {
            golden().assert_matches(&format!("view{view}-{medium}"), &pixmap)
        })
        .is_err()
        {
            failures.push(medium);
        }
    }
    assert!(
        failures.is_empty(),
        "review both theme candidates: {failures:?}"
    );
}

#[test]
fn the_ready_label_tracks_its_node_across_window_sizes_and_scales() {
    for (width, height, scale) in [(1100, 640, 1.0), (840, 720, 1.0), (2200, 1280, 2.0)] {
        let mut frame = Frame::new().expect("fonts");
        frame.rescale(scale).expect("display scale");
        let metrics = frame.metrics();
        let size = metrics.grid(width, height);
        let mut model = Fixture::with_pixels(Fixture::birth_f5(size));
        frame
            .still(&mut model, &clock(), width, height)
            .expect("first roadmap");
        let (_, [x, y, _, _]) = frame
            .says()
            .into_iter()
            .find(|(text, _)| text == "harmonic-engine · Ready")
            .expect("ready label");
        assert_eq!(
            frame
                .hits()
                .hit_at(f64::from(x) + 16.0 * scale, f64::from(y) - 12.0 * scale),
            Some(Widget::sky_star(0)),
            "{width}x{height} at {scale}: the label must follow the clickable node",
        );
    }
}

#[test]
fn repeated_rendering_converges_without_advancing_the_model() {
    for view in ["03", "05", "06", "11", "F4"] {
        let mut frame = Frame::new().expect("embedded fonts");
        let cell = frame.metrics();
        let size = Fixture::capture_grid((cell.width(), cell.height()));
        let mut model = Fixture::with_pixels(Fixture::mock(view, size).expect("fixture"))
            .with_surface(Surface::Native);
        let before = model.clone();
        let (width, height) = Fixture::CAPTURE;
        let first = frame
            .still(&mut model, &clock(), width, height)
            .expect("CPU frame");
        assert_eq!(model.now(), before.now(), "{view}: painting advanced time");
        assert_eq!(
            model.phase(),
            before.phase(),
            "{view}: painting changed phase"
        );
        assert_eq!(model.dock(), before.dock());
        assert_eq!(model.place(), before.place());
        assert_eq!(model.roadmap().rows(), before.roadmap().rows());
        let settled = model.clone();
        assert!(
            !frame
                .build(&mut model, &clock(), width, height)
                .expect("settled layout")
        );
        for _ in 0..3 {
            let next = frame
                .still(&mut model, &clock(), width, height)
                .expect("repeat");
            assert_eq!(next.data(), first.data(), "{view}: unstable pixels");
            assert_eq!(model, settled, "{view}: painting changed settled state");
        }
    }
}

// ── The views the composition root is proven on ────────────────────

/// View 05 — the Session: header band, the turn column with its cap
/// and composer, the supervision rail behind its divider, the foot.
#[test]
fn shot_session() {
    shots("05");
}

/// View 03 — the Sky: the constellation drawn into the root's field
/// reservation.
#[test]
fn shot_sky() {
    shots("03");
}

/// View 06 — the Moment: the plan card on its scrim, the rail stood
/// down, one phase word on the foot.
#[test]
fn shot_moment() {
    shots("06");
}

/// View 07 — the review: the change replaces the session under the
/// same altitude path.
#[test]
fn shot_review() {
    shots("07");
}

/// A turn taller than the window, still streaming — the tail flush above the composer,
/// the overflow clipped at the stage's top edge. Prose rows on purpose: the CPU tier
/// scissors glyphs but not tiny-skia surface fills, so a card in the overflow would
/// make the golden a picture of that divergence rather than of the window.
#[test]
fn shot_long_turn() {
    use magistr_application::sessions::AgentEvent;
    for (medium, mode) in MEDIA {
        let mut frame = Frame::new().expect("the embedded face yields a grid");
        let cell = frame.metrics();
        let size = Fixture::capture_grid((cell.width(), cell.height()));
        let mut model = Fixture::with_pixels(Model::new(size).with_theme(Theme::forced(mode)));
        for line in 0..120 {
            let _ = update(
                &mut model,
                Msg::Agent(AgentEvent::Text(format!("the stream's row {line}\n"))),
            );
        }
        settle(&mut model);
        let (width, height) = Fixture::CAPTURE;
        let pixmap = frame
            .still(&mut model, &clock(), width, height)
            .expect("the frame renders");
        golden().assert_matches(&format!("long-turn-{medium}"), &pixmap);
    }
}

/// Long shell commands stay inside their cards. Headers are truncated to the card
/// width without clipping at the stage edge.
#[test]
fn shot_long_commands() {
    use magistr_application::sessions::{AgentEvent, ToolStatus, ToolVerb};
    const COMMANDS: [&str; 5] = [
        "ls crates/magistr-gui2/tests/ && echo \"---\" && ls crates/magistr-gui2/src/ && echo \"---\" && ls scripts/",
        "wc -l crates/magistr-gui2/tests/mock_parity.rs crates/magistr-gui2/src/parity.rs crates/magistr-gui2/src/mask.rs",
        "cat crates/magistr-gui2/src/parity.rs && echo \"=== mask.rs ===\" && cat crates/magistr-gui2/src/mask.rs",
        "sed -n '1,140p' crates/magistr-gui2/src/fixture.rs",
        "./scripts/mock-parity-gate.sh 2>&1 | tail -80",
    ];
    for (medium, mode) in MEDIA {
        let mut frame = Frame::new().expect("the embedded face yields a grid");
        let cell = frame.metrics();
        let size = Fixture::capture_grid((cell.width(), cell.height()));
        let mut model = Fixture::with_pixels(Model::new(size).with_theme(Theme::forced(mode)));
        for (at, command) in COMMANDS.iter().enumerate() {
            let _ = update(
                &mut model,
                Msg::Agent(AgentEvent::ToolCall {
                    id: format!("c{at}"),
                    title: (*command).to_owned(),
                    verb: ToolVerb::Run,
                    status: ToolStatus::Completed,
                    diff: None,
                    images: Vec::new(),
                    record: Default::default(),
                }),
            );
        }
        settle(&mut model);
        let (width, height) = Fixture::CAPTURE;
        let pixmap = frame
            .still(&mut model, &clock(), width, height)
            .expect("the frame renders");
        golden().assert_matches(&format!("long-commands-{medium}"), &pixmap);
    }
}

/// The arrival itself, as a picture: the frame on which a row and its card land, beside
/// the same frame once they have settled. The pair is the fade — a still of the first
/// alone could not tell "faded" from "wrong ink".
#[test]
fn shot_arrival() {
    use magistr_application::sessions::{AgentEvent, ToolStatus, ToolVerb};
    let build = |settled: bool| {
        let mut frame = Frame::new().expect("the embedded face yields a grid");
        let cell = frame.metrics();
        let size = Fixture::capture_grid((cell.width(), cell.height()));
        let mut model =
            Fixture::with_pixels(Model::new(size).with_theme(Theme::forced(ThemeMode::Dark)));
        let _ = update(
            &mut model,
            Msg::Agent(AgentEvent::Text(
                "The row that just landed comes in from the dim pen.\n".to_owned(),
            )),
        );
        let _ = update(
            &mut model,
            Msg::Agent(AgentEvent::ToolCall {
                id: "c0".to_owned(),
                title: "cargo nextest run -p magistr-gui2".to_owned(),
                verb: ToolVerb::Run,
                status: ToolStatus::Completed,
                diff: None,
                images: Vec::new(),
                record: Default::default(),
            }),
        );
        if settled {
            settle(&mut model);
        }
        let (width, height) = Fixture::CAPTURE;
        frame
            .still(&mut model, &clock(), width, height)
            .expect("the frame renders")
    };
    golden().assert_matches("arrival-arriving", &build(false));
    golden().assert_matches("arrival-settled", &build(true));
}

/// The Almagest set's specimen sheet, in both media — a still of the
/// *painter*: every mark went through [`Marks::draw`], the atlas's
/// colour page and the CPU tier, the route a mark on a real surface
/// takes.
#[test]
fn shot_marks() {
    for (medium, mode) in MEDIA {
        let set = Marks::for_theme(&Theme::forced(mode));
        let mut frame = Frame::new().expect("the embedded face yields a grid");
        let pixmap = frame.specimen(&set).expect("the sheet renders");
        let name = match medium {
            "night" => "marks-mocha",
            _ => "marks-latte",
        };
        golden().assert_matches(name, &pixmap);
    }
}

/// The sheet is whole: every mark it declares reached the draw list,
/// which the atlas's two-pictures-a-frame bound only allows after the
/// specimen's own convergence loop has run.
#[test]
fn every_mark_the_specimen_declares_reaches_the_draw_list() {
    let set = Marks::for_theme(&Theme::forced(ThemeMode::Dark));
    let mut frame = Frame::new().expect("the embedded face yields a grid");
    frame.specimen(&set).expect("the sheet renders");
    let images = frame
        .list()
        .instances
        .iter()
        .filter(|i| i.kind == PrimKind::Image as u32)
        .count();
    assert!(
        images >= Marks::SPECIMEN,
        "{images} pictures for {} marks",
        Marks::SPECIMEN
    );
}

// ── What a frame is made of ────────────────────────────────────────

/// The ground under everything is the volume's own base for the
/// medium, not a colour this crate chose — the whole canvas resets to
/// the lexicon before anything is emitted.
#[test]
fn the_ground_is_the_lexicons_base() {
    let (pixmap, _, _) = still_of("05", ThemeMode::Dark);
    let corner = pixmap.pixel(1, pixmap.height() - 2).expect("a pixel");
    let base = crate::Lexicon::of(Medium::Night).base;
    let byte = |v: f32| (v * 255.0).round() as u8;
    assert_eq!(
        (corner.red(), corner.green(), corner.blue()),
        (byte(base.r), byte(base.g), byte(base.b)),
        "the canvas is the night base"
    );
}

/// A session frame carries surfaces, glyphs and — through the feed's
/// reserved footprint — the stream's pictures at their own pixels.
#[test]
fn a_streams_picture_is_drawn_into_the_feeds_reservation() {
    let mut frame = Frame::new().expect("the embedded face yields a grid");
    let cell = frame.metrics();
    let size = Size::new(80, 24);
    let mut model = Fixture::with_pixels(Fixture::screenshot(size));
    let (w, h) = (
        (f32::from(size.cols) * cell.width()) as u32,
        (f32::from(size.rows) * cell.height()) as u32,
    );
    frame
        .still(&mut model, &clock(), w, h)
        .expect("the frame renders");
    let images = frame
        .list()
        .instances
        .iter()
        .filter(|i| i.kind == PrimKind::Image as u32)
        .count();
    assert!(images >= 1, "the screenshot rides the colour page");
}

/// Nothing the frame emits falls outside a batch — the one-batch
/// contract the renderer draws by.
#[test]
fn nothing_the_frame_emits_falls_outside_a_batch() {
    let (_, frame, _) = still_of("05", ThemeMode::Dark);
    let list = frame.list();
    let batched: usize = list.batches.iter().map(|b| b.range.len()).sum::<usize>();
    assert_eq!(batched, list.instances.len(), "every instance is batched");
}

/// While a turn overflows the window, the stage's subtree is its own batch under a
/// scissor that is the stage's clamped box: the overflow rows are emitted above it
/// (justify-end moved them there) and the scissor is what keeps them off the header
/// band.
#[test]
fn a_streaming_turn_puts_the_stage_in_its_own_scissored_batch() {
    use magistr_application::sessions::AgentEvent;
    let mut frame = Frame::new().expect("the embedded face yields a grid");
    let cell = frame.metrics();
    let size = Size::new(80, 24);
    let mut model = Model::new(size);
    for line in 0..120 {
        let _ = update(
            &mut model,
            Msg::Agent(AgentEvent::ToolCall {
                id: format!("tool-{line}"),
                title: format!("Review delivery {line}"),
                verb: magistr_application::sessions::ToolVerb::Tool,
                status: magistr_application::sessions::ToolStatus::InProgress,
                diff: None,
                images: Vec::new(),
                record: Default::default(),
            }),
        );
    }
    let (w, h) = (
        (f32::from(size.cols) * cell.width()) as u32,
        (f32::from(size.rows) * cell.height()) as u32,
    );
    frame
        .still(&mut model, &clock(), w, h)
        .expect("the frame renders");
    let stage = frame
        .rect_of(crate::comp_root::RootComposition::STAGE)
        .expect("the stage is placed");
    let list = frame.list();
    let scissored: Vec<_> = list
        .batches
        .iter()
        .filter(|b| b.scissor == super::scissor_of(stage, [w, h]))
        .collect();
    assert_eq!(scissored.len(), 1, "the stage has its own scissored batch");
    let [sx, sy, sw, sh] = scissored[0].scissor.expect("a scissor");
    // Origin floors and extent ceils — the clip never shaves a content
    // edge pixel — so each measure sits within 2 px of the stage box.
    assert!(
        (sx as f32 - stage[0]).abs() < 2.0
            && (sy as f32 - stage[1]).abs() < 2.0
            && (sw as f32 - stage[2]).abs() < 2.0
            && (sh as f32 - stage[3]).abs() < 2.0,
        "the scissor is the stage's own box: {:?} vs {stage:?}",
        [sx, sy, sw, sh]
    );
    assert!(
        stage[1] > crate::comp_root::RootComposition::HEADER_HEIGHT,
        "the stage box starts below the header band, so the scissor keeps the feed off it"
    );
    let range = scissored[0].range.clone();
    let overflowed = list.instances[range.start as usize..range.end as usize]
        .iter()
        .any(|i| i.rect[1] < sy as f32);
    assert!(
        overflowed,
        "the top overflow is emitted into the scissored batch — the clip is load-bearing"
    );
    let batched: usize = list.batches.iter().map(|b| b.range.len()).sum::<usize>();
    assert_eq!(batched, list.instances.len(), "every instance is batched");
}

/// A long tool command is wrapped to the column the window gave it, not to the whole
/// window's columns: nothing the feed lays out reaches past the stage's own right edge,
/// so the scissor has nothing horizontal to cut.
#[test]
fn a_long_command_is_wrapped_to_the_stage_box_and_never_reaches_past_it() {
    use magistr_application::sessions::{ToolStatus, ToolVerb};
    let mut frame = Frame::new().expect("the embedded face yields a grid");
    let cell = frame.metrics();
    let size = Size::new(160, 40);
    let mut model = Model::new(size);
    let long = format!(
        "run cat crates/magistr-gui2/src/parity.rs && echo \"=== mask ===\" && {}",
        "cat crates/magistr-gui2/src/mask.rs ".repeat(6)
    );
    let _ = update(
        &mut model,
        Msg::Agent(magistr_application::sessions::AgentEvent::ToolCall {
            id: "c0".to_owned(),
            title: long,
            verb: ToolVerb::Run,
            status: ToolStatus::Completed,
            diff: None,
            images: Vec::new(),
            record: Default::default(),
        }),
    );
    let (w, h) = (
        (f32::from(size.cols) * cell.width()) as u32,
        (f32::from(size.rows) * cell.height()) as u32,
    );
    frame
        .still(&mut model, &clock(), w, h)
        .expect("the frame renders");

    let [sx, _, sw, _] = frame
        .rect_of(crate::comp_root::RootComposition::STAGE)
        .expect("the stage is placed");
    let (tree, layout) = frame.placed().expect("a frame was built");
    let stage = index_of(tree, crate::comp_root::RootComposition::STAGE).expect("the stage");
    let count = {
        let mut found = 0;
        tree.each(&mut |index, element| {
            if index == stage {
                found = element.count();
            }
        });
        found
    };
    for index in stage..stage + count {
        let [x, _, w, _] = layout.rect(index);
        assert!(
            x + w <= sx + sw + 1.0,
            "element {index} reaches {} past the stage's right edge {}",
            x + w,
            sx + sw
        );
    }
    // The head is cut to what the card holds, so the command reads as
    // elided rather than sliced: the model was told the card's mono
    // capacity and cut the title to it.
    let cols = model
        .mono_cols()
        .expect("the frame reported its mono column");
    assert!(
        cols < model.turn_width(),
        "the mono column ({cols}) is the card's, not the window's ({})",
        model.turn_width()
    );
    let head = frame
        .says()
        .into_iter()
        .find(|(text, _)| text.contains("crates/magistr-gui2/src/parity.rs"))
        .expect("the command head is drawn");
    assert!(
        head.0.chars().count() <= usize::from(cols),
        "the head is cut to the card ({cols} cells): {:?}",
        head.0
    );
    assert!(
        head.1[0] + head.1[2] <= sx + sw + 1.0,
        "and it stays inside the column: {head:?}"
    );

    // The card itself is the column's own width — the capture's
    // anatomy, and what makes a long command clip inside the card
    // instead of carrying the card off the edge with it.
    let card = layout.rect(stage + 2);
    assert!(
        (card[2] - sw).abs() < 1.0,
        "the card spans the column: {} vs {sw}",
        card[2]
    );
}

/// The report settles: once the turn is wrapped to the column it was
/// given, another build re-reports nothing — which is what stops
/// `Frame::still`'s convergence loop.
#[test]
fn the_turn_column_report_settles() {
    let (_, mut frame, mut model) = still_of("05", ThemeMode::Dark);
    let (width, height) = Fixture::CAPTURE;
    let reported = frame
        .build(&mut model, &clock(), width, height)
        .expect("the frame builds");
    assert!(!reported, "a settled frame re-reports nothing");
}

/// Roadmap clips graph, list and details separately from its fixed navigation.
#[test]
fn roadmap_frames_clip_their_independent_panes() {
    let (_, frame, _) = still_of("03", ThemeMode::Dark);
    assert!(
        frame.list().batches.iter().any(|b| b.scissor.is_some()),
        "roadmap content is clipped"
    );
    assert!(
        frame.list().batches.iter().any(|b| b.scissor.is_none()),
        "fixed navigation stays outside scrolling clips"
    );
}

/// The stream's pictures are feed content: their quads land inside the
/// stage's scissored batch, so a tall image clips at the fold exactly
/// as its rows do.
#[test]
fn the_feeds_picture_lands_inside_the_stage_scissored_batch() {
    let mut frame = Frame::new().expect("the embedded face yields a grid");
    let cell = frame.metrics();
    let size = Size::new(80, 24);
    let mut model = Fixture::with_pixels(Fixture::screenshot(size));
    let (w, h) = (
        (f32::from(size.cols) * cell.width()) as u32,
        (f32::from(size.rows) * cell.height()) as u32,
    );
    frame
        .still(&mut model, &clock(), w, h)
        .expect("the frame renders");
    let list = frame.list();
    let batch = list
        .batches
        .iter()
        .find(|b| b.scissor.is_some())
        .expect("the stage batch");
    // The chrome's marks ride `Instance::image` too; the stream's
    // picture is the one image wider than any mark's square.
    let pictures: Vec<u32> = list
        .instances
        .iter()
        .enumerate()
        .filter(|(_, i)| i.kind == PrimKind::Image as u32 && i.rect[2] > cell.width() * 3.0)
        .map(|(at, _)| at as u32)
        .collect();
    assert!(!pictures.is_empty(), "the screenshot rides the colour page");
    assert!(
        pictures.iter().all(|at| batch.range.contains(at)),
        "every picture quad is inside the stage's scissor: {pictures:?} vs {:?}",
        batch.range
    );
}

/// The pixel you click is the widget you click: the composer pill's
/// centre resolves to `Widget::Composer` through the registry the same
/// layout registered.
#[test]
fn a_pixel_inside_the_composer_resolves_to_the_composer() {
    let (_, frame, _) = still_of("05", ThemeMode::Dark);
    let (tree, layout) = frame.placed().expect("a frame was built");
    let mut pill = None;
    tree.each(&mut |index, element| {
        if element.id == Some(crate::comp_composer::ComposerComposition::PILL) {
            pill = Some(layout.rect(index));
        }
    });
    let [x, y, w, h] = pill.expect("the composer is placed");
    let hit = frame
        .hits()
        .hit_at(f64::from(x + w / 2.0), f64::from(y + h / 2.0))
        .expect("the pill answers");
    assert_eq!(hit, Widget::Composer.id());
}

/// The divider answers as the rail's drag handle, which is what makes
/// the drag arithmetic the model's own.
#[test]
fn the_divider_resolves_to_the_rail_divider() {
    let (_, frame, _) = still_of("05", ThemeMode::Dark);
    let (tree, layout) = frame.placed().expect("a frame was built");
    let mut divider = None;
    tree.each(&mut |index, element| {
        if element.id
            == Some(crate::comp_workspace::WorkspaceComposition::id(
                Widget::PanelColumns.id(),
            ))
        {
            divider = Some(layout.rect(index));
        }
    });
    let [x, y, w, h] = divider.expect("the divider is placed");
    let hit = frame
        .hits()
        .hit_at(f64::from(x + w / 2.0), f64::from(y + h / 2.0))
        .expect("the divider answers");
    assert_eq!(hit, Widget::PanelColumns.id());
}

/// A zero-sided viewport is refused, never painted — a window reports
/// one mid-minimize and a golden must not be a picture of it.
#[test]
fn a_zero_sided_viewport_is_refused_not_painted() {
    let mut frame = Frame::new().expect("the embedded face yields a grid");
    let mut model = Fixture::mock("05", Size::new(80, 24)).expect("a fixture");
    assert!(frame.still(&mut model, &clock(), 0, 100).is_err());
    assert!(frame.still(&mut model, &clock(), 100, 0).is_err());
}

/// The still is exactly the viewport, and every pixel of it is opaque
/// — the assumption `rasterize`'s straight-vs-premultiplied shortcut
/// stands on.
#[test]
fn the_still_is_the_viewport_and_every_pixel_is_opaque() {
    let (pixmap, frame, _) = still_of("05", ThemeMode::Dark);
    let cell = frame.metrics();
    let _ = cell;
    assert_eq!((pixmap.width(), pixmap.height()), Fixture::CAPTURE);
    assert!(
        pixmap.pixels().iter().all(|p| p.alpha() == 255),
        "an opaque canvas"
    );
}

/// A still leaves the graph-rect reports settled: the sky field's cell
/// rect is on the model (the constellation's placement reads it), the
/// rail's retired block is `None`, and one more build reports no
/// change.
#[test]
fn a_still_leaves_the_field_report_settled() {
    let (_, mut frame, mut model) = still_of("03", ThemeMode::Dark);
    assert!(
        model.roadmap_graph_rect().is_none(),
        "Tracks leaves graph geometry lazy"
    );
    update(
        &mut model,
        Msg::Mouse {
            event: MouseEvent::new(MouseKind::Down(Button::Left), 0, 0),
            target: Some(Widget::RoadmapDependencies.id()),
        },
    );
    let (width, height) = Fixture::CAPTURE;
    frame
        .still(&mut model, &clock(), width, height)
        .expect("dependencies render");
    assert!(
        model.roadmap_graph_rect().is_some(),
        "the sky field is reported"
    );
    assert_eq!(model.rail_graph_rect(), None, "no rail block any more");
    let (width, height) = Fixture::CAPTURE;
    let reported = frame
        .build(&mut model, &clock(), width, height)
        .expect("the frame builds");
    assert!(!reported, "a settled frame re-reports nothing");
}

/// `Frame::build` is the one site that declares the surface: whatever
/// the caller handed over, the model it painted is a window.
#[test]
fn the_frame_declares_its_own_surface() {
    let (_, _, model) = still_of("05", ThemeMode::Dark);
    assert!(model.surface().is_native());
}

/// The frame declares its cell to the model through the resize reply,
/// so an image footprint is measured on the cell the quad is drawn on.
#[test]
fn a_build_declares_the_windows_cell_to_the_model() {
    let (_, frame, model) = still_of("05", ThemeMode::Dark);
    assert_eq!(model.cell_pixels(), Some(frame.metrics().cell_pixels()));
}

/// Hover is folded over the placed tree: a pointer over the composer
/// changes the frame's interaction state, and the same move again does
/// not.
#[test]
fn a_pointer_over_the_composer_changes_the_state_once() {
    let (_, mut frame, _) = still_of("05", ThemeMode::Dark);
    let (tree, layout) = frame.placed().expect("a frame was built");
    let mut pill = None;
    tree.each(&mut |index, element| {
        if element.id == Some(crate::comp_composer::ComposerComposition::PILL) {
            pill = Some(layout.rect(index));
        }
    });
    let [x, y, w, h] = pill.expect("the composer is placed");
    let over = Msg::Mouse {
        event: MouseEvent::new(MouseKind::Move, 0, 0)
            .at_px((x + w / 2.0) as u32, (y + h / 2.0) as u32),
        target: None,
    };
    assert!(frame.fold_state(&over), "the hover arrived");
    assert!(!frame.fold_state(&over), "and it is settled");
    let _ = Button::Left;
}

// ── The density seam ───────────────────────────────────────────────

/// A 2x window grids by the size its logical pixels imply: the face is
/// drawn at twice the px, the cell doubles, and the same physical
/// viewport floors to the same column count a 1x window of half the
/// pixels has.
#[test]
fn a_two_x_window_grids_by_the_size_its_logical_pixels_imply() {
    let mut frame = Frame::new().expect("a grid");
    let one = frame.metrics();
    frame.rescale(2.0).expect("the face resizes");
    let two = frame.metrics();
    assert_eq!(
        one.grid(1280, 800),
        two.grid(2560, 1600),
        "the grid follows the logical size"
    );
    assert!(two.width() > one.width());
    assert!(two.height() > one.height());
}

/// A rescale to the same factor answers `false` and changes nothing.
#[test]
fn a_rescale_to_one_changes_nothing() {
    let mut frame = Frame::new().expect("a grid");
    let before = frame.metrics();
    assert!(!frame.rescale(1.0).expect("a no-op rescale"));
    assert_eq!(frame.metrics(), before);
}

/// A scale no window reports — zero, negative, NaN — is refused.
#[test]
fn an_unusable_scale_is_refused() {
    let mut frame = Frame::new().expect("a grid");
    assert!(frame.rescale(0.0).is_err());
    assert!(frame.rescale(-1.0).is_err());
    assert!(frame.rescale(f64::NAN).is_err());
}

// ── The Galaxy ───────────────────────────────────────

#[test]
fn project_stars_labels_buttons_and_accessibility_open_the_same_project() {
    use accesskit::{Action, ActionRequest, TreeId};
    use magistr_cockpit::cockpit::{Place, Surface};
    use magistr_cockpit::input::{KeyCode, KeyEvent};

    for (width, height, scale) in [(1100, 760, 1.0), (800, 640, 1.0), (2200, 1520, 2.0)] {
        for mode in [ThemeMode::Dark, ThemeMode::Light] {
            for activation in 0..6 {
                let mut frame = Frame::new().unwrap();
                frame.rescale(scale).unwrap();
                let mut model = Model::new(frame.metrics().grid(width, height))
                    .with_project("Delivery", "/projects/delivery")
                    .with_surface(Surface::Native)
                    .with_place(Place::Session)
                    .with_theme(Theme::forced(mode));
                update(&mut model, Msg::Paste("Keep my draft".into()));
                let draft = model.prompt().clone();
                model = model.with_place(Place::Galaxy);
                frame.still(&mut model, &clock(), width, height).unwrap();
                let target = Widget::galaxy_cluster(0);
                let access = frame.access();
                let link = access
                    .nodes()
                    .iter()
                    .find(|n| n.target == Some(target))
                    .unwrap();
                assert_eq!(link.label, "Open project Delivery");
                assert!(link.actions.contains(&Action::Click));
                assert!(link.actions.contains(&Action::Focus));
                let request = ActionRequest {
                    action: Action::Click,
                    target_tree: TreeId::ROOT,
                    target_node: link.id,
                    data: None,
                };
                let messages = match activation {
                    0..=2 => {
                        let (x, y) = match activation {
                            0 => {
                                let [x, y, w, h] = frame.rect_of(GalaxyComposition::FIELD).unwrap();
                                let projects = GalaxyComposition::projects(&model);
                                let placed = GalaxyLayout::place(
                                    &projects,
                                    (w / scale as f32, h / scale as f32),
                                    model.galaxy().zoom(),
                                );
                                let star = placed[0].stars[0];
                                (x + star.0 * scale as f32, y + star.1 * scale as f32)
                            }
                            1 => {
                                let (_, [x, y, w, h]) = frame
                                    .says()
                                    .into_iter()
                                    .find(|(text, _)| text == "Delivery")
                                    .unwrap();
                                (x + w / 2.0, y + h / 2.0)
                            }
                            _ => {
                                let [x, y, w, h] = frame
                                    .rect_of(crate::ElementId(GalaxyComposition::PROJECTS))
                                    .unwrap();
                                (x + w / 2.0, y + h / 2.0)
                            }
                        };
                        let hit = frame.hits().hit_at(x.into(), y.into());
                        assert_eq!(hit, Some(target), "activation {activation}, scale {scale}");
                        frame.input(
                            &model,
                            Msg::Mouse {
                                event: MouseEvent::new(MouseKind::Down(Button::Left), 0, 0)
                                    .at_px(x as u32, y as u32),
                                target: hit,
                            },
                        )
                    }
                    3 => frame.access_action(&model, request).unwrap(),
                    4 => frame.input(&model, Msg::Key(KeyEvent::new(KeyCode::Enter))),
                    _ => {
                        for _ in 0..8 {
                            let messages =
                                frame.input(&model, Msg::Key(KeyEvent::new(KeyCode::Tab)));
                            assert!(messages.is_empty());
                            if frame.focus_target() == Some(target) {
                                break;
                            }
                        }
                        assert_eq!(frame.focus_target(), Some(target));
                        frame.input(&model, Msg::Key(KeyEvent::new(KeyCode::Enter)))
                    }
                };
                assert!(!messages.is_empty(), "activation {activation} was ignored");
                for msg in messages {
                    let commands = update(&mut model, msg);
                    assert!(
                        commands
                            .iter()
                            .all(|cmd| matches!(cmd, magistr_cockpit::cockpit::Cmd::Animate(_))),
                        "activation {activation}: {commands:?}"
                    );
                }
                assert_eq!(model.place(), Some(Place::Sky), "activation {activation}");
                assert_eq!(model.prompt(), &draft);
            }
        }
    }
}

/// Another project's control on the home page, reached by pointer at its painted box,
/// by an accessibility click and by Tab then Return: each asks the host once and keeps
/// the draft. A refusal is then read from the window in words with the control live
/// again, and the host's answer swaps the window onto the new project.
#[test]
fn another_project_opens_by_pointer_keyboard_and_accessibility_and_a_refusal_is_read() {
    use accesskit::{Action, ActionRequest, Role, TreeId};
    use magistr_cockpit::cockpit::{Cmd, GalaxyData, GalaxyProject, Place, ProjectSeed, Surface};
    use magistr_cockpit::input::{KeyCode, KeyEvent};
    use magistr_cockpit::job::{JobId, JobOutput};

    let (width, height) = (1100, 760);
    let target = Widget::galaxy_cluster(1);
    let other = || Cmd::OpenProject("/projects/other".into());
    for activation in 0..3 {
        let mut frame = Frame::new().unwrap();
        let mut model = Model::new(frame.metrics().grid(width, height))
            .with_hosted_work()
            .with_project("Delivery", "/projects/delivery")
            .with_surface(Surface::Native)
            .with_place(Place::Session);
        update(&mut model, Msg::Paste("Keep my draft".into()));
        let draft = model.prompt().clone();
        model = model.with_place(Place::Galaxy);
        update(
            &mut model,
            Msg::Job {
                id: JobId::new(1),
                output: JobOutput::Galaxy(Box::new(GalaxyData {
                    projects: vec![
                        GalaxyProject {
                            name: "Delivery".into(),
                            dir: Some("/projects/delivery".into()),
                            here: true,
                            ..GalaxyProject::default()
                        },
                        GalaxyProject {
                            name: "Other".into(),
                            dir: Some("/projects/other".into()),
                            ..GalaxyProject::default()
                        },
                    ],
                    spent_cents: 0,
                    cap_cents: None,
                    stop_cents: None,
                })),
            },
        );
        frame.still(&mut model, &clock(), width, height).unwrap();
        let access = frame.access();
        let link = access
            .nodes()
            .iter()
            .find(|n| n.target == Some(target))
            .expect("the other project's control is published");
        assert_eq!(link.label, "Open project Other");
        assert_eq!(
            link.role,
            Role::Link,
            "a project is a place the window goes to"
        );
        assert!(link.actions.contains(&Action::Click) && link.actions.contains(&Action::Focus));
        let messages = match activation {
            0 => {
                let [x, y, w, h] = frame
                    .rect_of(crate::ElementId(GalaxyComposition::PROJECTS + 1))
                    .unwrap();
                let (x, y) = (x + w / 2.0, y + h / 2.0);
                let hit = frame.hits().hit_at(x.into(), y.into());
                assert_eq!(hit, Some(target));
                frame.input(
                    &model,
                    Msg::Mouse {
                        event: MouseEvent::new(MouseKind::Down(Button::Left), 0, 0)
                            .at_px(x as u32, y as u32),
                        target: hit,
                    },
                )
            }
            1 => frame
                .access_action(
                    &model,
                    ActionRequest {
                        action: Action::Click,
                        target_tree: TreeId::ROOT,
                        target_node: link.id,
                        data: None,
                    },
                )
                .unwrap(),
            _ => {
                for _ in 0..8 {
                    assert!(
                        frame
                            .input(&model, Msg::Key(KeyEvent::new(KeyCode::Tab)))
                            .is_empty()
                    );
                    if frame.focus_target() == Some(target) {
                        break;
                    }
                }
                assert_eq!(frame.focus_target(), Some(target));
                frame.input(&model, Msg::Key(KeyEvent::new(KeyCode::Enter)))
            }
        };
        let commands: Vec<Cmd> = messages
            .into_iter()
            .flat_map(|msg| update(&mut model, msg))
            .collect();
        assert_eq!(
            commands.iter().filter(|cmd| **cmd == other()).count(),
            1,
            "activation {activation}: {commands:?}"
        );
        assert_eq!(
            model.place(),
            Some(Place::Galaxy),
            "the window holds until the host answers"
        );
        assert_eq!(model.prompt(), &draft);
        frame.still(&mut model, &clock(), width, height).unwrap();
        assert!(
            frame
                .says()
                .iter()
                .any(|(text, _)| text == "Opening\u{2026}")
        );
        assert!(
            !frame
                .access()
                .nodes()
                .iter()
                .any(|n| n.target == Some(target)),
            "no second ask is offered while the project opens"
        );

        // The host refuses: the words are on the window and the control is back.
        update(
            &mut model,
            Msg::CommandFailed {
                cmd: other(),
                error: "Another Magistr session has this project open.".into(),
            },
        );
        frame.still(&mut model, &clock(), width, height).unwrap();
        let refusal =
            "Magistr could not open the project. Another Magistr session has this project open.";
        assert!(
            frame.says().iter().any(|(text, _)| text == refusal),
            "activation {activation}: {:?}",
            frame.says()
        );
        let access = frame.access();
        let refused = access
            .nodes()
            .iter()
            .find(|n| n.target == Some(target))
            .expect("the control is offered again");
        assert_eq!(
            refused.label,
            format!("Open project Other. {refusal}"),
            "the refusal is read, not only painted"
        );
        assert_eq!(model.galaxy().home(), Some(0), "still on Delivery");
        assert_eq!(model.prompt(), &draft);

        // Asked again, the host admits it: the window is the other project's.
        let again = frame.input(
            &model,
            Msg::Mouse {
                event: MouseEvent::new(MouseKind::Down(Button::Left), 0, 0),
                target: Some(target),
            },
        );
        for msg in again {
            update(&mut model, msg);
        }
        update(
            &mut model,
            Msg::SessionFrame {
                address: magistr_cockpit::cockpit::SessionAddress {
                    project: "other".into(),
                    workspace: "/projects/other".into(),
                    session: "other".into(),
                },
                sequence: 1,
                seed: Some(Box::new(ProjectSeed {
                    name: "Other".into(),
                    dir: "/projects/other".into(),
                    title: "Other".into(),
                    session_address: Some(magistr_cockpit::cockpit::SessionAddress {
                        project: "other".into(),
                        workspace: "/projects/other".into(),
                        session: "other".into(),
                    }),
                    draft: Some("Other draft".into()),
                    ..ProjectSeed::default()
                })),
                messages: Vec::new(),
            },
        );
        frame.still(&mut model, &clock(), width, height).unwrap();
        assert_eq!(model.place(), Some(Place::Sky));
        assert_eq!(model.galaxy().projects()[0].name, "Other");
        assert_eq!(
            model.prompt().text(),
            "Other draft",
            "the selected session owns its draft"
        );
    }
}

// ── Starting work ────────────────────────────────────

/// The two starts on the session window, each reached by pointer at its painted box,
/// by an accessibility click and by Tab then Return — one route, the cockpit's own
/// widget, giving the host's command and keeping the draft. With a loop live the starts
/// are gone and the loop's verbs answer the same three ways.
#[test]
fn the_starts_answer_pointer_keyboard_and_accessibility_and_yield_to_the_loops_verbs() {
    use accesskit::{Action, ActionRequest, Role, TreeId};
    use magistr_cockpit::cockpit::{Cmd, LaunchAction, LoopCmd};
    use magistr_cockpit::input::{KeyCode, KeyEvent};

    let (width, height) = (1100, 760);
    let cases = [
        (
            false,
            Widget::LaunchNext,
            "Start next item",
            Cmd::Launch(LaunchAction::Next),
        ),
        (true, Widget::LoopStep, "Step", Cmd::Loop(LoopCmd::Step)),
        (true, Widget::LoopAbort, "Abort", Cmd::Loop(LoopCmd::Abort)),
    ];
    for (live, widget, label, expected) in cases {
        for activation in 0..3 {
            let mut frame = Frame::new().unwrap();
            let size = frame.metrics().grid(width, height);
            let mut model = if live {
                Fixture::mock("05", size).unwrap().with_hosted_work()
            } else {
                Fixture::rail_idle(size)
            };
            model.set_surface(magistr_cockpit::cockpit::Surface::Native);
            update(&mut model, Msg::Paste("Keep my draft".into()));
            let draft = model.prompt().clone();
            frame.still(&mut model, &clock(), width, height).unwrap();
            let target = widget.id();
            let access = frame.access();
            let node = access
                .nodes()
                .iter()
                .find(|n| n.target == Some(target))
                .unwrap_or_else(|| panic!("{label} is published"));
            assert_eq!(node.label, label);
            assert_eq!(node.role, Role::Button);
            assert!(node.actions.contains(&Action::Click) && node.actions.contains(&Action::Focus));
            let absent = if live {
                Widget::LaunchLoop
            } else {
                Widget::LoopAbort
            };
            assert!(
                access.nodes().iter().all(|n| n.target != Some(absent.id())),
                "{absent:?} is not offered beside {label}"
            );
            let messages = match activation {
                0 => {
                    let (_, [x, y, w, h]) = frame
                        .says()
                        .into_iter()
                        .find(|(text, _)| text == label)
                        .unwrap();
                    let (x, y) = (x + w / 2.0, y + h / 2.0);
                    let hit = frame.hits().hit_at(x.into(), y.into());
                    assert_eq!(hit, Some(target), "{label} under the pointer");
                    frame.input(
                        &model,
                        Msg::Mouse {
                            event: MouseEvent::new(MouseKind::Down(Button::Left), 0, 0)
                                .at_px(x as u32, y as u32),
                            target: hit,
                        },
                    )
                }
                1 => frame
                    .access_action(
                        &model,
                        ActionRequest {
                            action: Action::Click,
                            target_tree: TreeId::ROOT,
                            target_node: node.id,
                            data: None,
                        },
                    )
                    .unwrap(),
                _ => {
                    for _ in 0..64 {
                        if frame.focus_target() == Some(target) {
                            break;
                        }
                        let messages = frame.input(&model, Msg::Key(KeyEvent::new(KeyCode::Tab)));
                        assert!(messages.is_empty());
                    }
                    assert_eq!(
                        frame.focus_target(),
                        Some(target),
                        "{label} takes Tab focus"
                    );
                    frame.input(&model, Msg::Key(KeyEvent::new(KeyCode::Enter)))
                }
            };
            assert!(
                !messages.is_empty(),
                "{label}: activation {activation} was ignored"
            );
            let commands: Vec<_> = messages
                .into_iter()
                .flat_map(|msg| update(&mut model, msg))
                .collect();
            assert!(
                commands.contains(&expected),
                "{label} {activation}: {commands:?}"
            );
            assert_eq!(model.prompt(), &draft, "the draft is kept");
        }
    }
}

/// View 01 — the Galaxy: three clusters on the field, their labels
/// pinned by the same layout, the chips in the header, the Galaxy's
/// foot, and the ⌘K palette floated over it all.
#[test]
fn shot_galaxy() {
    shots("01");
}

/// The motion goldens' home — the filmstrips, beside the stills.
fn motion_golden() -> Golden {
    Golden::new(
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("tests")
            .join("golden")
            .join("motion"),
    )
}

/// *Breathe* on the Galaxy, as a filmstrip: four panels across one period of the live
/// stars' breath — foot, rising, crest, falling — so the mauve session and the amber
/// question can be seen to swell and settle, which a still of one frame could not show.
#[test]
fn shot_galaxy_breathe() {
    use magistr_cockpit::cockpit::Motion;
    const PANELS: u32 = 4;
    let (width, height) = Fixture::CAPTURE;
    let mut frame = Frame::new().expect("the embedded face yields a grid");
    let cell = frame.metrics();
    let size = Fixture::capture_grid((cell.width(), cell.height()));
    let mut model =
        Fixture::with_pixels(Fixture::galaxy(size).with_theme(Theme::forced(ThemeMode::Dark)));
    let mut strip = Pixmap::new(width * PANELS, height).expect("the strip allocates");
    for panel in 0..PANELS {
        let still = frame
            .still(&mut model, &clock(), width, height)
            .expect("the frame renders");
        strip.draw_pixmap(
            (panel * width) as i32,
            0,
            still.as_ref(),
            &tiny_skia::PixmapPaint::default(),
            tiny_skia::Transform::identity(),
            None,
        );
        for _ in 0..(Motion::GLOW_STEPS / u64::from(PANELS)) {
            let _ = update(&mut model, Msg::Tick);
        }
    }
    motion_golden().assert_matches("galaxy-breathe-mocha", &strip);
}

// ── Birth ─────────────────────────────────────────

/// The five moments as stills through the whole frame: the ground card
/// over the room, the survey's columns over the sweep, the interview's
/// stack, the charting's list and field, first light's sky and toast.
#[test]
fn shot_birth() {
    for view in ["F1", "F2", "F3", "F4", "F5"] {
        shots(view);
    }
}

/// `model` walked `panels` frames `step` ticks apart, side by side.
fn filmstrip(model: &mut Model, panels: u32, step: u64) -> Pixmap {
    let (width, height) = Fixture::CAPTURE;
    let mut frame = Frame::new().expect("the embedded face yields a grid");
    let mut strip = Pixmap::new(width * panels, height).expect("the strip allocates");
    for panel in 0..panels {
        let still = frame
            .still(model, &clock(), width, height)
            .expect("the frame renders");
        strip.draw_pixmap(
            (panel * width) as i32,
            0,
            still.as_ref(),
            &tiny_skia::PixmapPaint::default(),
            tiny_skia::Transform::identity(),
            None,
        );
        for _ in 0..step {
            let _ = update(model, Msg::Tick);
        }
    }
    strip
}

/// *Radar* on the survey, as a filmstrip: four panels across one revolution of the
/// sweep — the wedge turning past the columns while every row holds where it landed —
/// which a still of one frame could not show.
#[test]
fn shot_birth_radar() {
    use magistr_cockpit::cockpit::Birth;
    const PANELS: u32 = 4;
    let frame = Frame::new().expect("the embedded face yields a grid");
    let cell = frame.metrics();
    let size = Fixture::capture_grid((cell.width(), cell.height()));
    let mut model =
        Fixture::with_pixels(Fixture::birth_f2(size).with_theme(Theme::forced(ThemeMode::Dark)));
    let strip = filmstrip(&mut model, PANELS, Birth::RADAR_STEPS / u64::from(PANELS));
    motion_golden().assert_matches("birth-radar-mocha", &strip);
}

/// *Born* on the charting, as a filmstrip: four panels from the proposal's arrival to
/// its last star — the stars appearing one by one with their edges and labels
/// following, the way the agent reasons them out.
#[test]
fn shot_birth_born() {
    use magistr_cockpit::cockpit::Birth;
    const PANELS: u32 = 4;
    let frame = Frame::new().expect("the embedded face yields a grid");
    let cell = frame.metrics();
    let size = Fixture::capture_grid((cell.width(), cell.height()));
    let mut model =
        Fixture::with_pixels(Fixture::birth_f4(size).with_theme(Theme::forced(ThemeMode::Dark)));
    let stars = Fixture::proposal().chunks.len() as u64;
    let whole = stars * Birth::BORN_STAGGER + magistr_cockpit::cockpit::Motion::ARRIVAL_STEPS;
    let strip = filmstrip(&mut model, PANELS, whole / u64::from(PANELS - 1));
    motion_golden().assert_matches("birth-born-mocha", &strip);
}

/// The terminal pane *following*, as a filmstrip: four panels of a run card's screen
/// with a row arriving between each — the tail moving up under the cursor, the cursor
/// blinking at the caret's rhythm — which a still of one frame could not show.
#[test]
fn shot_pane_follow() {
    use magistr_application::sessions::AgentEvent;
    use magistr_cockpit::theme::Theme;
    const PANELS: u32 = 4;
    let (width, height) = Fixture::CAPTURE;
    let mut frame = Frame::new().expect("the embedded face yields a grid");
    let cell = frame.metrics();
    let size = Fixture::capture_grid((cell.width(), cell.height()));
    let mut model = Fixture::with_pixels(
        Fixture::pane_following(size).with_theme(Theme::forced(ThemeMode::Dark)),
    );
    let mut strip = Pixmap::new(width * PANELS, height).expect("the strip allocates");
    for panel in 0..PANELS {
        let still = frame
            .still(&mut model, &clock(), width, height)
            .expect("the frame renders");
        strip.draw_pixmap(
            (panel * width) as i32,
            0,
            still.as_ref(),
            &tiny_skia::PixmapPaint::default(),
            tiny_skia::Transform::identity(),
            None,
        );
        let _ = update(
            &mut model,
            Msg::Agent(AgentEvent::Terminal {
                terminal_id: "term-1".to_owned(),
                bytes: format!(
                    "\r\n        \x1b[32mPASS\x1b[0m [   0.0{panel}s] follow::row_arrives_{panel}"
                )
                .into_bytes(),
            }),
        );
        // Half a blink period between panels: the cursor is on, off,
        // on, off across the strip while the rows climb.
        for _ in 0..(crate::term_pane::TermPane::BLINK_STEPS / 2) {
            let _ = update(&mut model, Msg::Tick);
        }
    }
    motion_golden().assert_matches("pane-follow-mocha", &strip);
}

/// The terminal dock *rising*, as a filmstrip: four panels from the press of ⌘↵ — the
/// run card's pane on the feed with nothing at the composer's edge — through the dock
/// climbing out of that edge row by row, the feed giving up the rows above it, to the
/// dock settled at a third of the stage with the grip over it; the composer and the
/// rail stand where they were on every panel.
#[test]
fn shot_dock_rise() {
    use magistr_cockpit::cockpit::Eased;
    use magistr_cockpit::input::{KeyCode, KeyEvent, Modifiers};
    use magistr_cockpit::theme::Theme;
    const PANELS: u32 = 4;
    let frame = Frame::new().expect("the embedded face yields a grid");
    let cell = frame.metrics();
    let size = Fixture::capture_grid((cell.width(), cell.height()));
    let mut model = Fixture::with_pixels(
        Fixture::pane_following(size).with_theme(Theme::forced(ThemeMode::Dark)),
    );
    let _ = update(
        &mut model,
        Msg::Key(KeyEvent::new(KeyCode::Enter).with(Modifiers::SUPER)),
    );
    assert!(model.term_dock().is_up());
    // Three ticks between panels: the edge, two frames of the climb,
    // and the settled dock past the glide's seven steps.
    let strip = filmstrip(&mut model, PANELS, Eased::STEPS / 2);
    motion_golden().assert_matches("dock-rise-mocha", &strip);
}

/// Reduced motion collapses both to their end frames: the survey's
/// rows are all landed with the sweep at its start and the flow
/// settles on its very next tick (the interview, its end frame); the
/// charting's stars are all born, and a second frame is the same
/// picture.
#[test]
fn reduced_motion_collapses_radar_and_born_to_their_end_frames() {
    use magistr_cockpit::cockpit::BirthMoment;
    use magistr_cockpit::theme::MotionMode;
    let (width, height) = Fixture::CAPTURE;
    let mut frame = Frame::new().expect("the embedded face yields a grid");
    let cell = frame.metrics();
    let size = Fixture::capture_grid((cell.width(), cell.height()));
    let reduced = |view: &str| {
        Fixture::with_pixels(
            Fixture::mock(view, size)
                .expect("the flow has a fixture")
                .with_theme(Theme::forced(ThemeMode::Dark).with_motion(MotionMode::Reduced)),
        )
    };
    let mut surveyed = reduced("F2");
    assert!(!surveyed.is_animating(), "F2: nothing asks for a tick");
    let report = Fixture::survey_report();
    assert_eq!(
        surveyed.birth().rows_landed(surveyed.now(), false),
        report.rows(),
        "every row is landed at once"
    );
    let _ = frame
        .still(&mut surveyed, &clock(), width, height)
        .expect("the frame renders");
    let _ = update(&mut surveyed, Msg::Tick);
    assert_eq!(
        surveyed.birth().moment(),
        BirthMoment::Interview,
        "and the survey settles on the next tick"
    );
    let mut charted = reduced("F4");
    assert!(!charted.is_animating(), "F4: nothing asks for a tick");
    let first = frame
        .still(&mut charted, &clock(), width, height)
        .expect("the frame renders");
    let _ = update(&mut charted, Msg::Tick);
    let second = frame
        .still(&mut charted, &clock(), width, height)
        .expect("the frame renders");
    assert_eq!(first.data(), second.data(), "F4: the end frame holds");
    let alphas: Vec<f32> = (0..8)
        .map(|i| charted.birth().born_alpha(i, charted.now(), false))
        .collect();
    assert!(alphas.iter().all(|a| (*a - 1.0).abs() < f32::EPSILON));
}

/// The stage *between rows* (stage-scroll-anchor-and-rate), as a
/// filmstrip: four panels of a pinned feed the wheel just moved a notch
/// up, sampled at the display's own phase — the frame the notch was
/// turned on, half a tick in, a whole tick, a tick and a half — so the
/// rows can be seen to slide by fractions of a row rather than step,
/// which no strip of whole ticks could show. The last panel is a
/// hair short of the settled window, the one every still pins.
#[test]
fn shot_feed_scroll() {
    use magistr_application::sessions::{AgentEvent, TurnEnd};
    use magistr_cockpit::cockpit::{Msg, Surface};
    use magistr_cockpit::input::{KeyCode, KeyEvent};
    const PANELS: u32 = 4;
    let (width, height) = Fixture::CAPTURE;
    let mut frame = Frame::new().expect("the embedded face yields a grid");
    let cell = frame.metrics();
    let size = Fixture::capture_grid((cell.width(), cell.height()));
    let mut model = Fixture::with_pixels(
        Model::new(size)
            .with_surface(Surface::Native)
            .with_theme(Theme::forced(ThemeMode::Dark)),
    );
    for i in 0..120 {
        let _ = update(
            &mut model,
            Msg::Agent(AgentEvent::Text(format!(
                "stream row {i}: the feed carried between rows\n"
            ))),
        );
    }
    let _ = update(
        &mut model,
        Msg::Agent(AgentEvent::TurnEnded(TurnEnd::EndTurn)),
    );
    let _ = update(&mut model, Msg::Key(KeyEvent::new(KeyCode::PageUp)));
    for _ in 0..24 {
        let _ = update(&mut model, Msg::Tick);
    }
    assert!(!model.viewport().is_following(), "pinned");
    let _ = update(
        &mut model,
        Msg::Mouse {
            event: MouseEvent::new(MouseKind::ScrollUp, 0, 0),
            target: Some(Widget::Stage.id()),
        },
    );
    let mut strip = Pixmap::new(width * PANELS, height).expect("the strip allocates");
    let mut fractions = Vec::new();
    for panel in 0..PANELS {
        // Half a tick between panels: the phase alternates 0 and ½
        // with a tick folded between each pair.
        let phase = if panel % 2 == 0 { 0.0 } else { 0.5 };
        let _ = update(&mut model, Msg::Phase(phase));
        fractions.push(model.stage_scroll_fraction());
        let still = frame
            .still(&mut model, &clock(), width, height)
            .expect("the frame renders");
        strip.draw_pixmap(
            (panel * width) as i32,
            0,
            still.as_ref(),
            &tiny_skia::PixmapPaint::default(),
            tiny_skia::Transform::identity(),
            None,
        );
        if panel % 2 == 1 {
            let _ = update(&mut model, Msg::Tick);
        }
    }
    assert!(
        fractions[1..].iter().all(|f| *f > 0.0 && *f < 1.0),
        "the paint sits between rows on every phase past the first: {fractions:?}"
    );
    motion_golden().assert_matches("feed-scroll-mocha", &strip);
}
