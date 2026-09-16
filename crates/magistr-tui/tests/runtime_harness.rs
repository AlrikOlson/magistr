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
    let root = view(&model, &FrameClock::new(0, Duration::ZERO));
    let rect = LayoutTree::new()
        .compute(&root, SIZE.cols, SIZE.rows)
        .expect("layout")
        .rect(id)
        .expect("the widget is laid out");
    assert!(!rect.is_empty(), "{id:?} covers cells");
    rect
}

/// on a kitty terminal the pointer becomes a hand over a tab,
/// stays one while it moves *within* the tab (one sequence, not one per
/// frame), and is handed back when it leaves for the header.
#[tokio::test]
async fn the_pointer_is_a_hand_over_a_tab_and_is_handed_back_when_it_leaves() {
    let tab = widget_rect(Widget::TabRoadmap.id());
    let script = Script::new()
        .startup_replies(MS(0))
        .mouse_move(MS(20), tab.x, tab.y)
        .mouse_move(MS(60), tab.x + tab.width - 1, tab.y)
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
    let tab = widget_rect(Widget::TabRoadmap.id());
    let script = Script::new()
        .startup_replies(MS(0))
        .mouse_move(MS(20), tab.x, tab.y)
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
    let tab = widget_rect(Widget::TabRoadmap.id());
    let script = Script::new()
        .plain_replies(MS(0))
        .mouse_move(MS(20), tab.x, tab.y)
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
    assert_eq!(model.keyboard_flags(), Some(3));
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
    assert!(model.frames() <= 8, "{} frames", model.frames());
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
    let runtime = Runtime::new(terminal, options());
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
    // The first frame, the delta, those steps, the turn end — and then
    // a whole second of nothing. The tick's period is fixed and the
    // sweep is a fixed number of steps, so this is a ceiling a slower
    // machine only comes in under.
    assert!(model.frames() <= 11, "{} frames", model.frames());
    assert_eq!(capture.count(BSU) as u64, model.frames());
}

/// a loop is live and nothing else at all
/// happens. The clocks still have to move, so the runtime keeps its tick
/// — at the clock's second rather than the motion layer's sixty
/// milliseconds. Over a quiet second and a half that is a frame or two,
/// not the two dozen a 60 ms tick would have cost.
#[tokio::test]
async fn a_quiet_running_loop_paints_at_the_clocks_rate_and_no_faster() {
    let quiet = MS(1500);
    let (model, capture) = run_with(
        Script::new()
            .startup_replies(MS(0))
            .text(MS(30) + quiet, "\x04"),
        |inbox| {
            tokio::spawn(async move {
                tokio::time::sleep(MS(30)).await;
                inbox.send(Msg::Loop(LoopEvent::Started {
                    iteration: 1,
                    max: 4,
                }));
            });
        },
    )
    .await;

    // One message, then silence: every step after it is the clock's own.
    let want = quiet.as_millis() / Animation::Clock.period().as_millis();
    let ticks = u128::from(model.ticks());
    assert!(
        (want / 2..=want + 2).contains(&ticks),
        "{ticks} steps over {quiet:?} at {:?} a step (wanted about {want})",
        Animation::Clock.period()
    );
    // What the chunk is about: the motion period would have cost this
    // many, and the reconciled set spends a small fraction of it.
    let motion = quiet.as_millis() / Model::TICK_PERIOD.as_millis();
    assert!(
        ticks * 4 < motion,
        "{ticks} steps is not far enough below the motion period's {motion}"
    );
    assert_eq!(capture.count(BSU) as u64, model.frames());
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
    assert_eq!(model.keyboard_flags(), None);
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

/// The screen the third column needs, and where it lands there — the
/// geometry `snap_undock_placement_180x50` pins.
const WIDE: Size = Size::new(180, 50);

/// a wheel over the undocked third column moves that
/// column's own rows and leaves the turn where it was. End to end,
/// because the routing depends on the driver reporting the column's
/// placed rect after layout — the model alone cannot know where the
/// column went.
#[tokio::test]
async fn a_wheel_over_the_undocked_column_moves_its_feed_and_not_the_turn() {
    let script = Script::new()
        .startup_replies(MS(0))
        // Well past the arrival transients the seed sets off, so the
        // cockpit is idle when the notch lands — and well past the
        // notch's own glide before the quit, so the model ends on the
        // window the scroll landed on rather than a frame still easing
        // towards it.
        .scroll_up(MS(400), 150, 20)
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
    inbox.send(Msg::Agent(magistr_cockpit::agent::AgentEvent::Text(lines)));
    inbox.send(Msg::Agent(magistr_cockpit::agent::AgentEvent::TurnEnded(
        magistr_cockpit::agent::TurnEnd::EndTurn,
    )));
    let model = runtime
        .run(Model::new(WIDE))
        .await
        .expect("the run succeeds");

    // One gutter cell in from the screen's edge on every side, the
    // pixel arrangement's frame.
    assert_eq!(
        model.undock_rect(),
        Some(magistr_cockpit::layout::Rect::new(131, 3, 48, 43)),
        "the driver reported where the column landed"
    );
    assert_eq!(
        model.undocked_tab(),
        Some(magistr_cockpit::cockpit::StageTab::Session)
    );
    assert_eq!(model.inputs(), 2, "the notch and the quit");
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
