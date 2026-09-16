//! The pixel layer end to end over the fake terminal: scripted
//! think frames put a branch graph on the Trace tab, and the byte stream
//! proves the compositor's economy — one kitty transmit when the graph
//! first paints, none across idle frames, one more after a resize
//! re-placement, the default cell size when the terminal never answered
//! `CSI 16 t`, and a re-size via the late reply when it answers after
//! startup.
//!
//! Since the header carries pixel chrome of its own (the brand
//! mark, the activity sparkline), so the graph's transmits are counted
//! by their declared pixel width rather than by the raw total: the
//! chrome's surfaces are a cell or eight wide, the graph's is the whole
//! block. [`graph_transmits`] is that filter.

use std::time::Duration;

use magistr_cockpit::cockpit::{Model, Motion, Size, StageTab};
use magistr_cockpit::theme::Icons;
use magistr_tui::graphics::Env;
use magistr_tui::harness::{Capture, FakeAgent, FakeEngines, FakeTerminal, Script};
use magistr_tui::runtime::{Options, Runtime};

const MS: fn(u64) -> Duration = Duration::from_millis;
const SIZE: Size = Size::new(80, 24);
/// A kitty image transmit's opening — continuation chunks carry `m=`
/// without `a=t`, so this counts transmits, not chunks.
const TRANSMIT: &[u8] = b"\x1b_Ga=t";
/// A kitty placement command's opening.
const PLACE: &[u8] = b"\x1b_Ga=p";
/// A kitty delete command's opening (placement deletes and the exit
/// release both).
const DELETE: &[u8] = b"\x1b_Ga=d";
/// A sixel image's opening.
const SIXEL: &[u8] = b"\x1bP0;1;0q";

/// The graph is at least this wide — the header's chrome surfaces are
/// one and eight cells across, the glyph marks one or two.
const GRAPH_PX: u32 = 200;
/// The graph's rows in pixels lie strictly between these: a rule, a key
/// cap or a phase segment is one row (20 or 32 px), the compact graph
/// two to four rows, and the stacked inspector ring at least seven rows.
const GRAPH_ROWS_PX: (u32, u32) = (39, 140);

/// Transmits of the trace graph, told apart from every other surface
/// by the `s=` and `v=` keys the first chunk of every transmit carries.
fn graph_transmits(capture: &Capture) -> usize {
    transmits_sized(capture, |width, height| {
        width >= GRAPH_PX && height > GRAPH_ROWS_PX.0 && height < GRAPH_ROWS_PX.1
    })
}

/// Transmits of an image whose `(width, height)` in pixels satisfy
/// `wanted`, read from the keys the first chunk of every transmit
/// carries.
fn transmits_sized(capture: &Capture, wanted: impl Fn(u32, u32) -> bool) -> usize {
    let bytes = capture.bytes();
    let mut found = 0;
    let mut at = 0;
    while let Some(start) = bytes[at..]
        .windows(TRANSMIT.len())
        .position(|w| w == TRANSMIT)
        .map(|offset| at + offset)
    {
        at = start + TRANSMIT.len();
        // The header runs to the `;` that opens the payload.
        let end = bytes[at..]
            .iter()
            .position(|b| *b == b';')
            .map_or(bytes.len(), |offset| at + offset);
        let header = &bytes[at..end];
        let key = |name: &[u8]| {
            header
                .windows(2)
                .position(|w| w == name)
                .map(|offset| &header[offset + 2..])
                .map(|rest| {
                    rest.iter()
                        .take_while(|b| b.is_ascii_digit())
                        .fold(0_u32, |n, b| n * 10 + u32::from(b - b'0'))
                })
                .unwrap_or(0)
        };
        if wanted(key(b"s="), key(b"v=")) {
            found += 1;
        }
    }
    found
}

/// Sixel images in the bytes written from `after` the run's first
/// write onward.
fn sixels_after(capture: &Capture, after: Duration) -> usize {
    let bytes = capture.bytes();
    let writes = capture.writes();
    let Some((start, _)) = writes.first() else {
        return 0;
    };
    let mut skip = 0;
    for (when, len) in &writes {
        if when.duration_since(*start) >= after {
            break;
        }
        skip += len;
    }
    bytes[skip..]
        .windows(SIXEL.len())
        .filter(|w| *w == SIXEL)
        .count()
}

fn options() -> Options {
    Options::new(Env::new("xterm-kitty", false, None)).probe_timeout(MS(500))
}

/// A think trace with one main step and one branch step, so the Trace
/// tab draws the branch graph.
fn branched_trace() -> FakeEngines {
    FakeEngines::new()
        .connected(MS(5))
        .frame(
            MS(8),
            "think",
            "step_appended",
            serde_json::json!({"step": {"step_number": 1, "purpose": "hypothesis"}}),
        )
        .frame(
            MS(9),
            "think",
            "step_appended",
            serde_json::json!({"step": {
                "step_number": 2,
                "purpose": "alternative",
                "branch_id": "b1",
                "branch_name": "alt",
                "branch_from": 1,
            }}),
        )
}

async fn run(script: Script, engines: FakeEngines) -> (Model, Capture) {
    let terminal = FakeTerminal::new(SIZE, script);
    let capture = terminal.capture();
    let runtime = Runtime::new(terminal, options());
    engines.play(runtime.inbox());
    let model = runtime
        .run(Model::new(SIZE))
        .await
        .expect("the run succeeds");
    (model, capture)
}

#[tokio::test]
async fn the_trace_graph_transmits_once_stays_quiet_while_idle_and_again_after_a_resize() {
    // Two tabs to Trace at 500 ms — past the sweep the arriving trace
    // starts, so what this test counts is
    // the caching and not the drawing-in — a key at 600 ms (paints a
    // frame, the graph unchanged), a resize at 700 ms (placements
    // forgotten, the graph re-sized), ctrl-U + ctrl-D at 850 ms.
    let script = Script::new()
        .startup_replies(MS(0))
        .text(MS(500), "\x1b[9;5u")
        .text(MS(600), "k")
        .resize(MS(700), Size::new(100, 30))
        .text(MS(850), "\x15\x04");
    let (model, capture) = run(script, branched_trace()).await;

    assert_eq!(model.tab(), StageTab::Session);
    assert!(model.trace().has_branches());
    // More frames were painted than transmits went out: the idle frames
    // (the key, the prompt clear) cost zero graphics bytes.
    assert!(model.frames() >= 4, "{} frames", model.frames());
    assert_eq!(
        graph_transmits(&capture),
        2,
        "one transmit for the first dirty graph, one for the resize"
    );
    assert!(
        capture.count(TRANSMIT) > graph_transmits(&capture),
        "the header's brand mark and activity sparkline transmit too"
    );
    assert!(
        capture.count(PLACE) >= 2,
        "the graph is placed and re-placed after the resize ({} placements)",
        capture.count(PLACE)
    );
    assert!(
        capture.count(DELETE) >= 1,
        "leaving the session releases the terminal's images"
    );
}

/// the trace graph draws itself in over
/// [`Motion::SWEEP_STEPS`] animation steps, so watching it arrive costs
/// one transmit per step — and then it stops. Bounded by the sweep, not
/// by the frames: an unbounded repaint would run to the quit.
#[tokio::test]
async fn the_trace_graph_sweeps_in_one_transmit_per_step_and_then_stops() {
    // On the Trace tab before the think frames land, so the whole sweep
    // is watched; then three quiet tenths of a second before the quit.
    let script = Script::new()
        .startup_replies(MS(0))
        .text(MS(2), "\x1b[9;5u")
        .text(MS(600), "\x15\x04");
    let (model, capture) = run(script, branched_trace()).await;

    assert_eq!(model.tab(), StageTab::Session);
    assert!(model.trace().has_branches());
    let transmits = graph_transmits(&capture);
    assert!(
        transmits >= 3,
        "the sweep draws the graph in over several frames ({transmits} transmits)"
    );
    assert!(
        transmits <= Motion::SWEEP_STEPS as usize + 2,
        "and stops when the sweep is over, not when the session is ({transmits} transmits over \
         {} frames)",
        model.frames()
    );
}

#[tokio::test]
async fn a_terminal_that_never_says_its_cell_size_still_gets_the_graph_on_the_default() {
    // Kitty OK and DA1 only: no `CSI 16 t` answer ever arrives, so the
    // surface is sized by the 10×20 default instead of staying invisible.
    let script = Script::new()
        .bytes(MS(0), b"\x1b_Gi=31;OK\x1b\\\x1b[?62;4c".as_slice())
        .text(MS(500), "\x1b[9;5u")
        .text(MS(700), "\x15\x04");
    let (model, capture) = run(script, branched_trace()).await;

    assert_eq!(model.tab(), StageTab::Session);
    assert_eq!(model.cell_pixels(), None, "the terminal never answered");
    assert_eq!(
        graph_transmits(&capture),
        1,
        "the graph transmits on the default cell size"
    );
}

#[tokio::test]
async fn a_late_cell_reply_resizes_the_graph_and_transmits_it_again() {
    // The `CSI 16 t` answer arrives 150 ms in, after the graph already
    // painted on the default: the surface adopts 16×32 px cells and the
    // repainted pixels go out once more.
    let script = Script::new()
        .bytes(MS(0), b"\x1b_Gi=31;OK\x1b\\\x1b[?62;4c".as_slice())
        .text(MS(500), "\x1b[9;5u")
        .bytes(MS(600), b"\x1b[6;32;16t".as_slice())
        .text(MS(800), "\x15\x04");
    let (model, capture) = run(script, branched_trace()).await;

    assert_eq!(model.cell_pixels(), Some((16, 32)), "the late reply landed");
    assert_eq!(
        graph_transmits(&capture),
        2,
        "the default-sized transmit, then the re-sized one"
    );
}

/// the header's chrome on both paths. With kitty the brand
/// mark and the activity sparkline are pixel surfaces and the cells
/// under them are blank; without a graphics protocol the very same
/// header is complete in cells — the brand glyph and the sparkline's
/// block ramp.
#[tokio::test]
async fn the_header_chrome_is_pixels_with_kitty_and_cells_without() {
    /// The brand slot as the cell path draws it.
    const BRAND: &[u8] = "▲ magistr".as_bytes();
    /// The brand slot with a pixel mark over its cell.
    const BLANK_BRAND: &[u8] = "  magistr".as_bytes();
    let script = || {
        Script::new()
            .startup_replies(MS(0))
            .text(MS(200), "\x15\x04")
    };
    let busy = || {
        FakeEngines::new().connected(MS(5)).frame(
            MS(8),
            "ship",
            "action_recorded",
            serde_json::json!({"action": {"description": "work"}}),
        )
    };

    let (model, capture) = run(script(), busy()).await;
    assert!(model.has_pixels(), "the probe found kitty");
    assert_eq!(
        capture.count(BRAND),
        0,
        "the brand's cell is left blank for the mark"
    );
    assert!(capture.count(BLANK_BRAND) > 0, "the slot is still a cell");
    // One transmit per header surface, none of them the graph's size.
    assert_eq!(graph_transmits(&capture), 0, "no graph on this run");
    assert!(
        capture.count(TRANSMIT) >= 2,
        "the brand mark and the activity sparkline both transmit ({})",
        capture.count(TRANSMIT)
    );

    // The same session on a terminal with no graphics protocol: DA1
    // ends the probe without listing sixel, and the kitty query goes
    // unanswered.
    // Long enough for the cell ramp to finish drawing itself in: with
    // no pixel layer the monitor sweeps as blocks, so its tallest cell
    // only lands once the sweep is over.
    let plain = Script::new()
        .bytes(MS(0), b"\x1b[?62;22c".as_slice())
        .text(MS(600), "\x15\x04");
    let terminal = FakeTerminal::new(SIZE, plain);
    let capture = terminal.capture();
    let runtime = Runtime::new(
        terminal,
        Options::new(Env::new("dumb", false, None)).probe_timeout(MS(200)),
    );
    busy().play(runtime.inbox());
    let model = runtime
        .run(Model::new(SIZE))
        .await
        .expect("the run succeeds");
    assert!(!model.has_pixels(), "no protocol, no pixel layer");
    assert_eq!(capture.count(TRANSMIT), 0, "not one graphics byte");
    assert!(capture.count(BRAND) > 0, "the brand glyph stands instead");
    let spark = Icons::UNICODE.spark(1.0).as_bytes();
    assert!(
        capture.count(spark) > 0,
        "and the sparkline is the block ramp"
    );
}

/// tui-image-blocks: an image the agent sends is pixels with kitty and
/// halfblocks without. The image is 40×40 px, which at the default
/// 10×20 cell is a 4×2 placement — 40×40 px of surface, told apart from
/// the header's chrome by its declared width.
#[tokio::test]
async fn a_stream_image_is_a_pixel_surface_with_kitty_and_halfblocks_without() {
    /// The halfblock row the cell path draws: four cells wide.
    const HALFBLOCKS: &[u8] = "▀▀▀▀".as_bytes();
    /// Every image at least this wide is the stream's, not the chrome's
    /// (the brand is one cell, the sparkline eight).
    const IMAGE_PX: u32 = 40;

    let mut pixmap = tiny_skia::Pixmap::new(40, 40).expect("a pixmap allocates");
    pixmap.fill(tiny_skia::Color::from_rgba8(200, 40, 40, 255));
    let png = pixmap.encode_png().expect("a pixmap encodes");
    let mut data = Vec::new();
    magistr_cockpit::base64::encode(&png, &mut data);
    let block = magistr_cockpit::agent::AgentEvent::Image(magistr_cockpit::agent::ImageBlock {
        mime_type: "image/png".to_owned(),
        data: String::from_utf8(data).expect("base64 is ascii"),
        uri: None,
    });

    let script = || {
        Script::new()
            .startup_replies(MS(0))
            .text(MS(300), "\x15\x04")
    };
    let sender = || {
        FakeAgent::new()
            .text(MS(10), "here it is:\n")
            .event(MS(20), block.clone())
    };

    let terminal = FakeTerminal::new(SIZE, script());
    let capture = terminal.capture();
    let runtime = Runtime::new(terminal, options());
    sender().play(runtime.inbox());
    let model = runtime
        .run(Model::new(SIZE))
        .await
        .expect("the run succeeds");

    assert!(model.has_pixels(), "the probe found kitty");
    assert_eq!(model.turn().len(), 3, "one prose row and two image rows");
    assert_eq!(
        capture.count(HALFBLOCKS),
        0,
        "the image's cells are left blank for the surface"
    );
    assert!(
        transmits_sized(&capture, |width, _| width >= IMAGE_PX) >= 1,
        "the image transmits as a surface of its own"
    );

    // The same stream on a terminal with no graphics protocol.
    let plain = Script::new()
        .bytes(MS(0), b"\x1b[?62;22c".as_slice())
        .text(MS(300), "\x15\x04");
    let terminal = FakeTerminal::new(SIZE, plain);
    let capture = terminal.capture();
    let runtime = Runtime::new(
        terminal,
        Options::new(Env::new("dumb", false, None)).probe_timeout(MS(200)),
    );
    sender().play(runtime.inbox());
    let model = runtime
        .run(Model::new(SIZE))
        .await
        .expect("the run succeeds");

    assert!(!model.has_pixels(), "no protocol, no pixel layer");
    assert_eq!(capture.count(TRANSMIT), 0, "not one graphics byte");
    assert!(
        capture.count(HALFBLOCKS) > 0,
        "the image is drawn in halfblocks instead"
    );
}

/// tui-image-tool-call-content: an image a *tool call* returned takes
/// the same route as one on a message chunk — its own line after the
/// call's card, composited as a pixel surface, and the completion
/// update re-sending the block leaves the row count exactly where it
/// was.
#[tokio::test]
async fn an_image_a_tool_call_returned_is_the_same_pixel_surface() {
    /// Every image at least this wide is the stream's, not the chrome's.
    const IMAGE_PX: u32 = 40;

    let mut pixmap = tiny_skia::Pixmap::new(40, 40).expect("a pixmap allocates");
    pixmap.fill(tiny_skia::Color::from_rgba8(40, 200, 90, 255));
    let png = pixmap.encode_png().expect("a pixmap encodes");
    let mut data = Vec::new();
    magistr_cockpit::base64::encode(&png, &mut data);
    let shot = magistr_cockpit::agent::ImageBlock {
        mime_type: "image/png".to_owned(),
        data: String::from_utf8(data).expect("base64 is ascii"),
        uri: None,
    };

    let terminal = FakeTerminal::new(
        SIZE,
        Script::new()
            .startup_replies(MS(0))
            .text(MS(300), "\x15\x04"),
    );
    let capture = terminal.capture();
    let runtime = Runtime::new(terminal, options());
    FakeAgent::new()
        .event(
            MS(10),
            magistr_cockpit::agent::AgentEvent::ToolCall {
                id: "tc-1".to_owned(),
                title: "browser screenshot".to_owned(),
                verb: magistr_cockpit::agent::ToolVerb::Tool,
                status: magistr_cockpit::agent::ToolStatus::InProgress,
                diff: None,
                images: vec![shot.clone()],
                record: Default::default(),
            },
        )
        .event(
            MS(20),
            magistr_cockpit::agent::AgentEvent::ToolCallUpdate {
                id: "tc-1".to_owned(),
                title: None,
                status: Some(magistr_cockpit::agent::ToolStatus::Completed),
                diff: None,
                duration_ms: Some(120),
                images: vec![shot],
                record: Default::default(),
            },
        )
        .play(runtime.inbox());
    let model = runtime
        .run(Model::new(SIZE))
        .await
        .expect("the run succeeds");

    assert!(model.has_pixels(), "the probe found kitty");
    assert_eq!(
        model.turn().len(),
        3,
        "the call's card and two image rows, after the update as before"
    );
    assert!(
        model.turn().image_at_row(1).is_some(),
        "the image is its own line under the card"
    );
    assert!(
        transmits_sized(&capture, |width, _| width >= IMAGE_PX) >= 1,
        "it transmits through the one surface path, like any stream image"
    );
}

/// the same cockpit on a sixel-only terminal. DA1 lists
/// parameter 4 and the kitty query goes unanswered, so `Mode::decide`
/// picks `Graphics(Sixel)` and every surface reaches the screen as a DCS
/// image at its own cell — no transmit, no placement, no delete.
#[tokio::test]
async fn a_sixel_only_terminal_gets_the_same_surfaces_as_a_dcs_image() {
    let script = Script::new()
        .bytes(MS(0), b"\x1b[?62;4c".as_slice())
        .text(MS(50), "\x1b[9;5u")
        .text(MS(300), "\x15\x04");
    let (model, capture) = run(script, branched_trace()).await;

    assert!(model.has_pixels(), "the probe found sixel");
    assert_eq!(model.tab(), StageTab::Session);
    assert!(
        capture.count(SIXEL) >= 1,
        "the trace graph and the header chrome go out as sixel images"
    );
    assert_eq!(capture.count(TRANSMIT), 0, "no kitty transmit");
    assert_eq!(capture.count(PLACE), 0, "sixel has no placements");
    assert_eq!(capture.count(DELETE), 0, "and nothing stored to delete");
}

#[tokio::test]
async fn an_idle_sixel_cockpit_re_emits_nothing() {
    // Everything happens at 500 ms and later — past the sweeps the
    // arriving surfaces draw themselves in over (the trace graph's,
    // and the header monitor's), so what
    // this counts is the caching and not the drawing-in.
    let script = Script::new()
        .bytes(MS(0), b"\x1b[?62;4c".as_slice())
        .text(MS(500), "\x1b[9;5u")
        .text(MS(600), "k")
        .text(MS(700), "j")
        .text(MS(850), "\x15\x04");
    let (model, capture) = run(script, branched_trace()).await;

    assert!(model.frames() >= 4, "{} frames", model.frames());
    let images = capture.count(SIXEL);
    assert!(images >= 1, "the surfaces painted once");
    // The tab switches at 500 ms vacate the surfaces the tab left — a
    // sixel image has no identity to delete, so the cells under it are
    // painted again and every surface goes out once more on the frame
    // after — and the `k` at 600 ms carries that frame. From then on
    // the cockpit is idle: `j` paints a frame, and not one image.
    assert_eq!(
        sixels_after(&capture, MS(650)),
        0,
        "{images} images in all — an idle frame costs nothing"
    );
}

/// the launcher's headline as one `OSC 66` run, over the two
/// rows the project card reserves, on a terminal that answered kitty's
/// text sizing handshake — and nothing at all on one that did not.
#[tokio::test]
async fn the_launcher_headline_is_one_osc_66_run_on_a_terminal_that_scales_text() {
    /// The opening of a scaled run.
    const SCALED: &[u8] = b"\x1b]66;s=2;";
    /// The probe's own scaled run: a single space, the handshake's
    /// second query. Every session writes it, whatever the answer.
    const QUERY: &[u8] = b"\x1b]66;s=2; \x07";
    /// The headline's run: the same opening, the project's name.
    const RUN: &[u8] = b"\x1b]66;s=2;magistr";
    /// The launcher grid needs ≥100 columns; below that the cards stack
    /// compact and carry no headline.
    const WIDE: Size = Size::new(120, 40);

    let launcher = || magistr_cockpit::cockpit::Msg::Job {
        id: magistr_cockpit::job::JobId::new(0),
        output: magistr_cockpit::job::JobOutput::Launcher(Box::new(
            magistr_cockpit::cockpit::LauncherData {
                project: "magistr".to_owned(),
                has_config: true,
                stack: "Rust / Cargo".to_owned(),
                git: "master, clean".to_owned(),
                backend: "ACP (available)".to_owned(),
                sessions: Vec::new(),
            },
        )),
    };
    let session = async |script: Script| {
        let terminal = FakeTerminal::new(WIDE, script);
        let capture = terminal.capture();
        let runtime = Runtime::new(terminal, options());
        runtime.inbox().send(launcher());
        let model = runtime
            .run(Model::new(WIDE).with_tab(StageTab::Home))
            .await
            .expect("the run succeeds");
        (model, capture)
    };

    let (model, capture) = session(
        // The handshake lands before DA1, which rides in
        // `startup_replies` and closes the probe's read window.
        Script::new()
            .text_sizing_replies(MS(0))
            .startup_replies(MS(1))
            .text(MS(200), "\x15\x04"),
    )
    .await;
    assert!(model.has_text_sizing(), "the handshake was answered");
    assert_eq!(model.headline(), Some("magistr"));
    assert_eq!(capture.count(QUERY), 1, "the handshake asked once");
    assert_eq!(
        capture.count(RUN),
        1,
        "one headline run, and it does not repeat across frames"
    );
    let bytes = capture.bytes();
    let at = bytes
        .windows(RUN.len())
        .position(|w| w == RUN)
        .expect("the run is on the wire");
    let tail = &bytes[at + SCALED.len()..];
    let end = tail
        .windows(2)
        .position(|w| w == b"\x1b\\")
        .expect("the run is terminated");
    assert!(
        std::str::from_utf8(&tail[..end])
            .expect("the payload is text")
            .starts_with("magistr"),
        "the project's name is the payload"
    );
    // The scale is off by the time the frame closes: the pen is reset
    // before ESU, so nothing after the run inherits it.
    let esu: &[u8] = b"\x1b[?2026l";
    let closes = tail[end..]
        .windows(esu.len())
        .position(|w| w == esu)
        .expect("the frame closes");
    assert!(
        tail[end..end + closes].windows(4).any(|w| w == b"\x1b[0m"),
        "the pen is reset between the run and ESU"
    );

    // The same session on a kitty terminal that swallowed the two runs:
    // the cards are the cells they always were.
    let (model, capture) = session(
        Script::new()
            .startup_replies(MS(0))
            .text(MS(200), "\x15\x04"),
    )
    .await;
    assert!(model.has_pixels(), "still kitty for graphics");
    assert!(!model.has_text_sizing(), "but no scaled text");
    assert!(model.headline().is_none());
    assert_eq!(capture.count(QUERY), 1, "the handshake still asked");
    assert_eq!(
        capture.count(SCALED) - capture.count(QUERY),
        0,
        "and nothing but the query is a scaled run"
    );
    assert!(
        capture.count(b"magistr") > 0,
        "the name is a cell row instead"
    );
}
