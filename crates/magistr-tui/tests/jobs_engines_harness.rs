//! The producers end to end over the fake terminal: a scripted
//! agent, scripted engines and a real job run against the cockpit, and
//! the frame count proves the loop coalesces their bursts and never waits
//! on a job.

use std::time::Duration;

use magistr_application::sessions::{AgentEvent, TurnEnd};
use magistr_cockpit::cockpit::{Model, Size};
use magistr_cockpit::job::{JobId, JobOutput};
use magistr_cockpit_runtime::Jobs;
use magistr_cockpit_runtime::channel::Inbox;
use magistr_engine::FeedStatus;
use magistr_tui::graphics::Env;
use magistr_tui::harness::{Capture, FakeAgent, FakeEngines, FakeTerminal, Script};
use magistr_tui::runtime::{Options, Runtime};

const MS: fn(u64) -> Duration = Duration::from_millis;
const SIZE: Size = Size::new(80, 24);
const BSU: &[u8] = b"\x1b[?2026h";

fn assert_pixel_region_changed(
    capture: &Capture,
    xs: std::ops::Range<u32>,
    ys: std::ops::Range<u32>,
) {
    let writes = capture.writes();
    let sample = |at| {
        let mut pixels = Vec::new();
        for p in capture.pixels_before(at) {
            for y in 0..p.height {
                let sy = u32::from(p.row) * 20 + y;
                if !ys.contains(&sy) {
                    continue;
                }
                for x in xs.clone() {
                    let sx = x.saturating_sub(u32::from(p.col) * 10);
                    if sx < p.width {
                        let offset = ((y * p.width + sx) * 4) as usize;
                        pixels.extend_from_slice(&p.rgba[offset..offset + 4]);
                    }
                }
            }
        }
        pixels
    };
    let before = sample(writes[2].0);
    let after = sample(writes.last().expect("exit write").0);
    assert!(
        !before.is_empty() && !after.is_empty(),
        "graphics decoded from both paints"
    );
    assert!(
        before != after,
        "the engine update changed the requested screen region"
    );
}

fn options() -> Options {
    Options::new(Env::new("xterm-kitty", false, None)).probe_timeout(MS(500))
}

/// Run `script` on the fake terminal with `producers` attached to the
/// runtime's inbox before the run starts.
async fn run(script: Script, producers: impl FnOnce(Inbox)) -> (Model, Capture) {
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

#[tokio::test]
async fn a_hundred_agent_deltas_in_ten_milliseconds_cost_at_most_two_frames() {
    let script = Script::new().startup_replies(MS(0)).text(MS(250), "\x04");
    let (model, capture) = run(script, |inbox| {
        FakeAgent::new()
            .deltas(MS(30), 100, Duration::from_micros(100), "tok")
            .turn_ended(MS(30) + Duration::from_micros(100 * 100))
            .play(inbox);
    })
    .await;
    assert_eq!(
        model.agent_events(),
        101,
        "every delta and the turn end arrived"
    );
    let frames = model.frames();
    assert!(frames >= 1, "the first frame is painted");
    let fade = u128::from(magistr_cockpit::cockpit::Motion::ARRIVAL_STEPS)
        * Model::TICK_PERIOD.as_millis();
    let frame_period = magistr_tui::runtime::Tier::Interactive.period().as_millis();
    let budget = 3 + fade.div_ceil(frame_period);
    assert!(
        u128::from(frames) <= budget,
        "{frames} frames: the initial frame, coalesced burst and bounded pixel fade ({budget})"
    );
    assert_eq!(capture.count(BSU) as u64, frames);
}

#[tokio::test]
async fn a_three_hundred_millisecond_job_sees_at_least_five_frames_painted_meanwhile() {
    // Keys every 30 ms keep the cockpit painting while the job runs; the
    // job result records the frame it landed on.
    let mut script = Script::new().startup_replies(MS(0));
    for i in 0..11 {
        script = script.text(MS(40 + 30 * i), "k");
    }
    // The keys went into the prompt line; ctrl-U clears it so `q` quits.
    let script = script.text(MS(450), "\x15\x04");
    let began = std::time::Instant::now();
    let (model, _) = run(script, |inbox| {
        let jobs = Jobs::new(inbox);
        tokio::spawn(async move {
            tokio::time::sleep(MS(20)).await;
            jobs.spawn_job(|| {
                std::thread::sleep(MS(300));
                JobOutput::Text("gate green".to_owned())
            });
        });
    })
    .await;
    assert!(
        began.elapsed() < MS(1500),
        "the run took {:?}: the loop waited on something",
        began.elapsed()
    );
    assert_eq!(model.inputs(), 13, "eleven keys, ctrl-U and q");
    assert_eq!(model.jobs_done(), 1);
    assert_eq!(
        model.last_job(),
        Some(&(JobId::new(0), JobOutput::Text("gate green".to_owned())))
    );
    let painted_meanwhile = model.last_job_frame().expect("the job landed");
    assert!(
        painted_meanwhile >= 5,
        "only {painted_meanwhile} frames were painted before the job's result landed"
    );
    assert!(model.frames() >= painted_meanwhile);
}

#[tokio::test]
async fn scripted_engines_are_counted_and_the_feed_status_kept() {
    let script = Script::new().startup_replies(MS(0)).text(MS(200), "\x04");
    let (model, _) = run(script, |inbox| {
        FakeEngines::new()
            .feed(MS(5), FeedStatus::Waiting)
            .connected(MS(20))
            .frame(
                MS(30),
                "roadmap",
                "chunk_changed",
                serde_json::json!({"chunk": {"id": "a"}}),
            )
            .frame(
                MS(31),
                "ship",
                "action_recorded",
                serde_json::json!({"action": {"id": 1}}),
            )
            .frame(
                MS(32),
                "think",
                "step_appended",
                serde_json::json!({"step": {"step_number": 135}}),
            )
            .feed(MS(60), FeedStatus::Disconnected)
            .play(inbox);
    })
    .await;
    assert_eq!(model.engine_frames(), 3);
    assert_eq!(model.feed(), Some(FeedStatus::Disconnected));
    assert_eq!(model.inputs(), 1);
}

#[tokio::test]
async fn engine_frames_reach_the_rail_and_its_next_paint() {
    let script = Script::new().startup_replies(MS(0)).text(MS(200), "\x04");
    let (model, capture) = run(script, |inbox| {
        FakeEngines::new()
            .connected(MS(5))
            .frame(
                MS(10),
                "ship",
                "objective_set",
                serde_json::json!({"objective": {"description": "polish the rail"}}),
            )
            .frame(
                MS(11),
                "ship",
                "task_added",
                serde_json::json!({"task_id": "t1", "title": "wire the dock"}),
            )
            .frame(
                MS(12),
                "ship",
                "task_started",
                serde_json::json!({"task_id": "t1"}),
            )
            .frame(
                MS(13),
                "roadmap",
                "chunk_changed",
                serde_json::json!({"chunk": {"id": "tui2-2a1-rail", "status": "in_progress"}}),
            )
            .frame(
                MS(14),
                "signal",
                "signal_captured",
                serde_json::json!({"signal": {"id": "s1", "status": "new"}}),
            )
            .play(inbox);
    })
    .await;
    assert_eq!(model.rail().ship().objective(), Some("polish the rail"));
    assert_eq!(model.rail().ship().tasks().len(), 1);
    assert_eq!(model.rail().chunk(), Some("tui2-2a1-rail"));
    assert_eq!(model.rail().signals_pending(), 1);
    assert_eq!(model.engine_frames(), 5);
    assert!(
        model.frames() >= 2,
        "{} frames: the first paint plus one for the burst",
        model.frames()
    );
    // The signals line moved down a row when the task appeared, so its
    // new row paints over blank cells — one contiguous run in the stream.
    // (Rows repainted in place diff cell by cell and split their runs.)
    assert_pixel_region_changed(&capture, 10..280, 80..260);
}

#[tokio::test]
async fn producers_learn_the_loop_is_gone_after_the_run() {
    // Allow the initial raster frame to finish before quitting, as in the other
    // runtime cases. The producer events must precede shutdown, not race it.
    let script = Script::new().startup_replies(MS(0)).text(MS(200), "\x04");
    let mut kept = None;
    let (model, _) = run(script, |inbox| {
        FakeAgent::new()
            .text(MS(5), "before")
            .event(MS(10), AgentEvent::TurnEnded(TurnEnd::EndTurn))
            .play(inbox.clone());
        kept = Some(inbox);
    })
    .await;
    assert_eq!(model.agent_events(), 2);
    let inbox = kept.expect("kept");
    assert!(inbox.is_closed());
    assert!(
        !inbox.send(magistr_cockpit::cockpit::Msg::Agent(AgentEvent::Text(
            "late".to_owned()
        )))
    );
}

#[tokio::test]
async fn a_think_step_updates_the_recorder_without_replacing_the_conversation() {
    let script = Script::new().startup_replies(MS(0)).text(MS(200), "\x04");
    let (model, capture) = run(script, |inbox| {
        FakeEngines::new()
            .connected(MS(5))
            .frame(
                MS(60),
                "think",
                "step_appended",
                serde_json::json!({"step": {
                    "step_number": 7, "purpose": "hypothesis", "confidence": 0.7}}),
            )
            .play(inbox);
    })
    .await;
    assert_eq!(model.trace().steps().len(), 1, "the step landed");
    assert_eq!(model.trace().steps()[0].number, 7);
    use magistr_cockpit::cockpit::{FrameClock, Widget};
    let layout = magistr_tui::screenshot::layout(
        &model,
        &FrameClock::new(0, Duration::ZERO),
        magistr_tui::screenshot::Profile::Kitty,
    )
    .unwrap();
    assert!(
        !model.reasoning_expanded(),
        "arrival keeps reasoning minimized"
    );
    assert!(layout.px(Widget::TraceGraph.id()).is_none());
    let graph = layout
        .px(Widget::Rail.id())
        .expect("reasoning heading and updated count stay visible");
    assert!(
        layout.px(Widget::Stage.id()).is_some(),
        "conversation stays visible"
    );
    assert_pixel_region_changed(
        &capture,
        graph.x.floor() as u32..graph.right().ceil() as u32,
        graph.y.floor() as u32..graph.bottom().ceil() as u32,
    );
}

#[tokio::test]
async fn phase_changes_and_needs_you_cards_speak_on_the_screen_reader_channel() {
    let script = Script::new().startup_replies(MS(0)).text(MS(250), "\x04");
    let (model, capture) = run(script, |inbox| {
        FakeEngines::new()
            .connected(MS(5))
            .frame(
                MS(30),
                "ship",
                "objective_set",
                serde_json::json!({"objective": {"description": "goal"}}),
            )
            // The same phase again must stay silent.
            .frame(
                MS(60),
                "ship",
                "objective_set",
                serde_json::json!({"objective": {"description": "goal"}}),
            )
            .frame(
                MS(90),
                "roadmap",
                "chunk_changed",
                serde_json::json!({"chunk": {
                    "id": "c1",
                    "status": "pending",
                    "title_proposal": {"suggested_title": "better", "reason": "clearer"},
                }}),
            )
            .play(inbox);
    })
    .await;
    assert_eq!(model.engine_frames(), 3, "every frame arrived");
    assert_eq!(
        capture.count("\x1b]9;phase plan\x07".as_bytes()),
        1,
        "the phase change was announced exactly once"
    );
    assert_eq!(
        capture
            .count("\x1b]9;needs you: \u{25c6} c1: title \u{2192} better (clearer)\x07".as_bytes()),
        1,
        "the new Needs-you card was announced"
    );
}
