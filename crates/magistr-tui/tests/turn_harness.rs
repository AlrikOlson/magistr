//! The turn stage end to end over the fake terminal: a
//! scripted agent streams into the cockpit while scripted keys and wheel
//! reports scroll it, and the byte stream, the model and the timing of
//! every paint are checked against what went in.

use std::time::{Duration, Instant};

use magistr_cockpit::agent::AgentEvent;
use magistr_cockpit::channel::Inbox;
use magistr_cockpit::cockpit::{Model, Size, TurnState};
use magistr_cockpit::turn::{RowKind, Viewport};
use magistr_tui::graphics::Env;
use magistr_tui::harness::{Capture, Deliveries, FakeAgent, FakeEngines, FakeTerminal, Script};
use magistr_tui::runtime::{Options, Runtime};

const MS: fn(u64) -> Duration = Duration::from_millis;
const SIZE: Size = Size::new(80, 24);
const BSU: &[u8] = b"\x1b[?2026h";

/// The screen a VT emulator shows after replaying every write that
/// happened before `until`.
fn screen_before(capture: &Capture, until: Instant) -> String {
    let bytes = capture.bytes();
    let mut end = 0;
    for (when, len) in capture.writes() {
        if when >= until {
            break;
        }
        end += len;
    }
    let mut parser = vt100::Parser::new(SIZE.rows, SIZE.cols, 0);
    parser.process(&bytes[..end]);
    parser.screen().contents()
}

fn options() -> Options {
    Options::new(Env::new("xterm-kitty", false, None)).probe_timeout(MS(500))
}

async fn run(script: Script, producers: impl FnOnce(Inbox)) -> (Model, Capture, Deliveries) {
    run_model(Model::new(SIZE), script, producers).await
}

async fn run_model(
    model: Model,
    script: Script,
    producers: impl FnOnce(Inbox),
) -> (Model, Capture, Deliveries) {
    let terminal = FakeTerminal::new(SIZE, script);
    let capture = terminal.capture();
    let deliveries = terminal.deliveries();
    let runtime = Runtime::new(terminal, options());
    producers(runtime.inbox());
    let model = runtime.run(model).await.expect("the run succeeds");
    (model, capture, deliveries)
}

#[tokio::test]
async fn a_ten_thousand_line_turn_scrolls_with_the_wheel_and_the_page_keys() {
    let lines: String = (0..10_000).map(|i| format!("line {i}\n")).collect();
    let script = Script::new()
        .startup_replies(MS(0))
        .page_up(MS(120))
        .scroll_up(MS(160), 40, 12)
        .scroll_up(MS(200), 40, 12)
        .scroll_down(MS(240), 40, 12)
        // Well past the last notch's glide, so the run ends on the
        // window the scrolls landed on rather than a frame of the paint
        // still easing after it. The glide in between is asserted below,
        // on the bytes the terminal actually received.
        .text(MS(800), "\x04");
    let (model, capture, deliveries) = run(script, |inbox| {
        FakeAgent::new()
            .event(MS(20), AgentEvent::Text(lines))
            .play(inbox);
    })
    .await;
    assert_eq!(model.turn().len(), 10_000);
    assert_eq!(model.turn_state(), TurnState::Streaming);
    assert_eq!(model.inputs(), 5, "PageUp, three wheel notches, q");
    let height = model.stage_rows();
    assert_eq!(
        height, 12,
        "24 rows less the chrome and the pixel arrangement's gutters"
    );
    // Tail 9988, a page (11) up, two notches (6) up, one notch (3) down.
    let top = 10_000 - height - (height - 1) - 3;
    assert_eq!(model.top_row(), top);
    assert!(!model.viewport().is_following());
    assert_eq!(model.visible_rows()[0].text, format!("line {top}"));
    assert_eq!(
        model.visible_rows()[height - 1].text,
        format!("line {}", top + height - 1)
    );
    // What a terminal showed: the tail before PageUp was delivered, the
    // scrolled window and its range at the end.
    let page_up = deliveries.instants()[1];
    let before = screen_before(&capture, page_up);
    assert!(before.contains("line 9999"), "before PageUp:\n{before}");
    assert!(before.contains(" streaming "), "{before}");
    // Before the quit was delivered: after it the leave sequence returns
    // the emulator to its main screen.
    let quit = *deliveries
        .instants()
        .last()
        .expect("the quit was delivered");
    // The scrolls reached the wire in order: the page's landing, two
    // notches up, one down — read off the header's row range, sampled
    // across the whole run finer than any input is spaced, so a loaded
    // machine shifts which frame a sample catches without changing the
    // windows it lands on. (The glide between two windows is finer than
    // this sampling; what is proven here is that every scroll painted,
    // and none was skipped or reordered.)
    let range = |screen: String| {
        screen.lines().find_map(|line| {
            let at = line.find("rows ")?;
            let words: Vec<&str> = line[at..].split_whitespace().take(2).collect();
            Some(words.join(" "))
        })
    };
    let mut landed: Vec<String> = Vec::new();
    for n in 0..40 {
        if let Some(window) = range(screen_before(&capture, page_up + MS(20 * n)))
            && landed.last() != Some(&window)
        {
            landed.push(window);
        }
    }
    let window = |top: usize| format!("rows {}–{}/10000", top + 1, top + height);
    let tail = 10_000 - height;
    assert_eq!(
        landed,
        vec![
            window(tail - (height - 1)),
            window(tail - (height - 1) - 3),
            window(tail - (height - 1) - 6),
            window(top),
        ],
        "the page and the three notches each painted their window, in order"
    );
    let after = screen_before(&capture, quit);
    assert!(
        after.contains(&format!("line {top}")),
        "at the end:\n{after}"
    );
    assert!(
        after.contains(&format!("line {}", top + height - 1)),
        "{after}"
    );
    assert!(!after.contains("line 9999"), "{after}");
    assert!(
        after.contains(&format!("rows {}–{}/10000", top + 1, top + height)),
        "{after}"
    );
    // The header's spinner paints a frame per tick of its own while the
    // turn streams; the frames the events cost are the coalescing bound.
    let frames = model.frames() - model.ticks();
    assert!(
        frames <= 8,
        "{frames} frames: the first, the turn, and one per scroll at most ({} steps)",
        model.ticks()
    );
    assert_eq!(capture.count(BSU) as u64, model.frames());
}

/// 200 lines a second for a second — with engine-feed cards landing in
/// the stream — while a key arrives every 20 ms: every line and card
/// lands, the frames coalesce, and the time from a key's delivery to
/// the next write stays under the bound — 20 ms at the 90th percentile
/// in release (the interactive tier ticks every 16 ms), a stall bound in
/// debug where the point is that nothing waits on the stream.
#[tokio::test]
async fn two_hundred_lines_a_second_keep_input_to_paint_under_the_bound() {
    const LINES: u32 = 200;
    const KEYS: u64 = 40;
    const CARDS: usize = 4;
    let mut script = Script::new().startup_replies(MS(0));
    for i in 0..KEYS {
        script = script.text(MS(100 + 20 * i), "k");
    }
    // The keys went into the prompt line; ctrl-U clears it so ctrl-D quits.
    let script = script.text(MS(1_250), "\x15\x04");
    let began = Instant::now();
    let (model, capture, deliveries) = run(script, |inbox| {
        let mut agent = FakeAgent::new();
        for i in 0..LINES {
            agent = agent.text(MS(30) + MS(5) * i, &format!("streamed line {i}\n"));
        }
        agent.play(inbox.clone());
        FakeEngines::new()
            .frame(
                MS(200),
                "ship",
                "task_started",
                serde_json::json!({"task": {"id": "t1", "title": "stream the turn"}}),
            )
            .frame(
                MS(400),
                "ship",
                "check_recorded",
                serde_json::json!({"check": {
                    "name": "cargo check", "passed": true, "exit_code": 0, "duration_ms": 900
                }}),
            )
            .frame(
                MS(600),
                "think",
                "step_appended",
                serde_json::json!({"step": {"step_number": 9, "purpose": "still streaming"}}),
            )
            .frame(
                MS(800),
                "ship",
                "check_recorded",
                serde_json::json!({"check": {
                    "name": "cargo nextest run", "passed": false, "exit_code": 100,
                    "duration_ms": 1500, "details": "1 failed"
                }}),
            )
            .play(inbox);
    })
    .await;
    assert!(
        began.elapsed() < MS(3_000),
        "the run took {:?}",
        began.elapsed()
    );
    assert_eq!(
        model.turn().len(),
        LINES as usize + CARDS,
        "every line and card arrived"
    );
    assert!(
        model
            .turn()
            .rows()
            .iter()
            .any(|r| r.kind == RowKind::Check(false)),
        "the failed check card sits in the stream"
    );
    assert_eq!(model.inputs(), KEYS + 2, "the keys, ctrl-U and q");
    let frames = model.frames();
    assert!(
        frames <= 150,
        "{frames} frames for 200 deltas and 40 keys: the loop coalesces"
    );
    assert_eq!(capture.count(BSU) as u64, frames);

    let writes = capture.writes();
    let instants = deliveries.instants();
    // The first delivery is the startup replies, the last the quit.
    let keys = &instants[1..instants.len() - 1];
    assert_eq!(keys.len(), KEYS as usize);
    let mut latencies: Vec<Duration> = keys
        .iter()
        .map(|&at| {
            writes
                .iter()
                .find(|(when, _)| *when >= at)
                .map(|(when, _)| when.duration_since(at))
                .expect("a frame was painted after the key")
        })
        .collect();
    latencies.sort();
    let p90 = latencies[latencies.len() * 9 / 10];
    let max = latencies[latencies.len() - 1];
    let (p90_bound, max_bound) = if cfg!(debug_assertions) {
        (MS(100), MS(200))
    } else {
        (MS(20), MS(40))
    };
    assert!(
        p90 < p90_bound,
        "input-to-paint p90 {p90:?} (max {max:?}) exceeds {p90_bound:?}"
    );
    assert!(
        max < max_bound,
        "input-to-paint max {max:?} exceeds {max_bound:?}"
    );
}

/// A click on a tool row expands it to the full title; Enter on the
/// empty prompt collapses it again.
#[tokio::test]
async fn a_tool_row_expands_by_click_and_collapses_by_enter() {
    const TITLE: &str = "cargo nextest run --workspace --exclude magistr-gui --all-targets \
                         -E 'test(turn_stage)'";
    // Rows on the kitty arrangement: gutter 0, header 1, strip 2, the
    // panel frame 3, tab bar 4, stage padding 5, turn rows from 6 — the
    // tool call is the turn's second row, on screen row 7. Columns: the
    // gutter, the rail and its divider take 30, so the stage's rows
    // start at 31 — the click lands well inside the row's text.
    let script = Script::new()
        .startup_replies(MS(0))
        .click(MS(120), 36, 7)
        .text(MS(200), "\r")
        .bytes(MS(280), b"\x04".as_slice());
    let (model, capture, deliveries) = run(script, |inbox| {
        FakeAgent::new()
            .event(MS(10), AgentEvent::Text("before\n".to_owned()))
            .tool_call(MS(20), "c1", TITLE)
            .event(MS(30), AgentEvent::Text("after\n".to_owned()))
            .play(inbox);
    })
    .await;
    let instants = deliveries.instants();
    assert_eq!(instants.len(), 4, "startup, click, enter, ctrl-D");
    // The expanded title is broken by columns over several rows, so the
    // tail is looked for with the row breaks and the gutters squashed
    // out — where the break falls depends on the stage's width, and
    // the claim is about the words, not the fold.
    let squashed = |screen: &str| {
        screen
            .chars()
            .filter(|c| !c.is_whitespace() && !matches!(c, '│' | '▎'))
            .collect::<String>()
    };
    let before_click = screen_before(&capture, instants[1]);
    assert!(
        !squashed(&before_click).contains("turn_stage"),
        "collapsed: the cut title ends before its tail:\n{before_click}"
    );
    let expanded = screen_before(&capture, instants[2]);
    assert!(
        squashed(&expanded).contains("turn_stage"),
        "the click expanded the call to its full title:\n{expanded}"
    );
    let collapsed = screen_before(&capture, instants[3]);
    assert!(
        !squashed(&collapsed).contains("turn_stage"),
        "enter collapsed it again:\n{collapsed}"
    );
    assert_eq!(model.turn().len(), 3, "back to one row per line");
}

/// The prompt editor end to end: `@` opens the file popup on
/// the real event path, Tab accepts the highlighted candidate into the
/// prompt, and a bracketed paste with a newline lands verbatim — the
/// box grows, nothing is sent.
#[tokio::test]
async fn the_file_popup_completes_and_a_paste_grows_the_prompt_without_sending() {
    let script = Script::new()
        .startup_replies(MS(0))
        .text(MS(100), "read @pro")
        .text(MS(160), "\t")
        .bytes(
            MS(220),
            b"\x1b[200~first line\nsecond line\x1b[201~".as_slice(),
        )
        .text(MS(300), "\x15\x7f\x15\x04");
    let files = vec!["src/cockpit/prompt.rs".to_owned(), "src/main.rs".to_owned()];
    let (model, capture, deliveries) = run_model(
        Model::new(SIZE).with_workspace_files(files),
        script,
        |_inbox| {},
    )
    .await;
    let instants = deliveries.instants();
    assert_eq!(instants.len(), 5, "startup, @pro, tab, paste, quit");
    let open = screen_before(&capture, instants[2]);
    assert!(
        open.contains("src/cockpit/prompt.rs"),
        "the popup lists the match while open:\n{open}"
    );
    assert!(
        open.contains(" ↑↓  select   enter  accept   esc  close"),
        "{open}"
    );
    let accepted = screen_before(&capture, instants[3]);
    assert!(
        accepted.contains("› read @src/cockpit/prompt.rs"),
        "tab put the file into the prompt:\n{accepted}"
    );
    assert!(
        !accepted.contains("↑↓ select"),
        "the popup closed on accept:\n{accepted}"
    );
    let pasted = screen_before(&capture, instants[4]);
    assert!(
        pasted.contains("› read @src/cockpit/prompt.rs first line"),
        "{pasted}"
    );
    assert!(
        pasted.contains("  second line"),
        "the pasted newline grew the box:\n{pasted}"
    );
    // The clean-up killed line two, the newline, then line one — three
    // line-scoped strokes — so ctrl-D could quit; nothing was ever sent.
    assert!(model.prompt().is_empty());
    assert!(model.turn().is_empty(), "no turn was sent");
    assert_eq!(
        model.inputs(),
        15,
        "9 keys, tab, one paste, ctrl-U, backspace, ctrl-U, ctrl-D"
    );
}

/// stage-scroll-anchor-and-rate, the terminal's half: a wheel notch is
/// on screen the frame it was turned — a cell surface holds each frame
/// for a tick, so the paint reads the glide a step ahead — and reaches
/// its rows well inside the glide's two ticks (120 ms), with no frame
/// between showing a window that is neither the one it left nor the
/// one it landed on: no whole-row strobe. Measured on the bytes the
/// terminal received, from the notch's delivery to the first write
/// after it.
#[tokio::test]
#[cfg_attr(
    windows,
    ignore = "Windows timers tick at ~16 ms: under a parallel test load a tick due before the notch lands after it and the first write after delivery is the pre-notch frame; passes in isolation"
)]
async fn a_wheel_notch_is_on_screen_the_frame_it_was_turned_and_never_strobes() {
    let lines: String = (0..10_000).map(|i| format!("line {i}\n")).collect();
    let notch = MS(400);
    let script = Script::new()
        .startup_replies(MS(0))
        .page_up(MS(120))
        .scroll_up(notch, 40, 12)
        .text(MS(800), "\x04");
    let (model, capture, deliveries) = run(script, |inbox| {
        FakeAgent::new()
            .event(MS(20), AgentEvent::Text(lines))
            .play(inbox);
    })
    .await;
    let height = model.stage_rows();
    assert_eq!(
        height, 12,
        "24 rows less the chrome and the pixel arrangement's gutters"
    );
    // The window the page left the stage on, and the notch's own.
    let from = 10_000 - height - (height - 1);
    let to = from - Viewport::WHEEL;
    let range = |top: usize| format!("rows {}–{}/10000", top + 1, top + height);
    assert_eq!(model.top_row(), to);
    let delivered = deliveries.instants()[2];
    let (first_after, _) = capture
        .writes()
        .into_iter()
        .find(|(when, _)| *when >= delivered)
        .expect("a frame after the notch");
    // Input to settled: the first frame after the delivery is the
    // notch's window, and it went out inside the glide's own duration
    // (a stall bound in debug; the claim is the frame, not the clock).
    let first = screen_before(&capture, first_after + Duration::from_nanos(1));
    assert!(
        first.contains(&range(to)),
        "the first frame after the notch shows its window:\n{first}"
    );
    let latency = first_after.duration_since(delivered);
    assert!(
        latency <= MS(120),
        "input to settled {latency:?} — over the glide's two ticks"
    );
    // And no frame from the delivery on shows any other window: the one
    // it left, then the one it landed on, never a row between.
    let strobe: Vec<String> = (0..20)
        .map(|n| screen_before(&capture, delivered + MS(10 * n)))
        .filter_map(|screen| {
            screen
                .lines()
                .find(|line| line.contains("rows "))
                .map(str::to_owned)
        })
        .filter(|line| !line.contains(&range(from)) && !line.contains(&range(to)))
        .collect();
    assert!(
        strobe.is_empty(),
        "rows painted between the windows: {strobe:#?}"
    );
    assert_eq!(capture.count(BSU) as u64, model.frames());
}
