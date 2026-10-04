use std::time::Duration;

use magistr_domain::workflow::CheckpointReason;
use serde_json::json;

use super::*;
use crate::cockpit::{
    AgentMissing, Budget, CheckpointEvent, Cmd, LoopEvent, LoopStage, LoopStatus, Msg,
    SettingsGroup, Size, StageTab, StarState, Surface, update,
};
use crate::engine::EngineEvent;
use crate::input::{Button, KeyCode, KeyEvent, MouseEvent, MouseKind};
use crate::job::{JobId, JobOutput};
use crate::widget_id::WidgetId;
use magistr_application::sessions::{AgentEvent, TurnEnd};

/// A window on the Sky with `e6b` on the rail — the room every fault
/// in this file is read on, since the cards are native only.
fn sky() -> Model {
    let mut model = Model::new(Size::new(120, 40))
        .with_surface(Surface::Native)
        .with_tab(StageTab::Roadmap);
    update(
        &mut model,
        Msg::Engine(EngineEvent::Seed(
            magistr_workflow_contracts::NativeSnapshot {
                progress: crate::harness::fixtures::progress(211, 306),
                chunks: crate::harness::fixtures::chunks_from(vec![
                    json!({"id": "layout", "name": "layout", "status": "done", "priority": 200, "deps": []}),
                    json!({"id": "e6b", "name": "e6b", "status": "in_progress", "priority": 300, "deps": ["layout"]}),
                ]),
                ..magistr_workflow_contracts::NativeSnapshot::default()
            },
        )),
    );
    model
}

fn looped(model: &mut Model, elapsed: u64) {
    update(
        model,
        Msg::Loop(LoopEvent::Started {
            iteration: 3,
            max: 10,
        }),
    );
    update(
        model,
        Msg::Loop(LoopEvent::Elapsed(Duration::from_secs(elapsed))),
    );
}

fn notes(model: &Model) -> String {
    model
        .turn()
        .rows()
        .iter()
        .filter(|r| r.kind == crate::turn::RowKind::Note)
        .map(|r| r.text.as_str())
        .collect::<Vec<_>>()
        .join(" ")
}

/// A press on `target`, less the animation tick the fold answers with.
fn click(model: &mut Model, target: WidgetId) -> Vec<Cmd> {
    update(
        model,
        Msg::Mouse {
            event: MouseEvent::new(MouseKind::Down(Button::Left), 0, 0),
            target: Some(target),
        },
    )
    .into_iter()
    .filter(|cmd| !matches!(cmd, Cmd::Animate(_) | Cmd::Still))
    .collect()
}

#[test]
fn a_quiet_model_has_no_bad_day() {
    let model = sky();
    assert!(model.bad_day().is_none());
    assert_eq!(model.star_note("e6b"), None);
}

#[test]
fn a_terminal_keeps_its_errors_on_the_stage() {
    let model = Model::new(Size::new(120, 40)).with_agent_missing(AgentMissing::new("claude"));
    assert!(
        model.bad_day().is_none(),
        "the cards are §1's overlay class; a terminal has no altitude for them"
    );
}

#[test]
fn the_missing_agent_blocks_and_rings_the_chunk_on_the_rail() {
    let model = sky().with_agent_missing(AgentMissing::new("claude").seen("09:14"));
    let day = model.bad_day().expect("a bad day");
    assert_eq!(day.blocking(), 1);
    let fault = &day.faults()[0];
    assert_eq!(fault.severity(), Severity::Alert);
    assert_eq!(
        fault.cap(),
        "Blocking \u{b7} New sessions cannot start \u{b7} Running sessions continue"
    );
    assert_eq!(fault.title(), Some("Agent binary not found"));
    assert_eq!(fault.text(), "claude is not on PATH. Last detected 09:14.");
    assert_eq!(fault.star(), Some(("e6b", "Requires agent")));
    let verbs: Vec<(&str, bool)> = fault
        .verbs()
        .iter()
        .map(|v| (v.verb(), v.is_lit()))
        .collect();
    assert_eq!(
        verbs,
        vec![("Install agent\u{2026}", true), ("Details", false)]
    );
    // The sky agrees: the star the fault rings is Blocked, whatever
    // the roadmap says, and the murmur says why.
    assert_eq!(model.star_note("e6b").as_deref(), Some("Requires agent"));
    let e6b = model
        .roadmap()
        .rows()
        .into_iter()
        .find(|c| c.id == "e6b")
        .expect("seeded");
    assert_eq!(model.star_state(e6b), StarState::Blocked);
    assert_eq!(model.star_status_label(e6b), "Blocked");
    assert_eq!(model.roadmap().status_label(e6b), "In progress");
    let ids = model.roadmap().graph_ids();
    let nodes = model.sky_nodes(0.0);
    let at = ids.iter().position(|id| *id == "e6b").expect("placed");
    assert_eq!(nodes[at].state, StarState::Blocked);
    assert_ne!(
        nodes[1 - at].state,
        StarState::Blocked,
        "only the ringed star"
    );
}

#[test]
fn a_missing_agent_with_no_stamp_and_no_chunk_still_makes_the_card() {
    let mut model = Model::new(Size::new(120, 40))
        .with_surface(Surface::Native)
        .with_agent_missing(AgentMissing::new("claude"));
    model.set_surface(Surface::Native);
    let day = model.bad_day().expect("a bad day");
    assert_eq!(day.faults()[0].text(), "claude is not on PATH.");
    assert_eq!(day.faults()[0].star(), None);
}

#[test]
fn the_tripped_breaker_names_the_checkpoint_the_spine_saved_and_ages_by_the_pause() {
    let mut model = sky();
    model
        .rail_mut()
        .set_budget(Budget::capped(1200).map(|b| b.spent(1000).stop_at(1000)));
    for _ in 0..6 {
        model
            .rail_mut()
            .spine_mut()
            .open(CheckpointReason::Iteration, "iteration");
    }
    model.rail_mut().spine_mut().saved("cp-6", None, true);
    looped(&mut model, 3600);
    update(
        &mut model,
        Msg::Loop(LoopEvent::Paused("the 10.00 USD limit".to_owned())),
    );
    update(
        &mut model,
        Msg::Loop(LoopEvent::Elapsed(Duration::from_secs(2 * 3600))),
    );
    let day = model.bad_day().expect("a bad day");
    let fault = &day.faults()[0];
    assert_eq!(fault.severity(), Severity::Attention);
    assert_eq!(fault.cap(), "Limit \u{b7} 1 h");
    assert_eq!(
        fault.text(),
        "Session paused at the 10.00 USD limit. Checkpoint cp-6 saved."
    );
    assert!(fault.blocks());
    let verbs: Vec<&str> = fault.verbs().iter().map(|v| v.verb()).collect();
    assert_eq!(verbs, vec!["Increase limit", "View receipts"]);
    assert_eq!(day.blocking(), 1);
}

#[test]
fn a_budget_under_its_breaker_is_no_fault() {
    let mut model = sky();
    model
        .rail_mut()
        .set_budget(Budget::capped(1200).map(|b| b.spent(361).stop_at(1000)));
    assert!(model.bad_day().is_none());
}

#[test]
fn the_lost_feed_does_not_block_and_ages_on_the_loops_clock() {
    let mut model = sky().with_server("tidewatch-server");
    looped(&mut model, 3600 + 57 * 60);
    update(
        &mut model,
        Msg::Engine(EngineEvent::Feed(FeedStatus::Disconnected)),
    );
    update(
        &mut model,
        Msg::Loop(LoopEvent::Elapsed(Duration::from_secs(2 * 3600))),
    );
    let day = model.bad_day().expect("a bad day");
    let fault = &day.faults()[0];
    assert_eq!(fault.cap(), "Connection \u{b7} 3 min");
    assert_eq!(
        fault.text(),
        "Connection to tidewatch-server lost. The session continues on the server."
    );
    assert!(!fault.blocks());
    assert_eq!(day.blocking(), 0);
    assert_eq!(fault.verbs()[0].verb(), "Reconnect");
    // Reattached: the card leaves with the fact.
    update(
        &mut model,
        Msg::Engine(EngineEvent::Feed(FeedStatus::Connected { reconnect: true })),
    );
    assert!(model.bad_day().is_none());
    assert_eq!(model.feed_lost_at(), None);
}

#[test]
fn a_feed_lost_with_no_loop_clock_wears_the_cap_alone() {
    let mut model = sky().with_server("tidewatch-server");
    update(
        &mut model,
        Msg::Engine(EngineEvent::Feed(FeedStatus::Disconnected)),
    );
    let day = model.bad_day().expect("a bad day");
    assert_eq!(day.faults()[0].cap(), "Connection");
    assert_eq!(
        day.faults()[0].text(),
        "Connection to tidewatch-server lost. The session continues on the server."
    );
}

/// A driver-owned loop names no server, and its feed's socket dies with
/// every agent: the hangup between one agent and the next is the gap the
/// loop is working through — the gates, the judge, the next spawn — not
/// a connection anybody could redial. No card; the stage says what is
/// happening, and the cockpit is still working.
#[test]
fn a_feed_lost_on_a_driver_owned_loop_is_the_gap_between_agents_not_a_fault() {
    let mut model = sky();
    looped(&mut model, 60);
    update(&mut model, Msg::Loop(LoopEvent::Stage(LoopStage::Gates)));
    update(
        &mut model,
        Msg::Engine(EngineEvent::Feed(FeedStatus::Disconnected)),
    );
    assert_eq!(model.feed(), Some(FeedStatus::Disconnected));
    assert!(model.bad_day().is_none(), "the gap is not a fault");
    assert_eq!(
        model.loop_status().map(LoopStatus::stage),
        Some(LoopStage::Gates)
    );
    assert!(model.is_working(), "the loop is running its gates");
    // A pause is the one gap that is not work.
    update(
        &mut model,
        Msg::Loop(LoopEvent::Paused("quality gates failed: test".to_owned())),
    );
    assert!(!model.is_working());
}

#[test]
fn failed_gates_block_and_offer_to_run_again() {
    let mut model = sky();
    looped(&mut model, 60);
    update(
        &mut model,
        Msg::Loop(LoopEvent::GateStarted {
            name: "lint".to_owned(),
            job: JobId::new(7),
        }),
    );
    update(
        &mut model,
        Msg::Job {
            id: JobId::new(7),
            output: JobOutput::Failed("clippy".to_owned()),
        },
    );
    let day = model.bad_day().expect("a bad day");
    let fault = &day.faults()[0];
    assert_eq!(fault.cap(), "Gates");
    assert_eq!(fault.text(), "lint failed. The loop waits on a fix.");
    assert!(fault.blocks());
    assert_eq!(fault.verbs()[0].widget(), Widget::LaunchGates);
}

#[test]
fn an_unsaved_checkpoint_is_the_red_card_with_the_error_and_a_retry() {
    let mut model = sky();
    model
        .rail_mut()
        .spine_mut()
        .open(CheckpointReason::Risk, "rm -rf build");
    update(
        &mut model,
        Msg::Checkpoint(CheckpointEvent::Failed {
            id: Some("cp-1".to_owned()),
            what: "save".to_owned(),
            error: "disk full".to_owned(),
        }),
    );
    let day = model.bad_day().expect("a bad day");
    let fault = &day.faults()[0];
    assert_eq!(fault.severity(), Severity::Alert);
    assert_eq!(fault.cap(), "Checkpoint \u{b7} cp-1");
    assert_eq!(fault.text(), "cp-1 was not saved: disk full.");
    assert_eq!(fault.detail(), Some("checkpoint cp-1: disk full"));
    assert_eq!(fault.verbs()[0].widget(), Widget::Checkpoint);
    assert!(!fault.blocks());
}

#[test]
fn a_refused_turn_is_a_card_until_the_next_turn_is_sent() {
    let mut model = sky();
    update(
        &mut model,
        Msg::Agent(AgentEvent::TurnEnded(TurnEnd::Refusal)),
    );
    assert!(model.refused());
    let day = model.bad_day().expect("a bad day");
    assert_eq!(day.faults()[0].cap(), "Refused");
    assert_eq!(day.faults()[0].verbs()[0].widget(), Widget::Composer);
    for c in "again".chars() {
        update(&mut model, Msg::Key(KeyEvent::new(KeyCode::Char(c))));
    }
    update(&mut model, Msg::Key(KeyEvent::new(KeyCode::Enter)));
    assert!(!model.refused(), "the next turn answers the card");
    assert!(model.bad_day().is_none());
}

#[test]
fn the_deal_is_worst_first_and_the_verbs_index_across_the_cards() {
    let mut model = sky()
        .with_server("tidewatch-server")
        .with_agent_missing(AgentMissing::new("claude"));
    model
        .rail_mut()
        .set_budget(Budget::capped(1200).map(|b| b.spent(1000).stop_at(1000)));
    update(
        &mut model,
        Msg::Engine(EngineEvent::Feed(FeedStatus::Disconnected)),
    );
    let day = model.bad_day().expect("a bad day");
    let caps: Vec<String> = day.faults().iter().map(Fault::cap).collect();
    assert_eq!(
        caps,
        vec![
            "Blocking \u{b7} New sessions cannot start \u{b7} Running sessions continue",
            "Limit",
            "Connection"
        ]
    );
    assert_eq!(day.blocking(), 2, "the agent and the breaker stop sessions");
    let verbs: Vec<Widget> = (0..6)
        .filter_map(|n| day.verb(n))
        .map(|v| v.widget())
        .collect();
    assert_eq!(
        verbs,
        vec![
            Widget::FaultInstall,
            Widget::FaultDetails,
            Widget::FaultIncreaseLimit,
            Widget::FaultReceipts,
            Widget::FaultReconnect
        ]
    );
}

#[test]
fn a_bad_day_stands_the_brief_down_and_holds_the_sky_still() {
    let mut model = sky().with_away(Duration::from_secs(3600));
    update(
        &mut model,
        Msg::Engine(EngineEvent::local(crate::harness::fixtures::frame(
            "roadmap",
            "chunk_completed",
            json!({"chunk": {"id": "layout", "name": "layout", "status": "done"}}),
        ))),
    );
    assert!(model.brief().is_some(), "news raises the Brief");
    let mut model = model.with_agent_missing(AgentMissing::new("claude"));
    assert!(
        model.brief().is_none(),
        "a fault is more urgent than a summary"
    );
    assert!(!model.is_sky(), "a bad day is not rest");
    assert!(!model.sky_at_rest());
    // Past the ship moment the completion lit: the bounded transients
    // end, and the room never counts down into rest.
    for _ in 0..1000 {
        update(&mut model, Msg::Tick);
    }
    assert!(!model.sky_at_rest());
    assert!(
        model.animations().is_empty(),
        "the stillest screen in the app declares no motion"
    );
}

#[test]
fn install_announces_the_line_and_leaves_it_on_the_stage() {
    let mut model = sky().with_agent_missing(AgentMissing::new("claude"));
    let cmds = click(&mut model, Widget::FaultInstall.id());
    assert_eq!(
        cmds,
        vec![Cmd::Announce(format!(
            "install the agent: {}",
            BadDay::INSTALL
        ))]
    );
    assert!(
        notes(&model).contains(BadDay::INSTALL),
        "the receipt is on the stage"
    );
    assert!(
        model.bad_day().is_some(),
        "a card is never dismissed by its verb"
    );
}

#[test]
fn details_puts_the_receipt_trail_on_the_stage() {
    let mut model = sky().with_agent_missing(AgentMissing::new("claude"));
    let cmds = click(&mut model, Widget::FaultDetails.id());
    assert!(cmds.is_empty());
    assert!(notes(&model).contains("Install the agent:"));
    assert_eq!(model.shown_tab(), StageTab::Turn);
}

#[test]
fn increase_limit_opens_settings_at_budget_and_receipts_climb_to_the_journal() {
    let mut model = sky();
    model
        .rail_mut()
        .set_budget(Budget::capped(1200).map(|b| b.spent(1000).stop_at(1000)));
    click(&mut model, Widget::FaultIncreaseLimit.id());
    assert_eq!(
        model.settings().map(crate::cockpit::Settings::group),
        Some(SettingsGroup::Budget)
    );
    let mut model = sky();
    click(&mut model, Widget::FaultReceipts.id());
    assert_eq!(model.shown_tab(), StageTab::Session);
}

#[test]
fn reconnect_asks_the_runtime_to_redial() {
    let mut model = sky().with_server("tidewatch-server");
    update(
        &mut model,
        Msg::Engine(EngineEvent::Feed(FeedStatus::Disconnected)),
    );
    assert_eq!(
        click(&mut model, Widget::FaultReconnect.id()),
        vec![Cmd::Reconnect]
    );
}
