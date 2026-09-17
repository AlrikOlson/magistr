use magistr_engine::FeedStatus;

use super::*;
use crate::cockpit::{Msg, Size, update};
use crate::engine::EngineEvent;
use crate::input::{KeyCode, KeyEvent};
use crate::job::JobId;
use crate::theme::ThemeMode;

#[test]
fn a_fresh_model_summarises_to_two_lines_of_unknowns_and_zeros() {
    let model = Model::new(Size::new(80, 24));
    assert_eq!(
        Summary::of(&model).to_string(),
        "magistr cockpit — 80×24 cells · theme dark · background unknown · keyboard press only\n\
         0 frames · 0 inputs · 0 agent events · turn idle (0 rows) · 0 engine frames (no feed) · 0 jobs"
    );
}

#[test]
fn the_summary_carries_what_the_terminal_answered_and_what_the_cockpit_counted() {
    let mut model = Model::new(Size::new(100, 40)).with_theme(Theme::forced(ThemeMode::Light));
    update(&mut model, Msg::Background(Color::Rgb(30, 30, 46)));
    update(&mut model, Msg::KeyReleases(true));
    update(
        &mut model,
        Msg::CellSize(Some(crate::pixel::CellSize {
            width: 10,
            height: 20,
        })),
    );
    for _ in 0..3 {
        update(&mut model, Msg::FrameDue);
    }
    update(&mut model, Msg::Key(KeyEvent::new(KeyCode::Char('x'))));
    update(
        &mut model,
        Msg::Engine(EngineEvent::Feed(FeedStatus::Connected { reconnect: true })),
    );
    update(
        &mut model,
        Msg::Job {
            id: JobId::new(4),
            output: JobOutput::Text("gate green".to_owned()),
        },
    );
    let summary = Summary::of(&model);
    assert_eq!(summary.theme(), "light (MAGISTR_THEME)");
    assert_eq!(summary.background(), "#1e1e2e");
    assert_eq!(summary.keyboard(), "press and release");
    assert_eq!(summary.cell().as_deref(), Some("cell 10×20px"));
    assert_eq!(summary.feed(), "feed reattached");
    assert_eq!(summary.last_job().as_deref(), Some("job 4: gate green"));
    assert_eq!(
        summary.to_string(),
        "magistr cockpit — 100×40 cells · theme light (MAGISTR_THEME) · background #1e1e2e · keyboard press and release · cell 10×20px\n\
         3 frames · 1 inputs · 0 agent events · turn idle (0 rows) · 0 engine frames (feed reattached) · 1 jobs · last job 4: gate green"
    );
}

#[test]
fn a_failed_job_and_a_lost_feed_read_as_such() {
    let mut model = Model::new(Size::new(80, 24));
    update(
        &mut model,
        Msg::Engine(EngineEvent::Feed(FeedStatus::Disconnected)),
    );
    update(
        &mut model,
        Msg::Job {
            id: JobId::new(0),
            output: JobOutput::Failed("boom".to_owned()),
        },
    );
    let summary = Summary::of(&model);
    assert_eq!(summary.feed(), "feed lost");
    assert_eq!(summary.last_job().as_deref(), Some("job 0 failed: boom"));
    assert_eq!(summary.theme(), "dark");
}

#[test]
fn the_turn_state_and_its_rows_are_summarised() {
    use magistr_application::sessions::{AgentEvent, TurnEnd};
    let mut model = Model::new(Size::new(80, 24));
    update(
        &mut model,
        Msg::Agent(AgentEvent::Text("one\ntwo".to_owned())),
    );
    assert_eq!(Summary::of(&model).turn(), "turn streaming (2 rows)");
    update(
        &mut model,
        Msg::Agent(AgentEvent::TurnEnded(TurnEnd::EndTurn)),
    );
    assert_eq!(Summary::of(&model).turn(), "turn awaiting input (2 rows)");
    assert!(
        Summary::of(&model)
            .to_string()
            .contains("2 agent events · turn awaiting input (2 rows) ·")
    );
}

#[test]
fn a_loop_adds_its_own_line() {
    use std::time::Duration;

    use crate::cockpit::LoopEvent;
    use crate::job::JobOutput;
    let mut model = Model::new(Size::new(80, 24));
    assert_eq!(Summary::of(&model).loop_status(), None);
    update(
        &mut model,
        Msg::Loop(LoopEvent::Started {
            iteration: 1,
            max: 2,
        }),
    );
    assert_eq!(
        Summary::of(&model).loop_status().as_deref(),
        Some("loop iteration 1/2 · 0s · running")
    );
    update(
        &mut model,
        Msg::Loop(LoopEvent::GateStarted {
            name: "check".to_owned(),
            job: JobId::new(0),
        }),
    );
    update(
        &mut model,
        Msg::Job {
            id: JobId::new(0),
            output: JobOutput::Text("ok".to_owned()),
        },
    );
    update(
        &mut model,
        Msg::Loop(LoopEvent::Elapsed(Duration::from_secs(61))),
    );
    update(&mut model, Msg::Key(KeyEvent::new(KeyCode::Char(' '))));
    assert_eq!(
        Summary::of(&model).loop_status().as_deref(),
        Some("loop iteration 1/2 · ✓ check · 1m 01s · paused")
    );
    update(
        &mut model,
        Msg::Loop(LoopEvent::Ended("aborted".to_owned())),
    );
    let text = Summary::of(&model).to_string();
    assert!(
        text.ends_with("\nloop iteration 1/2 · ✓ check · 1m 01s · ended: aborted"),
        "{text}"
    );
    assert_eq!(text.lines().count(), 3);
}

#[test]
fn the_loop_line_carries_the_judges_count_and_last_reason() {
    use crate::cockpit::LoopEvent;
    let mut model = Model::new(Size::new(80, 24));
    update(
        &mut model,
        Msg::Loop(LoopEvent::Started {
            iteration: 1,
            max: 2,
        }),
    );
    update(
        &mut model,
        Msg::Loop(LoopEvent::Verdict {
            iteration: 1,
            ok: false,
            impossible: false,
            reason: "no test for the timeout".to_owned(),
        }),
    );
    assert_eq!(
        Summary::of(&model).loop_status().as_deref(),
        Some(
            "loop iteration 1/2 · 0s · running · judge (1 verdicts): not yet — no test for the timeout"
        )
    );
}
