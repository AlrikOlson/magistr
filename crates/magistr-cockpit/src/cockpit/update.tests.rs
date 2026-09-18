#[test]
fn composer_action_sends_stops_and_preserves_a_waiting_draft() {
    let mut m = model();
    assert!(update(&mut m, left_click(Widget::PromptAction.id())).is_empty());
    update(&mut m, Msg::Paste("first direction".into()));
    let commands = update(&mut m, left_click(Widget::PromptAction.id()));
    assert!(commands.contains(&Cmd::Prompt("first direction".into())));
    assert!(m.prompt().is_empty());
    assert_eq!(m.turn_state(), TurnState::Streaming);
    update(&mut m, Msg::Paste("a revised direction".into()));
    assert_eq!(
        update(&mut m, left_click(Widget::PromptAction.id())),
        vec![Cmd::Cancel]
    );
    assert_eq!(m.prompt().text(), "a revised direction");
    assert!(
        !update(&mut m, key(KeyCode::Enter))
            .iter()
            .any(|cmd| matches!(cmd, Cmd::Prompt(_)))
    );
    assert_eq!(m.prompt().text(), "a revised direction");
}

#[test]
fn composer_action_cannot_bypass_a_permission_moment() {
    let mut m = model();
    agent(&mut m, plan_request(1));
    assert!(m.moment().is_some());
    update(&mut m, Msg::Paste("keep this draft".into()));
    assert!(
        !update(&mut m, left_click(Widget::PromptAction.id()))
            .iter()
            .any(|cmd| matches!(cmd, Cmd::Prompt(_) | Cmd::Cancel))
    );
}

#[test]
fn session_change_selection_survives_reseed_with_shared_undo_ids() {
    let mut m = model().with_tab(StageTab::Session);
    let a = journal_entry("op", "src/a.rs");
    let b = journal_entry("op", "src/b.rs");
    update(
        &mut m,
        Msg::Workbench(WorkbenchEvent::Journal {
            entries: vec![a.clone(), b.clone()],
        }),
    );
    change_click(&mut m, Widget::change_row(1));
    assert_eq!(m.selected(), Some(&Selected::Change(b.identity())));
    update(
        &mut m,
        Msg::Workbench(WorkbenchEvent::Journal {
            entries: vec![b.clone(), a],
        }),
    );
    assert_eq!(m.selected(), Some(&Selected::Change(b.identity())));
    assert_eq!(
        change_click(&mut m, Widget::change_open(0)),
        vec![Cmd::Workbench(WorkbenchCmd::Open {
            path: "src/b.rs".into()
        })]
    );
    assert_eq!(
        change_click(&mut m, Widget::change_undo(0)),
        vec![Cmd::Workbench(WorkbenchCmd::Undo { id: "op".into() })]
    );
}

#[test]
fn session_report_scroll_and_close_leave_the_timeline_in_place() {
    let mut m = crate::harness::Scenes::long_trace(Size::new(120, 40));
    let top = m.session_top();
    let window = m.session_window();
    update(
        &mut m,
        Msg::Engine(EngineEvent::Checkpoint {
            lines: (0..100).map(|n| format!("report line {n}")).collect(),
        }),
    );
    assert_eq!(m.pane(), Pane::Inspector);
    update(&mut m, key(KeyCode::End));
    assert!(m.inspector_scroll > 0);
    assert_eq!(m.session_top(), top);
    assert_eq!(m.session_window(), window);
    update(&mut m, key(KeyCode::Esc));
    assert!(m.trace().viewer().is_none());
    assert_eq!(m.inspector_scroll, 0);
    assert_eq!(m.session_top(), top);
}

#[test]
fn session_view_joins_navigation_and_keeps_viewers_out_of_the_stage() {
    let mut m = model();
    m.show(StageTab::Session);
    assert_eq!(m.tab().label(), "Session");
    update(
        &mut m,
        frame(
            "think",
            "step_appended",
            serde_json::json!({"step": {
                "step_number": 1, "timestamp": "2026-09-15T00:00:00Z"
            }}),
        ),
    );
    update(
        &mut m,
        Msg::Workbench(WorkbenchEvent::Journal {
            entries: vec![crate::workbench::JournalEntry {
                id: "7".into(),
                path: "src/a.rs".into(),
                at_ms: Some(1_789_430_401_000),
                ..Default::default()
            }],
        }),
    );
    m.inspect(Selected::Step(1));
    update(&mut m, key(KeyCode::Down));
    assert!(matches!(m.selected(), Some(Selected::Change(_))));
    update(&mut m, key(KeyCode::Up));
    assert_eq!(m.selected(), Some(&Selected::Step(1)));
    let extent = m.session_window();
    update(
        &mut m,
        Msg::Engine(EngineEvent::Checkpoint {
            lines: vec!["report".into(); 50],
        }),
    );
    assert_eq!(
        m.session_window(),
        extent,
        "viewer belongs to the inspector"
    );
    assert_eq!(m.pane(), Pane::Inspector);
}

use magistr_domain::policy::RiskTier;
use magistr_workflow_contracts::FeedStatus;

use super::*;
use crate::cockpit::WorkbenchEvent;

#[test]
fn roadmap_table_controls_preserve_selected_identity_and_keyboard_walks_visible_order() {
    let mut m = roadmap_model();
    update(&mut m, left_click(Widget::roadmap_row(1)));
    let chosen = m.selected().cloned();
    update(&mut m, left_click(Widget::RoadmapSortName.id()));
    update(&mut m, left_click(Widget::RoadmapSortName.id()));
    assert_eq!(m.selected(), chosen.as_ref());
    assert_eq!(m.roadmap().rows()[0].id, "ui");
    update(&mut m, key(KeyCode::Down));
    assert_eq!(m.selected(), Some(&Selected::Chunk("mid".into())));
    update(
        &mut m,
        Msg::Key(KeyEvent::new(KeyCode::Char('f')).with(Modifiers::ALT)),
    );
    assert_eq!(m.roadmap().filter_label(), "In progress");
    assert_eq!(m.roadmap_top(), 0);
    update(
        &mut m,
        Msg::Key(KeyEvent::new(KeyCode::Char('s')).with(Modifiers::ALT)),
    );
    assert_eq!(m.roadmap().sort(), super::super::RoadmapSort::Status);
    update(
        &mut m,
        Msg::Key(KeyEvent::new(KeyCode::Char('r')).with(Modifiers::ALT)),
    );
    assert!(m.roadmap().descending());
    assert!(
        m.prompt().is_empty(),
        "table accelerators never enter the prompt"
    );
}

#[test]
fn roadmap_signals_are_selectable_by_pointer_and_keyboard_without_changing_chunk_identity() {
    let mut m = crate::harness::Scenes::roadmapped(Size::new(132, 42));
    let signal_id = m.roadmap().signals()[0].id.clone();
    update(&mut m, left_click(Widget::roadmap_row(1)));
    let chunk = m.roadmap().selected().expect("selected chunk").id.clone();
    update(&mut m, key(KeyCode::Down));
    assert_eq!(m.selected(), Some(&Selected::Signal(signal_id.clone())));
    update(&mut m, key(KeyCode::Up));
    assert_eq!(m.selected(), Some(&Selected::Chunk(chunk.clone())));
    update(&mut m, left_click(Widget::signal_row(0)));
    assert_eq!(m.selected(), Some(&Selected::Signal(signal_id)));
    assert_eq!(
        m.roadmap().selected().expect("graph selection retained").id,
        chunk
    );
    assert!(matches!(
        update(&mut m, left_click(Widget::signal_promote(0))).as_slice(),
        [Cmd::Engine(EngineCmd::PromoteSignal { .. })]
    ));
}

use crate::cockpit::Size;
use crate::cockpit::{
    Animation, Budget, Eased, KillCmd, Motion, MotionName, ShipRun, Surface, Term, TermDock,
    TermHold,
};
use crate::engine::EngineEvent;
use crate::input::KeyKind;
use crate::job::{JobId, JobOutput};
use crate::render::Color;
use crate::theme::{MotionMode, Theme, ThemeMode};
use crate::turn::RowKind;
use magistr_application::sessions::{AgentEvent, ImageBlock, ToolStatus, TurnEnd};

/// The model advanced past a scroll's glide: every viewport
/// moves its window the instant a scroll lands, but the *paint* eases
/// after it over [`Eased::STEPS`]. Assertions about where a scroll left
/// the window take the settled frame; the glide itself is asserted in
/// the tests that are about it.
fn settle(model: &mut Model) {
    for _ in 0..Eased::STEPS {
        update(model, Msg::Tick);
    }
}

#[test]
fn turn_tool_selection_opens_a_live_record_in_the_inspector() {
    let mut m = model();
    agent(
        &mut m,
        AgentEvent::ToolCall {
            id: "inspect-call".into(),
            title: "read settings".into(),
            verb: magistr_application::sessions::ToolVerb::Read,
            status: ToolStatus::InProgress,
            diff: None,
            images: Vec::new(),
            record: Box::new(magistr_application::sessions::ToolRecord {
                input: Some(magistr_application::sessions::ToolPayload::Json(
                    r#"{"path":"settings.toml"}"#.into(),
                )),
                ..Default::default()
            }),
        },
    );
    change_click(&mut m, Widget::stage_row(0));
    assert_eq!(
        m.selected(),
        Some(&crate::cockpit::Selected::Tool("inspect-call".into()))
    );
    agent(
        &mut m,
        AgentEvent::ToolCallUpdate {
            id: "inspect-call".into(),
            title: Some("settings loaded".into()),
            status: Some(ToolStatus::Completed),
            diff: None,
            duration_ms: Some(19),
            images: Vec::new(),
            record: Box::new(magistr_application::sessions::ToolRecord {
                text: vec!["loaded configuration".into()],
                ..Default::default()
            }),
        },
    );
    update(&mut m, Msg::Resize(Size::new(132, 42)));
    assert_eq!(
        m.selected(),
        Some(&crate::cockpit::Selected::Tool("inspect-call".into()))
    );
    update(&mut m, key(KeyCode::Enter));
    assert!(m.inspector_open());
}

#[test]
fn selected_tool_actions_follow_the_identity_after_resize_and_scrolling() {
    use crate::cockpit::{Pane, Selected};
    let mut m = model();
    agent(&mut m, edit_call("first"));
    agent(&mut m, edit_call("second"));
    update(&mut m, key(KeyCode::Enter));
    assert_eq!(m.selected(), Some(&Selected::Tool("second".into())));
    update(&mut m, key(KeyCode::Up));
    assert_eq!(m.selected(), Some(&Selected::Tool("first".into())));
    update(&mut m, key(KeyCode::Enter));
    update(&mut m, Msg::Resize(Size::new(120, 40)));
    for _ in 0..40 {
        agent(&mut m, AgentEvent::Text("streaming text\n".into()));
    }
    update(&mut m, key(KeyCode::End));
    assert_eq!(m.selected(), Some(&Selected::Tool("first".into())));
    assert_eq!(
        change_click(&mut m, Widget::ToolOpen.id()),
        vec![Cmd::Workbench(WorkbenchCmd::Open {
            path: "src/main.rs".into()
        })]
    );
    assert_eq!(m.pane(), Pane::Inspector);
    assert_eq!(
        change_click(&mut m, Widget::ToolUndo.id()),
        vec![Cmd::Workbench(WorkbenchCmd::Undo { id: "first".into() })]
    );
}

fn model() -> Model {
    Model::new(Size::new(80, 24)).with_tab(StageTab::Turn)
}

fn key(code: KeyCode) -> Msg {
    Msg::Key(KeyEvent::new(code))
}

/// The same key with SHIFT held — how the decoder delivers `⇧A`.
fn shifted(code: KeyCode) -> Msg {
    Msg::Key(KeyEvent::new(code).with(Modifiers::SHIFT))
}

#[test]
fn ctrl_c_quits_and_a_bare_q_types_q() {
    let mut m = model();
    assert_eq!(update(&mut m, key(KeyCode::Char('q'))), vec![]);
    assert_eq!(m.prompt().text(), "q", "a prompt that starts with q");
    assert!(!m.quit_requested());

    let mut m = model();
    let ctrl_c = Msg::Key(KeyEvent::new(KeyCode::Char('c')).with(Modifiers::CTRL));
    assert_eq!(update(&mut m, ctrl_c), vec![Cmd::Quit]);
    assert!(m.quit_requested());
}

#[test]
fn a_release_is_not_typed_and_does_not_quit() {
    let mut m = model();
    let shift_q = Msg::Key(KeyEvent::new(KeyCode::Char('Q')).with(Modifiers::SHIFT));
    assert_eq!(update(&mut m, shift_q), vec![]);
    assert_eq!(m.prompt().text(), "Q", "shift types the capital");
    let release = Msg::Key(KeyEvent::new(KeyCode::Char('q')).kind(KeyKind::Release));
    assert_eq!(update(&mut m, release), vec![]);
    assert_eq!(m.prompt().text(), "Q", "the release types nothing");
    assert!(!m.quit_requested());
    assert_eq!(
        m.recent().count(),
        1,
        "the shifted Q was remembered, the release was not"
    );
}

#[test]
fn resize_frames_focus_and_replies_update_state() {
    let mut m = model();
    update(&mut m, Msg::Resize(Size::new(120, 40)));
    update(&mut m, Msg::FrameDue);
    update(&mut m, Msg::FrameDue);
    update(&mut m, Msg::Focus(false));
    update(&mut m, Msg::Background(Color::Rgb(30, 30, 46)));
    update(&mut m, Msg::KeyReleases(true));
    assert_eq!(m.size(), Size::new(120, 40));
    assert_eq!(m.frames(), 2);
    assert!(!m.is_focused());
    assert_eq!(m.background(), Some(Color::Rgb(30, 30, 46)));
    assert!(m.reports_key_releases());
    assert_eq!(m.recent().count(), 0);
}

#[test]
fn a_minimal_text_mode_sets_the_banner_and_a_graphics_mode_clears_it() {
    use crate::capability::{Mode, TextReason};
    let mut m = model();
    assert_eq!(m.banner(), None);
    update(
        &mut m,
        Msg::Mode(Mode::MinimalText(TextReason::TmuxNoPassthrough)),
    );
    let banner = m.banner().expect("a plain-text session carries a banner");
    assert_eq!(
        Some(banner),
        Mode::MinimalText(TextReason::TmuxNoPassthrough).banner()
    );
    update(
        &mut m,
        Msg::Mode(Mode::Graphics {
            pointer_shapes: true,
        }),
    );
    assert_eq!(m.banner(), None, "pixels never show the banner");
}

/// the probe's decision is what the view reads to stand its
/// cell drawings down for a pixel surface.
#[test]
fn the_graphics_mode_turns_the_pixel_layer_on_and_plain_text_leaves_it_off() {
    use crate::capability::{Mode, TextReason};
    let mut m = model();
    assert!(!m.has_pixels(), "nothing is composited before the probe");
    update(
        &mut m,
        Msg::Mode(Mode::Graphics {
            pointer_shapes: true,
        }),
    );
    assert!(m.has_pixels());
    update(&mut m, Msg::Mode(Mode::MinimalText(TextReason::Forced)));
    assert!(!m.has_pixels(), "a forced plain-text session draws cells");
}

/// the header's sparkline is one bucket per BUSY painted
/// frame — the agent events and engine frames that landed in it. A
/// quiet frame is not a sample, so an idle cockpit leaves the ribbon,
/// and the pixel surface drawn from it, completely still.
#[test]
fn the_activity_ribbon_records_busy_frames_only_and_keeps_its_width() {
    let mut m = model();
    assert_eq!(m.activity().count(), 0, "nothing painted yet");
    update(&mut m, Msg::Agent(AgentEvent::Text("hi".into())));
    update(&mut m, Msg::Agent(AgentEvent::Text("hi".into())));
    update(&mut m, Msg::FrameDue);
    update(&mut m, Msg::FrameDue);
    update(&mut m, Msg::Agent(AgentEvent::Text("hi".into())));
    update(&mut m, Msg::FrameDue);
    assert_eq!(
        m.activity().collect::<Vec<u64>>(),
        vec![2, 1],
        "two events in one frame, then one in another; the quiet frame \
         between them is not a sample"
    );
    let busy = |m: &mut Model| {
        update(
            m,
            frame(
                "magistr",
                "phase_set",
                serde_json::json!({"phase": "executing"}),
            ),
        );
        update(m, Msg::FrameDue);
    };
    for _ in 0..crate::cockpit::model::ACTIVITY * 2 {
        busy(&mut m);
    }
    assert_eq!(
        m.activity().len(),
        crate::cockpit::model::ACTIVITY,
        "the ribbon rolls at its width"
    );
    assert!(m.activity().all(|n| n == 1), "one engine frame each");
}

/// `UsageUpdate.used` is "tokens currently in
/// context" — a level, not a running total — so the per-turn figure is
/// how far that level moved across the turn. A turn the context did not
/// grow over is not a sample, exactly as a quiet frame is not one for
/// the event ribbon; the mark still follows the level down, so the turn
/// after a compaction measures from where the window actually is.
#[test]
fn a_turn_samples_the_context_it_added_and_a_compaction_is_not_a_sample() {
    let mut m = model();
    assert_eq!(m.tokens().count(), 0, "no usage reported yet");

    let turn = |m: &mut Model, used: u64| {
        update(
            m,
            Msg::Agent(AgentEvent::Usage {
                used,
                size: 200_000,
                cost_cents: None,
                elapsed_secs: None,
            }),
        );
        update(m, Msg::Agent(AgentEvent::TurnEnded(TurnEnd::EndTurn)));
    };
    turn(&mut m, 12_000);
    turn(&mut m, 18_500);
    assert_eq!(
        m.tokens().collect::<Vec<u64>>(),
        vec![12_000, 6_500],
        "the first turn's whole context, then what the second added"
    );

    // The agent compacted: the window shrank. That turn contributes
    // nothing rather than a negative, and the next one measures from
    // the compacted level.
    turn(&mut m, 4_000);
    turn(&mut m, 5_200);
    assert_eq!(
        m.tokens().collect::<Vec<u64>>(),
        vec![12_000, 6_500, 1_200],
        "a shrinking window is not a sample, and the mark followed it down"
    );

    // A turn with no usage behind it adds nothing either.
    update(&mut m, Msg::Agent(AgentEvent::TurnEnded(TurnEnd::EndTurn)));
    assert_eq!(m.tokens().count(), 3, "no new usage, no new sample");

    for n in 0..crate::cockpit::model::ACTIVITY * 2 {
        turn(&mut m, 6_000 + n as u64 * 1_000);
    }
    assert_eq!(
        m.tokens().len(),
        crate::cockpit::model::ACTIVITY,
        "the token series rolls at the ribbon's width"
    );
}

/// the header's monitor plots one series or the
/// other, and `Model::monitor` is the only thing that decides which —
/// `view::activity_series` and `Model::motion_key`'s sweep both read it,
/// so the painted line and how far it has drawn itself in can never
/// describe different series.
#[test]
fn the_monitor_falls_back_to_the_event_count_until_the_agent_reports_usage() {
    let mut m = model();
    update(&mut m, Msg::Agent(AgentEvent::Text("hi".into())));
    update(&mut m, Msg::Agent(AgentEvent::Text("hi".into())));
    update(&mut m, Msg::FrameDue);
    update(&mut m, Msg::Agent(AgentEvent::Text("hi".into())));
    update(&mut m, Msg::FrameDue);
    assert_eq!(
        m.monitor().collect::<Vec<u64>>(),
        m.activity().collect::<Vec<u64>>(),
        "an agent that never reports usage leaves the busy-frame count on screen"
    );

    update(
        &mut m,
        Msg::Agent(AgentEvent::Usage {
            used: 12_000,
            size: 200_000,
            cost_cents: None,
            elapsed_secs: None,
        }),
    );
    assert_eq!(
        m.monitor().collect::<Vec<u64>>(),
        m.activity().collect::<Vec<u64>>(),
        "usage alone is not a sample — the turn has to end first"
    );

    update(&mut m, Msg::Agent(AgentEvent::TurnEnded(TurnEnd::EndTurn)));
    assert_eq!(
        m.monitor().collect::<Vec<u64>>(),
        vec![12_000],
        "the first turn's tokens take the slot over from the event count"
    );
}

/// the cost on the agent's usage update fills
/// the rail's budget meter — the same message that feeds the token
/// gauge — and a new session's handshake settles the last one's spend,
/// so a loop of sessions meters the whole run against the cap.
#[test]
fn the_agents_priced_usage_fills_the_rails_budget_meter_across_sessions() {
    let mut m = model();
    m.rail_mut().set_budget(Budget::capped(1200));
    assert_eq!(
        m.rail().budget().map(Budget::readout).as_deref(),
        Some("— / 12.00"),
        "seeded, unmetered"
    );
    let priced = |m: &mut Model, cents: u64| {
        update(
            m,
            Msg::Agent(AgentEvent::Usage {
                used: 1_000,
                size: 200_000,
                cost_cents: Some(cents),
                elapsed_secs: None,
            }),
        );
    };
    update(
        &mut m,
        Msg::Agent(AgentEvent::Initialized {
            session_id: "s1".to_owned(),
            model: None,
        }),
    );
    priced(&mut m, 120);
    priced(&mut m, 184);
    assert_eq!(
        m.rail().budget().map(Budget::readout).as_deref(),
        Some("1.84 / 12.00"),
        "the session's running total, not the sum of its reports"
    );
    assert_eq!(
        m.rail().budget().map(Budget::fill_per_mille),
        Some(153),
        "and the meter fills to match"
    );

    // The loop's next iteration is a new session whose cost starts over.
    update(
        &mut m,
        Msg::Agent(AgentEvent::Initialized {
            session_id: "s2".to_owned(),
            model: None,
        }),
    );
    priced(&mut m, 200);
    assert_eq!(
        m.rail().budget().map(Budget::readout).as_deref(),
        Some("3.84 / 12.00"),
        "the run's spend is the settled session plus the live one"
    );

    // An unpriced update meters the context and leaves the money alone.
    update(
        &mut m,
        Msg::Agent(AgentEvent::Usage {
            used: 2_000,
            size: 200_000,
            cost_cents: None,
            elapsed_secs: None,
        }),
    );
    assert_eq!(m.rail().spent_cents(), Some(384));
}

#[test]
fn a_background_reply_moves_a_detected_theme_and_leaves_a_forced_one() {
    let latte = Msg::Background(Color::Rgb(239, 241, 245));
    let mut m = model();
    assert_eq!(m.theme().mode(), ThemeMode::Dark);
    update(&mut m, latte.clone());
    assert_eq!(m.theme().mode(), ThemeMode::Light);
    assert!(!m.theme().is_forced());
    update(&mut m, Msg::Background(Color::Rgb(30, 30, 46)));
    assert_eq!(m.theme().mode(), ThemeMode::Dark);

    let mut forced = model().with_theme(Theme::forced(ThemeMode::Dark));
    update(&mut forced, latte);
    assert_eq!(forced.theme().mode(), ThemeMode::Dark);
    assert!(forced.theme().is_forced());
    assert_eq!(forced.background(), Some(Color::Rgb(239, 241, 245)));
}

#[test]
fn measured_cell_size_is_a_surface_value_and_can_become_unavailable() {
    let mut m = model();
    assert_eq!(m.cell_pixels(), None);
    for (width, height) in [(10, 20), (9, 18)] {
        assert!(
            update(
                &mut m,
                Msg::CellSize(Some(crate::pixel::CellSize::new(width, height)))
            )
            .is_empty()
        );
        assert_eq!(m.cell_pixels(), Some((width, height)));
    }
    update(&mut m, Msg::CellSize(None));
    assert_eq!(m.cell_pixels(), None);
    assert_eq!(m.cell_size(), crate::pixel::CellSize::DEFAULT);
    update(
        &mut m,
        Msg::CellSize(Some(crate::pixel::CellSize::new(0, 20))),
    );
    assert_eq!(m.cell_pixels(), None);
}

#[test]
fn clicks_and_pastes_are_remembered_motion_is_not() {
    let mut m = model();
    let click = Msg::Mouse {
        event: MouseEvent::new(MouseKind::Down(Button::Left), 3, 1),
        target: Some(Widget::Plan.id()),
    };
    let motion = Msg::Mouse {
        event: MouseEvent::new(MouseKind::Move, 4, 1),
        target: None,
    };
    let drag = Msg::Mouse {
        event: MouseEvent::new(MouseKind::Drag(Button::Left), 5, 1),
        target: None,
    };
    update(&mut m, click.clone());
    update(&mut m, motion);
    update(&mut m, drag);
    update(&mut m, Msg::Paste("hi".to_owned()));
    let recent: Vec<&Msg> = m.recent().collect();
    assert_eq!(recent, vec![&click, &Msg::Paste("hi".to_owned())]);
}

#[test]
fn the_ribbon_keeps_the_last_eight() {
    let mut m = model();
    for i in 0..12u32 {
        let c = char::from_u32(u32::from(b'a') + i).expect("letter");
        update(&mut m, key(KeyCode::Char(c)));
    }
    let letters: Vec<char> = m
        .recent()
        .map(|msg| match msg {
            Msg::Key(k) => match k.code {
                KeyCode::Char(c) => c,
                _ => '?',
            },
            _ => '?',
        })
        .collect();
    assert_eq!(letters, ['e', 'f', 'g', 'h', 'i', 'j', 'k', 'l']);
}

#[test]
fn producers_are_counted_and_never_remembered_as_inputs() {
    let mut m = model();
    update(&mut m, Msg::Agent(AgentEvent::Text("a".to_owned())));
    update(&mut m, Msg::Agent(AgentEvent::Thought("b".to_owned())));
    update(
        &mut m,
        Msg::Engine(EngineEvent::Feed(FeedStatus::Connected {
            reconnect: false,
        })),
    );
    update(
        &mut m,
        Msg::Engine(EngineEvent::local(crate::harness::fixtures::frame(
            "roadmap",
            "chunk_changed",
            serde_json::json!({"chunk": {"id": "a"}}),
        ))),
    );
    update(
        &mut m,
        Msg::Engine(EngineEvent::Feed(FeedStatus::Disconnected)),
    );
    assert_eq!(m.agent_events(), 2);
    assert_eq!(m.engine_frames(), 1);
    assert_eq!(m.feed(), Some(FeedStatus::Disconnected));
    assert_eq!(m.inputs(), 0);
    assert_eq!(m.recent().count(), 0);
}

#[test]
fn a_job_result_is_kept_with_the_frame_it_landed_on() {
    let mut m = model();
    assert_eq!(m.jobs_done(), 0);
    assert_eq!(m.last_job(), None);
    assert_eq!(m.last_job_frame(), None);
    for _ in 0..7 {
        update(&mut m, Msg::FrameDue);
    }
    let cmds = update(
        &mut m,
        Msg::Job {
            id: JobId::new(3),
            output: JobOutput::Text("done".to_owned()),
        },
    );
    assert_eq!(cmds, vec![]);
    assert_eq!(m.jobs_done(), 1);
    assert_eq!(
        m.last_job(),
        Some(&(JobId::new(3), JobOutput::Text("done".to_owned())))
    );
    assert_eq!(m.last_job_frame(), Some(7));
    update(
        &mut m,
        Msg::Job {
            id: JobId::new(4),
            output: JobOutput::Failed("boom".to_owned()),
        },
    );
    assert_eq!(m.jobs_done(), 2);
    assert!(m.last_job().is_some_and(|(_, out)| out.is_failed()));
}

fn agent(m: &mut Model, event: AgentEvent) {
    update(m, Msg::Agent(event));
}

fn stage(m: &Model) -> Vec<&str> {
    m.turn().rows().iter().map(|r| r.text.as_str()).collect()
}

#[test]
fn agent_events_fold_into_the_turn_and_move_its_state() {
    use crate::cockpit::TurnState;
    use crate::turn::{RowKind, Voice};
    use magistr_application::sessions::{ToolStatus, ToolVerb, TurnEnd};
    let mut m = model();
    assert_eq!(m.turn_state(), TurnState::Idle);
    agent(
        &mut m,
        AgentEvent::Initialized {
            session_id: "s".to_owned(),
            model: None,
        },
    );
    agent(
        &mut m,
        AgentEvent::Plan {
            entries: 3,
            completed: 1,
        },
    );
    agent(
        &mut m,
        AgentEvent::Usage {
            used: 1,
            size: 2,
            cost_cents: None,
            elapsed_secs: None,
        },
    );
    agent(
        &mut m,
        AgentEvent::System {
            subtype: "current_mode".to_owned(),
        },
    );
    assert!(m.turn().is_empty(), "nothing to show yet");
    assert_eq!(m.turn_state(), TurnState::Idle);
    agent(&mut m, AgentEvent::UserText("fix it".to_owned()));
    agent(&mut m, AgentEvent::Thought("hmm".to_owned()));
    agent(&mut m, AgentEvent::Text("Sure.\nStep ".to_owned()));
    agent(&mut m, AgentEvent::Text("one.".to_owned()));
    agent(
        &mut m,
        AgentEvent::ToolCall {
            id: "c1".to_owned(),
            title: "cargo test".to_owned(),
            verb: ToolVerb::Run,
            status: ToolStatus::InProgress,
            diff: None,
            images: Vec::new(),
            record: Default::default(),
        },
    );
    agent(
        &mut m,
        AgentEvent::ToolCallUpdate {
            id: "c1".to_owned(),
            title: None,
            status: Some(ToolStatus::Completed),
            diff: None,
            duration_ms: Some(842),
            images: Vec::new(),
            record: Default::default(),
        },
    );
    agent(&mut m, AgentEvent::Error("pipe closed".to_owned()));
    assert_eq!(m.turn_state(), TurnState::AwaitingInput);
    assert_eq!(
        stage(&m),
        [
            "fix it",
            "hmm",
            "Sure.",
            "Step one.",
            "✓ run    cargo test · 842ms",
            "pipe closed"
        ]
    );
    let kinds: Vec<RowKind> = m.turn().rows().iter().map(|r| r.kind).collect();
    assert_eq!(kinds[0], RowKind::Prose(Voice::User));
    assert_eq!(kinds[1], RowKind::Prose(Voice::Thought));
    assert_eq!(kinds[4], RowKind::Tool(ToolStatus::Completed));
    assert_eq!(kinds[5], RowKind::Error);
    agent(&mut m, AgentEvent::TurnEnded(TurnEnd::EndTurn));
    assert_eq!(m.turn_state(), TurnState::AwaitingInput);
    assert_eq!(m.turn().len(), 6, "a normal end adds no row");
    assert_eq!(m.agent_events(), 12);
    assert_eq!(m.inputs(), 0);
}

#[test]
fn a_turn_that_ends_early_leaves_a_note() {
    use crate::turn::RowKind;
    use magistr_application::sessions::TurnEnd;
    for (end, why) in [
        (TurnEnd::MaxTokens, "The response reached its length limit."),
        (TurnEnd::MaxTurnRequests, "The request limit was reached."),
        (TurnEnd::Refusal, "The agent declined this request."),
        (TurnEnd::Cancelled, "Stopped."),
        (TurnEnd::Other, "The agent stopped."),
    ] {
        let mut m = model();
        agent(&mut m, AgentEvent::Text("partial".to_owned()));
        agent(&mut m, AgentEvent::TurnEnded(end));
        let note: Vec<&str> = m
            .turn()
            .rows()
            .iter()
            .filter(|r| r.kind == RowKind::Note)
            .map(|r| r.text.as_str())
            .collect();
        assert!(!note.is_empty(), "a note");
        assert_eq!(note.join(" "), why);
    }
}

#[test]
fn a_resize_rewraps_the_turn_to_the_new_stage_width() {
    let mut m = model();
    assert_eq!(m.stage_width(), 49, "80 cols less padding and the rail");
    assert_eq!(m.stage_rows(), 9);
    agent(
        &mut m,
        AgentEvent::Text("word ".repeat(30).trim_end().to_owned()),
    );
    assert_eq!(m.turn().len(), 4);
    update(&mut m, Msg::Resize(Size::new(40, 12)));
    assert_eq!(m.stage_width(), 17, "the rail caps at half the screen");
    assert_eq!(m.stage_rows(), 3);
    assert_eq!(m.turn().width(), 15, "the stage less the accent gutter");
    assert_eq!(m.turn().len(), 10);
    update(&mut m, Msg::Resize(Size::new(1, 1)));
    assert_eq!(m.stage_width(), 0);
    assert_eq!(m.stage_rows(), 0);
    assert_eq!(m.turn().len(), 0);
    assert!(m.visible_rows().is_empty());
}

#[test]
fn a_seed_shows_the_stores_plan_without_waiting_for_a_frame() {
    let mut m = model();
    assert_eq!(m.rail().ship().objective(), None);
    update(
        &mut m,
        Msg::Engine(EngineEvent::Seed(
            magistr_workflow_contracts::NativeSnapshot {
                objective: Some(crate::harness::fixtures::objective("ship the rail")),
                tasks: vec![crate::harness::fixtures::task_from(serde_json::json!({
                    "id": "t1", "title": "implement", "status": "active"
                }))],
                progress: crate::harness::fixtures::progress(5, 9),
                chunks: crate::harness::fixtures::chunks_from(vec![
                    serde_json::json!({"id": "tui2-2a1-rail", "status": "in_progress"}),
                ]),
                signals: (0..1)
                    .map(|n| {
                        crate::harness::fixtures::signal_from(
                            serde_json::json!({"id": format!("s{n}"), "status": "new"}),
                        )
                    })
                    .collect(),
                ..magistr_workflow_contracts::NativeSnapshot::default()
            },
        )),
    );
    assert_eq!(m.rail().ship().objective(), Some("ship the rail"));
    assert_eq!(m.rail().ship().tasks().len(), 1);
    assert_eq!(m.rail().chunk(), Some("tui2-2a1-rail"));
    assert_eq!(m.rail().progress(), (5, 9));
    assert_eq!(m.rail().signals_pending(), 1);
}

#[test]
fn a_seed_shows_the_stores_trace_and_a_re_seed_replaces_it() {
    let mut m = model();
    assert!(m.trace().is_empty());
    let step = |number: u32, purpose: &str| {
        crate::harness::fixtures::step_from(
            serde_json::json!({"step_number": number, "purpose": purpose}),
        )
    };
    // The task the gate was recorded on, as the store holds it.
    let gated_task = || {
        crate::harness::fixtures::task_from(serde_json::json!({
            "id": "t1", "checks": [{"name": "lint", "passed": true}],
        }))
    };
    let seed = |steps: Vec<magistr_workflow_contracts::ThinkStep>| {
        Msg::Engine(EngineEvent::Seed(
            magistr_workflow_contracts::NativeSnapshot {
                steps,
                tasks: vec![gated_task()],
                ..magistr_workflow_contracts::NativeSnapshot::default()
            },
        ))
    };
    update(&mut m, seed(vec![step(1, "open")]));
    assert_eq!(
        m.trace().steps().len(),
        1,
        "the store's step, with no frame yet"
    );
    assert_eq!(m.trace().step(1).expect("step 1").purpose, "open");
    assert_eq!(m.trace().moments().len(), 1);

    // The loop driver's per-iteration re-seed: the store grew by a step,
    // and the gate it already carried is not counted twice.
    update(&mut m, seed(vec![step(1, "open"), step(2, "close")]));
    assert_eq!(m.trace().steps().len(), 2);
    assert_eq!(m.trace().moments().len(), 1, "no duplicate moment");
}

#[test]
fn one_engine_frame_updates_the_rail_at_the_next_paint() {
    let mut m = model();
    update(
        &mut m,
        Msg::Engine(EngineEvent::local(crate::harness::fixtures::frame(
            "ship",
            "objective_set",
            serde_json::json!({"objective": {"description": "goal"}}),
        ))),
    );
    assert_eq!(m.rail().ship().objective(), Some("goal"));
    update(
        &mut m,
        Msg::Engine(EngineEvent::local(crate::harness::fixtures::frame(
            "signal",
            "signal_captured",
            serde_json::json!({"signal": {"id": "s1", "status": "new"}}),
        ))),
    );
    assert_eq!(m.rail().signals_pending(), 1);
}

#[test]
fn ship_and_think_frames_land_as_cards_in_the_turn() {
    let mut m = model();
    update(
        &mut m,
        frame(
            "ship",
            "objective_set",
            serde_json::json!({"objective": {"description": "goal"}}),
        ),
    );
    update(
        &mut m,
        frame(
            "ship",
            "task_added",
            serde_json::json!({"task_id": "t1", "title": "wire the cards"}),
        ),
    );
    update(
        &mut m,
        frame("ship", "task_started", serde_json::json!({"task_id": "t1"})),
    );
    update(
        &mut m,
        frame(
            "think",
            "step_appended",
            serde_json::json!({"step": {"step_number": 42, "purpose": "open the iteration"}}),
        ),
    );
    update(
        &mut m,
        frame(
            "ship",
            "check_recorded",
            serde_json::json!({"task_id": "t1", "check": {
                "name": "cargo test", "passed": false, "exit_code": 101,
                "duration_ms": 2345, "details": "3 failed"
            }}),
        ),
    );
    update(
        &mut m,
        frame("ship", "objective_shipped", serde_json::json!({})),
    );
    update(
        &mut m,
        frame(
            "signal",
            "signal_captured",
            serde_json::json!({"signal": {"id": "s1", "status": "new"}}),
        ),
    );
    let texts: Vec<&str> = m.turn().rows().iter().map(|r| r.text.as_str()).collect();
    assert_eq!(
        texts,
        [
            "▸ objective set · goal",
            "▸ task started · wire the cards",
            "✦ Step 42 · open the iteration",
            "✗ gate   cargo test · exit 101 · 2.3s",
            "▸ objective shipped",
        ],
        "signal frames stay out of the turn"
    );
}

#[test]
fn a_click_on_a_check_card_expands_it() {
    use crate::input::{Button, MouseEvent, MouseKind};
    let mut m = model();
    update(
        &mut m,
        frame(
            "ship",
            "check_recorded",
            serde_json::json!({"check": {
                "name": "cargo test", "passed": true, "exit_code": 0, "details": "348 passed"
            }}),
        ),
    );
    assert_eq!(m.turn().len(), 1);
    let click = |m: &mut Model| {
        update(
            m,
            Msg::Mouse {
                event: MouseEvent::new(MouseKind::Down(Button::Left), 30, 5),
                target: Some(Widget::stage_row(0)),
            },
        );
    };
    click(&mut m);
    assert_eq!(m.turn().len(), 2, "the card expanded");
    assert!(m.turn().rows()[1].text.starts_with("  348 passed"));
    click(&mut m);
    assert_eq!(m.turn().len(), 1, "and collapsed again");
}

#[test]
fn dragging_the_divider_resizes_the_rail_and_rewraps_the_turn() {
    use crate::input::{Button, MouseEvent, MouseKind};
    let mut m = model();
    agent(
        &mut m,
        AgentEvent::Text("word ".repeat(30).trim_end().to_owned()),
    );
    let wrapped = m.turn().width();
    let press = Msg::Mouse {
        event: MouseEvent::new(MouseKind::Down(Button::Left), 29, 10),
        target: Some(Widget::RailDivider.id()),
    };
    assert_eq!(update(&mut m, press), vec![]);
    assert!(m.rail().is_dragging());
    let drag = Msg::Mouse {
        event: MouseEvent::new(MouseKind::Drag(Button::Left), 20, 10),
        target: None,
    };
    update(&mut m, drag);
    assert_eq!(m.rail().width(), 20);
    assert_eq!(
        m.turn().width(),
        m.turn_width(),
        "the turn follows the drag"
    );
    assert_ne!(m.turn().width(), wrapped, "a narrower rail rewraps wider");
    let release = Msg::Mouse {
        event: MouseEvent::new(MouseKind::Up(Button::Left), 20, 10),
        target: None,
    };
    update(&mut m, release);
    assert!(!m.rail().is_dragging());
}

/// The divider dragged to column `col`, with the pixel a 10x20 cell
/// puts it at when `window` — the gesture on both surfaces.
fn drag_divider_to(m: &mut Model, col: u16, window: bool) {
    use crate::input::{Button, MouseEvent, MouseKind};
    let at = |kind, col: u16| {
        let event = MouseEvent::new(kind, col, 10);
        if window {
            event.at_px(u32::from(col) * 10 + 3, 205)
        } else {
            event
        }
    };
    update(
        m,
        Msg::Mouse {
            event: at(MouseKind::Down(Button::Left), col),
            target: Some(Widget::RailDivider.id()),
        },
    );
    update(
        m,
        Msg::Mouse {
            event: at(MouseKind::Drag(Button::Left), col),
            target: None,
        },
    );
    update(
        m,
        Msg::Mouse {
            event: at(MouseKind::Up(Button::Left), col),
            target: None,
        },
    );
}

/// On a window the rail is on the right (view 05), so dragging the
/// divider to a column asks for the columns right of it; on a terminal
/// the rail is on the left and the column is the width. The pixel is
/// what the window reads, and it lands on the same column.
#[test]
fn the_divider_measures_the_rail_from_the_side_it_is_on() {
    let mut terminal = Model::new(Size::new(120, 40));
    drag_divider_to(&mut terminal, 20, false);
    assert_eq!(terminal.rail().width(), 20);

    let mut window = on_a_window(Model::new(Size::new(120, 40)));
    assert!(window.rail_on_right());
    drag_divider_to(&mut window, 90, true);
    assert_eq!(
        window.rail().width(),
        120 - 90 - 1,
        "the columns right of the divider, less the divider's own"
    );
    assert_eq!(
        window.turn().width(),
        window.turn_width(),
        "the turn follows"
    );
    // The minimums hold on the right as on the left: the rail's own,
    // and the stage's half screen.
    drag_divider_to(&mut window, 118, true);
    assert_eq!(window.rail().width(), 16, "the rail's minimum");
    drag_divider_to(&mut window, 10, true);
    assert_eq!(window.rail().width(), 48, "the rail's maximum");
    drag_divider_to(&mut window, 65, true);
    assert_eq!(window.rail().width(), 48);
    let mut narrow = on_a_window(Model::new(Size::new(80, 24)));
    drag_divider_to(&mut narrow, 10, true);
    assert_eq!(
        narrow.rail().width(),
        40,
        "half the screen: the stage's minimum"
    );
}

/// The width the user dragged to is the rail's, whatever happens to
/// the rail after: a collapse keeps it for the expand, and a narrower
/// window caps it without forgetting it.
#[test]
fn the_dragged_width_survives_a_collapse_and_a_resize() {
    use crate::input::{Button, MouseEvent, MouseKind};
    let mut m = on_a_window(Model::new(Size::new(120, 40)));
    drag_divider_to(&mut m, 79, true);
    assert_eq!(m.rail().width(), 40);
    let toggle = |m: &mut Model| {
        update(
            m,
            Msg::Mouse {
                event: MouseEvent::new(MouseKind::Down(Button::Left), 118, 3),
                target: Some(Widget::RailToggle.id()),
            },
        );
    };
    toggle(&mut m);
    assert!(m.rail().is_collapsed());
    assert_eq!(m.rail().cols(120), 2, "the collapsed strip");
    toggle(&mut m);
    assert_eq!(
        m.rail().content_cols(120),
        40,
        "expanded back to the dragged width"
    );

    update(&mut m, Msg::Resize(Size::new(60, 40)));
    assert_eq!(
        m.rail().content_cols(60),
        30,
        "capped at half the narrower window"
    );
    assert_eq!(
        m.turn().width(),
        m.turn_width(),
        "and the turn re-wrapped to it"
    );
    update(&mut m, Msg::Resize(Size::new(120, 40)));
    assert_eq!(
        m.rail().content_cols(120),
        40,
        "and back, the width remembered"
    );
}

/// The rail's collapse and expand fade its chrome in on a window over
/// the design's chrome fade — the tick runs for exactly that long — and
/// a terminal, whose motion key never carries the rail's state, does
/// not animate a toggle at all.
#[test]
fn a_rail_toggle_fades_its_chrome_on_a_window_and_is_instant_on_a_terminal() {
    use crate::input::{Button, MouseEvent, MouseKind};
    let toggle = |m: &mut Model| {
        update(
            m,
            Msg::Mouse {
                event: MouseEvent::new(MouseKind::Down(Button::Left), 1, 3),
                target: Some(Widget::RailToggle.id()),
            },
        )
    };
    let mut terminal = model();
    assert_eq!(toggle(&mut terminal), vec![], "no tick asked for");
    assert!(!terminal.is_animating());

    let mut window = on_a_window(model());
    assert!(!window.is_animating());
    toggle(&mut window);
    assert!(window.is_animating(), "the collapse fades the strip in");
    assert_eq!(window.motion().rail_fade(window.now()), Some(0.0));
    for _ in 0..Motion::CHROME_FADE_STEPS {
        update(&mut window, Msg::Tick);
    }
    assert!(!window.is_animating(), "and the window goes quiet again");
    assert!(window.motion().rail_fade(window.now()).is_none());
    toggle(&mut window);
    assert!(window.is_animating(), "the expand fades the rows in");
}

#[test]
fn the_rail_toggle_collapses_and_rewraps() {
    use crate::input::{Button, MouseEvent, MouseKind};
    let mut m = model();
    let expanded_width = m.stage_width();
    let toggle = |m: &mut Model| {
        update(
            m,
            Msg::Mouse {
                event: MouseEvent::new(MouseKind::Down(Button::Left), 1, 3),
                target: Some(Widget::RailToggle.id()),
            },
        );
    };
    toggle(&mut m);
    assert!(m.rail().is_collapsed());
    assert!(m.stage_width() > expanded_width);
    assert_eq!(m.turn().width(), m.turn_width());
    toggle(&mut m);
    assert!(!m.rail().is_collapsed());
    assert_eq!(m.stage_width(), expanded_width);
}

#[test]
fn keys_and_the_wheel_move_the_window_over_a_ten_thousand_line_turn() {
    use crate::input::MouseEvent;
    let mut m = model();
    for i in 0..10_000 {
        agent(&mut m, AgentEvent::Text(format!("line {i}\n")));
    }
    let height = m.stage_rows();
    assert_eq!(height, 9);
    assert!(m.viewport().is_following());
    assert_eq!(m.top_row(), 10_000 - height);
    assert_eq!(m.visible_rows().len(), height);
    assert_eq!(m.visible_rows()[height - 1].text, "line 9999");

    update(&mut m, key(KeyCode::PageUp));
    settle(&mut m);
    assert_eq!(m.top_row(), 10_000 - height - (height - 1));
    assert!(!m.viewport().is_following());
    let wheel = |kind| Msg::Mouse {
        event: MouseEvent::new(kind, 5, 5),
        target: Some(Widget::Stage.id()),
    };
    update(&mut m, wheel(MouseKind::ScrollUp));
    settle(&mut m);
    assert_eq!(m.top_row(), 10_000 - height - (height - 1) - 3);
    update(&mut m, wheel(MouseKind::ScrollDown));
    settle(&mut m);
    assert_eq!(m.top_row(), 10_000 - height - (height - 1));
    update(&mut m, key(KeyCode::PageDown));
    assert!(m.viewport().is_following(), "back at the tail");

    update(&mut m, key(KeyCode::Home));
    settle(&mut m);
    assert_eq!(m.top_row(), 0);
    assert_eq!(m.visible_rows()[0].text, "line 0");
    agent(&mut m, AgentEvent::Text("line 10000\n".to_owned()));
    assert_eq!(m.top_row(), 0, "a pinned window stays while rows arrive");
    update(&mut m, key(KeyCode::End));
    settle(&mut m);
    assert!(m.viewport().is_following());
    assert_eq!(m.visible_rows()[height - 1].text, "line 10000");
    let release = Msg::Key(KeyEvent::new(KeyCode::PageUp).kind(KeyKind::Release));
    update(&mut m, release);
    assert!(m.viewport().is_following(), "a release scrolls nothing");
    assert_eq!(m.inputs(), 7);
}

fn ctrl(c: char) -> Msg {
    Msg::Key(KeyEvent::new(KeyCode::Char(c)).with(Modifiers::CTRL))
}

fn type_text(m: &mut Model, text: &str) {
    for c in text.chars() {
        update(m, key(KeyCode::Char(c)));
    }
}

#[test]
fn typing_pasting_and_the_shell_keys_edit_the_prompt() {
    let mut m = model();
    type_text(&mut m, "helo");
    update(&mut m, key(KeyCode::Left));
    update(&mut m, key(KeyCode::Left));
    update(&mut m, key(KeyCode::Char('l')));
    assert_eq!(m.prompt().text(), "hello");
    update(&mut m, Msg::Paste(" world\n".to_owned()));
    assert_eq!(m.prompt().text(), "hel world\nlo");
    update(&mut m, ctrl('e'));
    update(&mut m, key(KeyCode::Backspace));
    update(&mut m, key(KeyCode::Backspace));
    assert_eq!(m.prompt().text(), "hel world\n");
    update(&mut m, key(KeyCode::Up));
    update(&mut m, key(KeyCode::Delete));
    update(&mut m, ctrl('d'));
    assert_eq!(m.prompt().text(), "l world\n", "up reaches the first line");
    assert_eq!(m.prompt().cursor(), 0);
    update(&mut m, key(KeyCode::End));
    update(&mut m, ctrl('w'));
    assert_eq!(
        m.prompt().text(),
        "l \n",
        "ctrl-W takes the last word on its line"
    );
    update(&mut m, key(KeyCode::Down));
    update(&mut m, key(KeyCode::Backspace));
    assert_eq!(m.prompt().text(), "l ", "backspace joins the lines");
    update(&mut m, key(KeyCode::Home));
    update(&mut m, key(KeyCode::Right));
    update(&mut m, ctrl('k'));
    assert_eq!(m.prompt().text(), "l");
    update(&mut m, ctrl('u'));
    assert_eq!(m.prompt().text(), "");
    let shifted = Msg::Key(KeyEvent::new(KeyCode::Char('Q')).with(Modifiers::SHIFT));
    update(&mut m, shifted);
    assert_eq!(m.prompt().text(), "Q", "shift types a capital");
    assert!(!m.quit_requested());
    assert_eq!(m.inputs(), 23);
}

#[test]
fn enter_sends_the_prompt_echoes_it_and_streams_until_the_turn_ends() {
    use crate::cockpit::TurnState;
    use crate::turn::{RowKind, Voice};
    let mut m = model();
    assert_eq!(
        update(&mut m, key(KeyCode::Enter)),
        vec![],
        "nothing to send"
    );
    type_text(&mut m, "fix the tests");
    update(&mut m, key(KeyCode::PageUp));
    let cmds = update(&mut m, key(KeyCode::Enter));
    assert_eq!(
        cmds,
        vec![
            Cmd::Prompt("fix the tests".to_owned()),
            Cmd::Animate(Model::TICK_PERIOD),
        ]
    );
    assert!(m.prompt().is_empty());
    assert_eq!(m.prompt().history(), ["fix the tests"]);
    assert_eq!(m.turn_state(), TurnState::Streaming);
    assert!(m.viewport().is_following(), "the echo is at the tail");
    let last = m.turn().rows().last().expect("the echo");
    assert_eq!(last.kind, RowKind::Prose(Voice::User));
    assert_eq!(last.text, "fix the tests");
    type_text(&mut m, "again");
    assert_eq!(
        update(&mut m, key(KeyCode::Enter)),
        vec![],
        "no second turn while one streams"
    );
    assert_eq!(m.prompt().text(), "again", "the line waits");
    agent(&mut m, AgentEvent::Text("done\n".to_owned()));
    agent(
        &mut m,
        AgentEvent::TurnEnded(magistr_application::sessions::TurnEnd::EndTurn),
    );
    assert_eq!(
        update(&mut m, key(KeyCode::Enter)),
        vec![Cmd::Prompt("again".to_owned())],
        "the second turn sends the prompt"
    );
    assert!(
        m.is_animating(),
        "the spinner runs again — the tick was never given back, since the \
         rows the first turn ended on were still fading in"
    );
    update(&mut m, key(KeyCode::Up));
    assert_eq!(m.prompt().text(), "again", "history comes back with Up");
    update(&mut m, key(KeyCode::Up));
    assert_eq!(m.prompt().text(), "fix the tests");
    update(&mut m, key(KeyCode::Down));
    update(&mut m, key(KeyCode::Down));
    assert!(m.prompt().is_empty(), "below the newest is the empty draft");
}

#[test]
fn ctrl_c_cancels_a_streaming_turn_and_quits_otherwise_and_q_only_on_an_empty_prompt() {
    use crate::cockpit::TurnState;
    use crate::turn::RowKind;
    let mut m = model();
    agent(&mut m, AgentEvent::Text("working…".to_owned()));
    assert_eq!(m.turn_state(), TurnState::Streaming);
    assert_eq!(update(&mut m, ctrl('c')), vec![Cmd::Cancel]);
    assert!(!m.quit_requested());
    let note = m.turn().rows().last().expect("a note");
    assert_eq!(
        (note.kind, note.text.as_str()),
        (RowKind::Note, "Stopping…")
    );
    agent(
        &mut m,
        AgentEvent::TurnEnded(magistr_application::sessions::TurnEnd::Cancelled),
    );
    assert_eq!(m.turn_state(), TurnState::AwaitingInput);
    assert_eq!(update(&mut m, ctrl('c')), vec![Cmd::Quit]);
    assert!(m.quit_requested());

    let mut m = model();
    type_text(&mut m, "hq");
    assert!(!m.quit_requested(), "q after a letter is a letter");
    assert_eq!(m.prompt().text(), "hq");
    update(&mut m, key(KeyCode::Home));
    assert_eq!(
        update(&mut m, ctrl('d')),
        vec![],
        "ctrl-D with text deletes"
    );
    assert_eq!(m.prompt().text(), "q");
    assert_eq!(update(&mut m, ctrl('d')), vec![]);
    assert!(m.prompt().is_empty());
    assert_eq!(
        update(&mut m, ctrl('d')),
        vec![Cmd::Quit],
        "ctrl-D on an empty prompt quits"
    );

    let mut streaming = model();
    agent(&mut streaming, AgentEvent::Text("working…".to_owned()));
    assert_eq!(
        update(&mut streaming, ctrl('d')),
        vec![Cmd::Quit],
        "ctrl-D on an empty prompt quits whatever the turn is doing"
    );
}

fn request(id: u64, title: &str) -> AgentEvent {
    use magistr_application::sessions::{PermissionChoice, PermissionKind};
    AgentEvent::PermissionRequest {
        id,
        title: title.to_owned(),
        choices: vec![
            PermissionChoice::new("allow", PermissionKind::AllowOnce),
            PermissionChoice::new("allow_always", PermissionKind::AllowAlways),
            PermissionChoice::new("reject", PermissionKind::RejectOnce),
        ],
        edit: false,
        risk: RiskTier::Medium,
        plan: None,
    }
}

fn answered(id: u64, option_id: &str) -> Vec<Cmd> {
    vec![Cmd::Permission {
        id,
        option_id: option_id.to_owned(),
    }]
}

fn front_title(m: &Model) -> Option<&str> {
    match m.dock().front() {
        Some(DockItem::Permission(p)) => Some(p.title()),
        _ => None,
    }
}

/// A plan approval request with a scope, three steps and one
/// picture.
fn plan_request(id: u64) -> AgentEvent {
    use magistr_application::sessions::{AskPlan, ImageBlock, PermissionChoice, PermissionKind};
    AgentEvent::PermissionRequest {
        id,
        title: "Re-bless 12 golden screenshots".to_owned(),
        choices: vec![
            PermissionChoice::new("allow", PermissionKind::AllowOnce),
            PermissionChoice::new("allow_always", PermissionKind::AllowAlways),
            PermissionChoice::new("reject", PermissionKind::RejectOnce),
        ],
        edit: false,
        risk: RiskTier::Medium,
        plan: Some(
            AskPlan::new([
                "Render the 6 surfaces in both themes",
                "Compare with goldens. Only status-bar rows may change",
                "Update matching goldens. Reject all others",
            ])
            .scoped("goldens")
            .with_images(vec![ImageBlock {
                mime_type: "image/png".to_owned(),
                data: "not-a-png".to_owned(),
                uri: None,
            }]),
        ),
    }
}

#[test]
fn a_plan_ask_raises_a_moment_and_a_plain_ask_does_not() {
    let mut m = model();
    agent(&mut m, request(1, "Bash"));
    assert!(m.moment().is_none(), "a tool ask stays the dock's strip");
    assert_eq!(update(&mut m, key(KeyCode::Enter)), answered(1, "allow"));
    agent(&mut m, plan_request(2));
    let moment = m.moment().expect("a plan ask is a Moment");
    assert_eq!(moment.title(), "Re-bless 12 golden screenshots");
    assert_eq!(moment.steps().count(), 3);
    assert_eq!(moment.scope(), Some("goldens"));
}

/// A plain tool ask — no plan, not an edit — is a card on the window
/// at the Session and nowhere else: the
/// terminal keeps its strip, the Sky keeps its lit room (view 01
/// carries such an ask over the Galaxy), and a plan or an edit is the
/// other Moment's.
#[test]
fn a_plain_ask_is_a_card_on_the_window_at_the_session_and_nowhere_else() {
    use crate::cockpit::{Place, Surface};
    use magistr_application::sessions::{PermissionChoice, PermissionKind};
    let mut terminal = model();
    agent(&mut terminal, request(1, "Bash"));
    assert!(terminal.ask().is_none(), "the terminal has its strip");

    let mut w = model().with_surface(Surface::Native);
    agent(&mut w, request(1, "Bash"));
    assert_eq!(
        w.ask().map(|p| p.title()),
        Some("Bash"),
        "the window lays out no strip: the ask is the card"
    );
    assert!(w.moment().is_none() && w.review().is_none());
    w.place = Place::Sky;
    assert!(w.ask().is_none(), "the constellation stays lit");
    w.place = Place::Session;
    assert_eq!(update(&mut w, key(KeyCode::Enter)), answered(1, "allow"));
    assert!(w.ask().is_none(), "answered, the card is gone");

    agent(&mut w, plan_request(2));
    assert!(
        w.ask().is_none() && w.moment().is_some(),
        "a plan is the plan card's"
    );
    assert_eq!(update(&mut w, key(KeyCode::Enter)), answered(2, "allow"));
    agent(
        &mut w,
        AgentEvent::PermissionRequest {
            id: 3,
            title: "Edit main.rs".to_owned(),
            choices: vec![
                PermissionChoice::new("allow", PermissionKind::AllowOnce),
                PermissionChoice::new("reject", PermissionKind::RejectOnce),
            ],
            edit: true,
            risk: RiskTier::Medium,
            plan: None,
        },
    );
    assert!(
        w.ask().is_none() && w.review().is_some(),
        "an edit is the review's"
    );
}

#[test]
fn the_moments_single_keys_answer_it() {
    for (msg, option) in [
        (key(KeyCode::Char('a')), "allow"),
        (shifted(KeyCode::Char('A')), "allow_always"),
        (key(KeyCode::Char('d')), "reject"),
        (key(KeyCode::Enter), "allow"),
        (key(KeyCode::Esc), "reject"),
    ] {
        let mut m = model();
        agent(&mut m, plan_request(7));
        let cmds = update(&mut m, msg.clone());
        assert!(
            cmds.contains(&Cmd::Permission {
                id: 7,
                option_id: option.to_owned(),
            }),
            "{msg:?} answers {option}: {cmds:?}"
        );
        assert!(m.moment().is_none(), "the card left");
    }
}

#[test]
fn a_moment_click_answers_through_the_same_target_the_strip_sends() {
    let mut m = model();
    agent(&mut m, plan_request(9));
    let click = Msg::Mouse {
        event: MouseEvent::new(MouseKind::Down(Button::Left), 10, 20),
        target: Some(Widget::Allow.id()),
    };
    assert_eq!(update(&mut m, click), answered(9, "allow"));
}

#[test]
fn space_shows_the_pictures_once_and_the_card_keeps_its_place() {
    let mut m = model();
    agent(&mut m, plan_request(3));
    assert!(!m.moment().expect("a Moment").hidden_images().is_empty());
    assert_eq!(update(&mut m, key(KeyCode::Char(' '))), vec![]);
    let moment = m.moment().expect("the card is still up");
    assert!(
        moment.hidden_images().is_empty(),
        "the pictures were handed over"
    );
    // A second press has nothing left to give and types instead.
    update(&mut m, key(KeyCode::Char(' ')));
    assert_eq!(m.condition().text(), " ");
    assert!(m.condition_open(), "a key that answers nothing opens it");
}

#[test]
fn a_written_condition_owns_the_letters_and_enter_answers_with_it() {
    let mut m = model();
    agent(&mut m, plan_request(4));
    // The sentence begins with the letter that answers the card, so the
    // register is opened first — which is what `⇥` is for.
    update(&mut m, key(KeyCode::Tab));
    assert!(m.condition_open());
    for c in "allow, stop if other rows change".chars() {
        update(&mut m, key(KeyCode::Char(c)));
    }
    assert_eq!(m.condition().text(), "allow, stop if other rows change");
    assert!(m.moment().is_some(), "no letter answered the card");
    assert!(m.prompt().is_empty(), "and none reached the prompt");
    let cmds = update(&mut m, key(KeyCode::Enter));
    assert_eq!(
        cmds,
        vec![
            Cmd::Constraint {
                id: 4,
                condition: "allow, stop if other rows change".to_owned(),
            },
            Cmd::Permission {
                id: 4,
                option_id: "allow".to_owned(),
            },
        ],
        "a standing constraint on the ask, then the answer that closes it"
    );
    assert!(m.condition().is_empty(), "the register was spent");
    assert!(
        m.rail()
            .charter()
            .iter()
            .any(|rule| rule.subject() == "allow, stop if other rows change"
                && rule.mode() == RuleMode::Ask),
        "the constraint is on the charter to read back: {:?}",
        m.rail().charter()
    );
}

#[test]
fn esc_clears_a_written_condition_before_it_denies() {
    let mut m = model();
    agent(&mut m, plan_request(5));
    update(&mut m, key(KeyCode::Char('x')));
    assert_eq!(update(&mut m, key(KeyCode::Esc)), vec![]);
    assert!(m.condition().is_empty());
    assert!(m.moment().is_some(), "the decision survived the clear");
    assert_eq!(update(&mut m, key(KeyCode::Esc)), answered(5, "reject"));
}

#[test]
fn always_allow_records_the_scope_it_covers() {
    let mut m = model();
    agent(&mut m, plan_request(6));
    let cmds = update(&mut m, shifted(KeyCode::Char('A')));
    assert_eq!(
        cmds,
        vec![Cmd::Permission {
            id: 6,
            option_id: "allow_always".to_owned(),
        }],
        "the scope is the request's own: the policy reads it off the answer"
    );
    assert!(
        m.rail()
            .charter()
            .iter()
            .any(|rule| rule.subject() == "always allow · goldens"
                && rule.mode() == RuleMode::Auto)
    );
}

/// The charter's offer: its Delegate takes the offer
/// off the rail, notes the row it becomes, and sends the key to the
/// policy's owner; the palette's row is the same verb by key.
#[test]
fn accepting_a_charter_offer_notes_the_row_and_delegates_by_key() {
    let mut m = model();
    m.rail_mut().set_offers(vec![
        crate::cockpit::CharterOffer::new("edit:crates/x", "Edit \u{b7} crates/x", 5, 5),
        crate::cockpit::CharterOffer::new("bash:cargo", "Bash \u{b7} cargo", 7, 7),
    ]);
    assert_eq!(
        update(&mut m, click(Widget::charter_offer(3))),
        vec![],
        "no offer there"
    );
    assert_eq!(
        update(&mut m, click(Widget::charter_offer(1))),
        vec![Cmd::Delegate("bash:cargo".to_owned())]
    );
    assert_eq!(m.rail().offers().len(), 1, "the accepted offer left");
    assert!(
        m.rail()
            .charter()
            .iter()
            .any(|rule| rule.subject() == "Bash \u{b7} cargo" && rule.mode() == RuleMode::Auto),
        "and became a row: {:?}",
        m.rail().charter()
    );
}

#[test]
fn a_once_only_answer_leaves_no_rule_behind() {
    let mut m = model();
    agent(&mut m, plan_request(8));
    assert_eq!(
        update(&mut m, key(KeyCode::Char('a'))),
        answered(8, "allow")
    );
    assert!(
        m.rail().charter().is_empty(),
        "the whole point of once is that it is not a rule"
    );
}

#[test]
fn a_permission_request_takes_the_keys_until_it_is_answered() {
    let mut m = model();
    assert_eq!(m.stage_rows(), 9, "chrome, tab bar and the empty dock");
    agent(&mut m, request(1, "Bash"));
    agent(&mut m, request(2, "Edit"));
    assert_eq!(m.permissions_pending(), 2);
    assert_eq!(m.permissions_seen(), 2);
    assert_eq!(front_title(&m), Some("Bash"));
    assert_eq!(m.stage_rows(), 6, "a row per card inside the panel ring");
    assert_eq!(update(&mut m, key(KeyCode::Char('x'))), vec![]);
    assert!(m.prompt().is_empty(), "letters do not reach the prompt");
    assert_eq!(
        update(&mut m, shifted(KeyCode::Char('D'))),
        vec![],
        "not offered"
    );
    assert_eq!(m.permissions_pending(), 2);
    assert_eq!(
        update(&mut m, shifted(KeyCode::Char('A'))),
        answered(1, "allow_always"),
        "⇧A is the session-wide allow"
    );
    assert_eq!(front_title(&m), Some("Edit"));
    assert_eq!(update(&mut m, key(KeyCode::Esc)), answered(2, "reject"));
    assert_eq!(m.permissions_pending(), 0);
    assert_eq!(m.stage_rows(), 9);
    agent(&mut m, request(3, "Read"));
    assert_eq!(update(&mut m, key(KeyCode::Enter)), answered(3, "allow"));
    agent(&mut m, request(4, "Write"));
    assert_eq!(
        update(&mut m, key(KeyCode::Char('d'))),
        answered(4, "reject")
    );
    agent(&mut m, request(5, "Bash"));
    assert_eq!(
        update(&mut m, key(KeyCode::Char('a'))),
        answered(5, "allow")
    );
    assert_eq!(
        update(&mut m, key(KeyCode::Char('a'))),
        vec![],
        "nothing pending"
    );
    assert_eq!(m.prompt().text(), "a");
}

#[test]
fn a_click_on_a_strip_button_answers_and_other_clicks_do_not() {
    let click = |widget: Widget| Msg::Mouse {
        event: MouseEvent::new(MouseKind::Down(Button::Left), 10, 20),
        target: Some(widget.id()),
    };
    let mut m = model();
    assert_eq!(
        update(&mut m, click(Widget::Allow)),
        vec![],
        "nothing pending"
    );
    agent(&mut m, request(9, "Bash"));
    assert_eq!(update(&mut m, click(Widget::Stage)), vec![]);
    assert_eq!(update(&mut m, click(Widget::Never)), vec![], "not offered");
    assert_eq!(m.permissions_pending(), 1);
    assert_eq!(update(&mut m, click(Widget::Deny)), answered(9, "reject"));
    agent(&mut m, request(10, "Bash"));
    assert_eq!(
        update(&mut m, click(Widget::Always)),
        answered(10, "allow_always")
    );
    agent(&mut m, request(11, "Bash"));
    let release = Msg::Mouse {
        event: MouseEvent::new(MouseKind::Up(Button::Left), 10, 20),
        target: Some(Widget::Allow.id()),
    };
    assert_eq!(update(&mut m, release), vec![], "a release answers nothing");
    assert_eq!(update(&mut m, click(Widget::Allow)), answered(11, "allow"));
    assert_eq!(m.permissions_pending(), 0);
}

#[test]
fn home_and_end_move_the_cursor_with_text_and_scroll_the_stage_without() {
    let mut m = model();
    for i in 0..100 {
        agent(&mut m, AgentEvent::Text(format!("line {i}\n")));
    }
    update(&mut m, key(KeyCode::Home));
    settle(&mut m);
    assert_eq!(m.top_row(), 0);
    update(&mut m, key(KeyCode::End));
    assert!(m.viewport().is_following());
    type_text(&mut m, "abc");
    update(&mut m, key(KeyCode::Home));
    assert_eq!(m.prompt().cursor(), 0);
    assert!(m.viewport().is_following(), "the stage did not move");
    update(&mut m, key(KeyCode::End));
    assert_eq!(m.prompt().cursor(), 3);
    update(&mut m, key(KeyCode::PageUp));
    assert!(!m.viewport().is_following(), "PageUp always scrolls");
}

fn loop_msg(event: LoopEvent) -> Msg {
    Msg::Loop(event)
}

fn frame(family: &str, kind: &str, payload: serde_json::Value) -> Msg {
    Msg::Engine(EngineEvent::local(crate::harness::fixtures::frame(
        family, kind, payload,
    )))
}

#[test]
fn loop_events_fold_into_the_status_and_leave_lines_on_the_stage() {
    use crate::turn::RowKind;
    use std::time::Duration;
    let mut m = model();
    assert_eq!(m.loop_status(), None);
    agent(&mut m, AgentEvent::Text("left over from before".to_owned()));
    update(&mut m, key(KeyCode::PageUp));
    // Events before a start are dropped, notes still land.
    update(&mut m, loop_msg(LoopEvent::Elapsed(Duration::from_secs(3))));
    assert_eq!(m.loop_status(), None);
    update(
        &mut m,
        loop_msg(LoopEvent::Started {
            iteration: 2,
            max: 5,
        }),
    );
    let status = m.loop_status().expect("a loop");
    assert_eq!(status.iteration_text(), "iteration 2/5");
    assert_eq!(m.turn().len(), 1, "the transcript stays");
    assert!(
        m.viewport().is_following(),
        "the window is back at the tail"
    );
    assert_eq!(m.turn_state(), TurnState::Idle);
    update(
        &mut m,
        loop_msg(LoopEvent::Note("starting iteration 2 — 1/3".to_owned())),
    );
    update(
        &mut m,
        loop_msg(LoopEvent::Elapsed(Duration::from_secs(83))),
    );
    update(
        &mut m,
        loop_msg(LoopEvent::GateStarted {
            name: "check".to_owned(),
            job: JobId::new(1),
        }),
    );
    update(
        &mut m,
        loop_msg(LoopEvent::GateStarted {
            name: "test".to_owned(),
            job: JobId::new(2),
        }),
    );
    assert_eq!(m.loop_status().and_then(LoopStatus::gates_passed), None);
    update(
        &mut m,
        Msg::Job {
            id: JobId::new(2),
            output: JobOutput::Failed("3 tests failed".to_owned()),
        },
    );
    update(
        &mut m,
        Msg::Job {
            id: JobId::new(1),
            output: JobOutput::Text("ok".to_owned()),
        },
    );
    update(
        &mut m,
        Msg::Job {
            id: JobId::new(9),
            output: JobOutput::Text("not a gate".to_owned()),
        },
    );
    let status = m.loop_status().expect("a loop");
    let leds: Vec<String> = status
        .gates()
        .iter()
        .map(|gate| gate.text(&crate::theme::Icons::UNICODE))
        .collect();
    assert_eq!(leds, ["✓ check", "✗ test"]);
    assert_eq!(status.gates_passed(), Some(false));
    assert_eq!(status.elapsed_text(), "1m 23s");
    assert_eq!(m.jobs_done(), 3);
    update(&mut m, loop_msg(LoopEvent::Failed("no commit".to_owned())));
    update(
        &mut m,
        loop_msg(LoopEvent::Paused("gates failed".to_owned())),
    );
    assert!(m.loop_status().is_some_and(LoopStatus::is_paused));
    update(&mut m, loop_msg(LoopEvent::Ended("aborted".to_owned())));
    assert_eq!(m.loop_status().and_then(LoopStatus::ended), Some("aborted"));
    let rows: Vec<(RowKind, &str)> = m
        .turn()
        .rows()
        .iter()
        .map(|r| (r.kind, r.text.as_str()))
        .collect();
    assert_eq!(
        rows,
        [
            (
                RowKind::Prose(crate::turn::Voice::Agent),
                "left over from before"
            ),
            (RowKind::Note, "starting iteration 2 — 1/3"),
            (RowKind::Error, "error: no commit"),
            (RowKind::Note, "paused: gates failed"),
            (RowKind::Note, "Work stopped: aborted"),
        ]
    );
    assert_eq!(m.inputs(), 1, "loop events are not inputs");
}

#[test]
fn engine_frames_advance_the_phase_and_a_new_iteration_starts_over() {
    let mut m = model();
    assert_eq!(m.phase(), None);
    update(
        &mut m,
        frame("think", "step_appended", serde_json::json!({"step": {}})),
    );
    assert_eq!(m.phase(), None);
    update(
        &mut m,
        frame(
            "roadmap",
            "chunk_changed",
            serde_json::json!({"chunk": {"id": "a", "status": "in_progress"}}),
        ),
    );
    assert_eq!(m.phase(), Some(DisplayPhase::Research));
    update(
        &mut m,
        frame("ship", "task_started", serde_json::json!({"task_id": "t"})),
    );
    assert_eq!(m.phase(), Some(DisplayPhase::Execute));
    update(
        &mut m,
        frame(
            "ship",
            "objective_set",
            serde_json::json!({"objective": {}}),
        ),
    );
    assert_eq!(m.phase(), Some(DisplayPhase::Execute), "never backwards");
    update(
        &mut m,
        frame(
            "ship",
            "check_recorded",
            serde_json::json!({"task_id": "t", "check": {"passed": true}}),
        ),
    );
    assert_eq!(m.phase(), Some(DisplayPhase::Verify));
    update(
        &mut m,
        frame(
            "ship",
            "objective_shipped",
            serde_json::json!({"warnings": []}),
        ),
    );
    assert_eq!(m.phase(), Some(DisplayPhase::Done));
    update(
        &mut m,
        frame(
            "ship",
            "objective_set",
            serde_json::json!({"objective": {}}),
        ),
    );
    assert_eq!(
        m.phase(),
        Some(DisplayPhase::Plan),
        "after done, a new objective starts over"
    );
    update(
        &mut m,
        loop_msg(LoopEvent::Started {
            iteration: 1,
            max: 1,
        }),
    );
    assert_eq!(m.phase(), None, "a new iteration starts over");
    assert_eq!(m.engine_frames(), 7);
}

#[test]
fn space_pauses_a_running_loop_and_the_paused_keys_decide() {
    let mut m = model();
    let space = key(KeyCode::Char(' '));
    // Without a loop, Space is a character.
    update(&mut m, space.clone());
    assert_eq!(m.prompt().text(), " ");
    update(&mut m, ctrl('u'));
    update(
        &mut m,
        loop_msg(LoopEvent::Started {
            iteration: 1,
            max: 3,
        }),
    );
    type_text(&mut m, "hi");
    update(&mut m, space.clone());
    assert_eq!(
        m.prompt().text(),
        "hi ",
        "Space with text in the prompt is a character"
    );
    assert!(!m.loop_status().is_some_and(LoopStatus::is_paused));
    update(&mut m, ctrl('u'));
    assert_eq!(
        update(&mut m, space.clone()),
        vec![Cmd::Loop(LoopCmd::Pause), Cmd::Animate(Model::TICK_PERIOD)],
        "the pause leaves a note row on the stage, which fades in"
    );
    assert!(m.loop_status().is_some_and(LoopStatus::is_paused));
    assert_eq!(
        update(&mut m, key(KeyCode::Char('q'))),
        vec![],
        "swallowed while paused"
    );
    assert!(m.prompt().is_empty());
    assert_eq!(
        update(&mut m, space.clone()),
        vec![Cmd::Loop(LoopCmd::Resume)]
    );
    assert!(!m.loop_status().is_some_and(LoopStatus::is_paused));
    update(&mut m, space.clone());
    assert_eq!(
        update(&mut m, key(KeyCode::Char('s'))),
        vec![Cmd::Loop(LoopCmd::Step)],
        "s steps (DESIGN.md §6.1); the driver pauses again after the iteration"
    );
    assert!(!m.loop_status().is_some_and(LoopStatus::is_paused));
    update(&mut m, space.clone());
    assert_eq!(
        update(&mut m, key(KeyCode::Char('x'))),
        vec![Cmd::Loop(LoopCmd::Skip)]
    );
    assert!(!m.loop_status().is_some_and(LoopStatus::is_paused));
    update(&mut m, space.clone());
    assert_eq!(
        update(&mut m, key(KeyCode::Esc)),
        vec![Cmd::Loop(LoopCmd::Abort)]
    );
    update(&mut m, space.clone());
    assert_eq!(
        update(
            &mut m,
            Msg::Key(KeyEvent::new(KeyCode::Char('d')).with(Modifiers::CTRL))
        ),
        vec![Cmd::Quit],
        "ctrl-D quits even paused"
    );
    let notes: Vec<&str> = m
        .turn()
        .rows()
        .iter()
        .filter(|r| r.kind == crate::turn::RowKind::Note)
        .map(|r| r.text.as_str())
        .collect();
    assert_eq!(
        notes,
        [
            "pausing at the end of this iteration",
            "resuming",
            "pausing at the end of this iteration",
            "stepping one iteration, then pausing",
            "pausing at the end of this iteration",
            "skipping the next iteration",
            "pausing at the end of this iteration",
            "aborting the loop",
            "pausing at the end of this iteration",
        ]
    );
    // Space pauses ahead of a pending permission; the strip keeps its
    // accelerators, and s / Esc reach the strip before the paused loop.
    let mut m = model();
    update(
        &mut m,
        loop_msg(LoopEvent::Started {
            iteration: 1,
            max: 3,
        }),
    );
    update(&mut m, Msg::Agent(request(1, "Bash")));
    assert_eq!(
        update(&mut m, space.clone()),
        vec![Cmd::Loop(LoopCmd::Pause)]
    );
    assert!(m.loop_status().is_some_and(LoopStatus::is_paused));
    assert_eq!(update(&mut m, key(KeyCode::Esc)), answered(1, "reject"));
    assert!(
        m.loop_status().is_some_and(LoopStatus::is_paused),
        "Esc denied, the loop stays paused"
    );
    assert_eq!(
        update(&mut m, key(KeyCode::Esc)),
        vec![Cmd::Loop(LoopCmd::Abort)]
    );
    // Once the loop ended, the keys are the prompt's again.
    let mut ended = model();
    update(
        &mut ended,
        loop_msg(LoopEvent::Started {
            iteration: 1,
            max: 1,
        }),
    );
    update(&mut ended, loop_msg(LoopEvent::Ended("done".to_owned())));
    assert_eq!(update(&mut ended, space), vec![]);
    assert_eq!(ended.prompt().text(), " ");
}

/// `c` at a held boundary takes a checkpoint of the state as it stands
/// — the ledger head's verb, through the /checkpoint-now fold under
/// the verb's own label — and the loop stays held: a checkpoint is not
/// a decision.
#[test]
fn c_while_paused_takes_a_checkpoint_of_the_held_state() {
    let mut m = model();
    update(
        &mut m,
        loop_msg(LoopEvent::Started {
            iteration: 1,
            max: 3,
        }),
    );
    update(&mut m, key(KeyCode::Char(' ')));
    assert_eq!(
        update(&mut m, key(KeyCode::Char('c'))),
        vec![Cmd::Checkpoint(CheckpointCmd::Save {
            id: "cp-1".to_owned(),
            reason: CheckpointReason::Manual,
            label: "checkpoint now".to_owned(),
        })]
    );
    assert!(m.loop_status().is_some_and(LoopStatus::is_paused));
    assert_eq!(
        m.rail().spine().get("cp-1").map(|cp| cp.label()),
        Some("checkpoint now"),
        "the spine minted the row the port was asked for"
    );
}

/// `⌘.` aborts a live loop from any state on a window — `⌘.` Abort — and is nothing without a loop; a terminal never
/// reports the chord at all.
#[test]
fn cmd_period_aborts_a_live_loop_on_a_window() {
    let chord = || Msg::Key(KeyEvent::new(KeyCode::Char('.')).with(Modifiers::SUPER));
    let mut idle = model().with_surface(crate::cockpit::Surface::Native);
    assert_eq!(update(&mut idle, chord()), vec![]);
    assert!(idle.prompt().is_empty(), "the chord is no letter");
    let mut m = model().with_surface(crate::cockpit::Surface::Native);
    update(
        &mut m,
        loop_msg(LoopEvent::Started {
            iteration: 2,
            max: 5,
        }),
    );
    let cmds = update(&mut m, chord());
    assert!(cmds.contains(&Cmd::Loop(LoopCmd::Abort)), "{cmds:?}");
    assert!(!m.loop_status().is_some_and(LoopStatus::is_paused));
    // The chord is the registry's, so the ledger head's cap and the
    // key the fold takes cannot drift apart.
    assert_eq!(Binding::ABORT_CHORD.code, KeyCode::Char('.'));
    assert_eq!(Binding::ABORT_CHORD.mods, Modifiers::SUPER);
}

/// The ledger head's verbs click through the one loop
/// fold the keys and the ⌘K rows take: Pause toggles with the held
/// state, Step steps, Checkpoint now takes the state; without a live
/// loop the two decisions are nothing.
#[test]
fn the_ledger_heads_verbs_click_through_the_one_loop_fold() {
    let mut m = model().with_surface(crate::cockpit::Surface::Native);
    update(
        &mut m,
        loop_msg(LoopEvent::Started {
            iteration: 2,
            max: 5,
        }),
    );
    assert!(update(&mut m, click(Widget::LoopPause.id())).contains(&Cmd::Loop(LoopCmd::Pause)));
    assert!(m.loop_status().is_some_and(LoopStatus::is_paused));
    assert!(update(&mut m, click(Widget::LoopPause.id())).contains(&Cmd::Loop(LoopCmd::Resume)));
    assert!(!m.loop_status().is_some_and(LoopStatus::is_paused));
    assert!(update(&mut m, click(Widget::LoopStep.id())).contains(&Cmd::Loop(LoopCmd::Step)));
    let cmds = update(&mut m, click(Widget::LoopCheckpoint.id()));
    assert!(
        matches!(
            cmds.as_slice(),
            [Cmd::Checkpoint(CheckpointCmd::Save {
                reason: CheckpointReason::Manual,
                ..
            })]
        ),
        "{cmds:?}"
    );
    // The fourth pair, `⌘.` Abort: the pointer's one
    // abort on a window, whose Esc climbs.
    assert!(update(&mut m, click(Widget::LoopAbort.id())).contains(&Cmd::Loop(LoopCmd::Abort)));
    let mut idle = model().with_surface(crate::cockpit::Surface::Native);
    assert_eq!(update(&mut idle, click(Widget::LoopStep.id())), vec![]);
    assert_eq!(update(&mut idle, click(Widget::LoopPause.id())), vec![]);
    assert_eq!(update(&mut idle, click(Widget::LoopAbort.id())), vec![]);
}

#[test]
fn tab_cycles_the_stage_and_a_click_jumps_to_one() {
    use crate::cockpit::StageTab;
    let mut m = model();
    assert_eq!(m.tab(), StageTab::Turn);
    assert_eq!(
        update(
            &mut m,
            Msg::Key(KeyEvent::new(KeyCode::Tab).with(Modifiers::CTRL))
        ),
        vec![Cmd::Animate(Model::TICK_PERIOD)],
        "the switch transitions"
    );
    assert_eq!(m.tab(), StageTab::Session);
    update(
        &mut m,
        Msg::Key(KeyEvent::new(KeyCode::Tab).with(Modifiers::CTRL)),
    );
    assert_eq!(m.tab(), StageTab::Roadmap);
    let back = Msg::Key(KeyEvent::new(KeyCode::Tab).with(Modifiers::CTRL | Modifiers::SHIFT));
    update(&mut m, back);
    assert_eq!(m.tab(), StageTab::Session);
    let click = Msg::Mouse {
        event: MouseEvent::new(MouseKind::Down(Button::Left), 40, 2),
        target: Some(Widget::TabRoadmap.id()),
    };
    assert_eq!(update(&mut m, click), vec![]);
    assert_eq!(m.tab(), StageTab::Roadmap);
    // Tab still cycles while a permission waits — Roadmap wraps to Turn.
    agent(&mut m, request(30, "Bash"));
    update(
        &mut m,
        Msg::Key(KeyEvent::new(KeyCode::Tab).with(Modifiers::CTRL)),
    );
    assert_eq!(m.tab(), StageTab::Turn);
    assert_eq!(m.permissions_pending(), 1);
}

fn proposal_frame(id: &str) -> Msg {
    frame(
        "roadmap",
        "chunk_changed",
        serde_json::json!({"chunk": {
            "id": id,
            "reprioritize": {"suggested_priority": 300, "reason": "later"},
        }}),
    )
}

fn blocker_frame(id: &str) -> Msg {
    frame(
        "roadmap",
        "chunk_changed",
        serde_json::json!({"chunk": {
            "id": id,
            "blocked_by": {"kind": "awaiting_human", "reason": "needs a decision"},
        }}),
    )
}

#[test]
fn the_engines_frame_is_what_confirms_a_cleared_blocker_not_the_click() {
    // The card leaves the dock the moment it is answered, so the human
    // sees the gesture land — but the pop is optimistic. What decides
    // whether the blocker is really gone is the frame the engine's
    // answer comes back as, folded through the one path the feed uses.
    let mut m = model();
    update(&mut m, blocker_frame("base"));
    assert_eq!(
        update(&mut m, key(KeyCode::Char('c'))),
        vec![Cmd::Engine(EngineCmd::ClearBlocker {
            chunk: "base".to_owned(),
        })]
    );
    assert!(m.dock().is_empty(), "the answered card leaves at once");

    // An engine that did not clear it — another writer put the blocker
    // back, or the answer raced a re-block — says so in its frame, and
    // the card returns rather than the cockpit quietly disagreeing with
    // the store.
    update(&mut m, blocker_frame("base"));
    assert_eq!(m.dock().len(), 1, "the frame still carries the blocker");

    // The engine's real answer: the chunk without its blocker.
    update(
        &mut m,
        frame(
            "roadmap",
            "chunk_changed",
            serde_json::json!({"chunk": {"id": "base"}}),
        ),
    );
    assert!(m.dock().is_empty());
}

#[test]
fn a_front_proposal_answers_on_an_empty_prompt_and_types_otherwise() {
    use crate::cockpit::ProposalKind;
    let mut m = model();
    update(&mut m, proposal_frame("base"));
    assert_eq!(m.dock().len(), 1);
    assert_eq!(
        m.stage_rows(),
        7,
        "one card and the needs-you panel's two border rows"
    );
    // With text in the prompt the letters are the prompt's.
    type_text(&mut m, "xy");
    assert_eq!(m.prompt().text(), "xy");
    assert_eq!(m.dock().len(), 1, "the card stayed");
    update(&mut m, key(KeyCode::Backspace));
    update(&mut m, key(KeyCode::Backspace));
    // On an empty prompt the accelerator answers.
    assert_eq!(
        update(&mut m, key(KeyCode::Char('y'))),
        vec![Cmd::Engine(EngineCmd::ResolveProposal {
            chunk: "base".to_owned(),
            kind: ProposalKind::Reprioritize,
            accept: true,
        })]
    );
    assert!(m.dock().is_empty());
    // Reject by key, too.
    update(&mut m, proposal_frame("other"));
    assert_eq!(
        update(&mut m, key(KeyCode::Char('n'))),
        vec![Cmd::Engine(EngineCmd::ResolveProposal {
            chunk: "other".to_owned(),
            kind: ProposalKind::Reprioritize,
            accept: false,
        })]
    );
}

#[test]
fn a_front_blocker_clears_by_key_and_by_click_and_a_permission_takes_the_front() {
    let mut m = model();
    update(&mut m, blocker_frame("base"));
    assert_eq!(
        update(&mut m, key(KeyCode::Char('c'))),
        vec![Cmd::Engine(EngineCmd::ClearBlocker {
            chunk: "base".to_owned(),
        })]
    );
    assert!(m.dock().is_empty());
    update(&mut m, blocker_frame("other"));
    let click = |widget: Widget| Msg::Mouse {
        event: MouseEvent::new(MouseKind::Down(Button::Left), 10, 21),
        target: Some(widget.id()),
    };
    assert_eq!(
        update(&mut m, click(Widget::Accept)),
        vec![],
        "not a proposal"
    );
    assert_eq!(
        update(&mut m, click(Widget::Clear)),
        vec![Cmd::Engine(EngineCmd::ClearBlocker {
            chunk: "other".to_owned(),
        })]
    );
    // A permission arriving jumps ahead of a waiting proposal and takes
    // the keys whole — its accelerator answers it, not the proposal.
    update(&mut m, proposal_frame("queued"));
    agent(&mut m, request(40, "Bash"));
    assert_eq!(m.dock().len(), 2);
    assert_eq!(
        update(&mut m, key(KeyCode::Char('a'))),
        answered(40, "allow")
    );
    assert_eq!(m.dock().len(), 1, "the proposal still waits");
}

#[test]
fn proposal_and_reject_clicks_answer_the_front_proposal() {
    use crate::cockpit::ProposalKind;
    let mut m = model();
    let click = |widget: Widget| Msg::Mouse {
        event: MouseEvent::new(MouseKind::Down(Button::Left), 10, 21),
        target: Some(widget.id()),
    };
    assert_eq!(update(&mut m, click(Widget::Accept)), vec![], "empty dock");
    update(&mut m, proposal_frame("base"));
    assert_eq!(
        update(&mut m, click(Widget::Accept)),
        vec![Cmd::Engine(EngineCmd::ResolveProposal {
            chunk: "base".to_owned(),
            kind: ProposalKind::Reprioritize,
            accept: true,
        })]
    );
    update(&mut m, proposal_frame("other"));
    assert_eq!(
        update(&mut m, click(Widget::Reject)),
        vec![Cmd::Engine(EngineCmd::ResolveProposal {
            chunk: "other".to_owned(),
            kind: ProposalKind::Reprioritize,
            accept: false,
        })]
    );
}

fn edit_call(id: &str) -> AgentEvent {
    use magistr_application::sessions::{EditDiff, ToolStatus, ToolVerb};
    AgentEvent::ToolCall {
        id: id.to_owned(),
        title: format!("edit src/main.rs ({id})"),
        verb: ToolVerb::Edit,
        status: ToolStatus::Completed,
        diff: Some(EditDiff {
            path: "src/main.rs".to_owned(),
            old_text: "fn main() {\n    old();\n}\n".to_owned(),
            new_text: "fn main() {\n    new();\n}\n".to_owned(),
        }),
        images: Vec::new(),
        record: Default::default(),
    }
}

#[test]
fn an_edit_tool_call_is_a_foldable_diff_card() {
    use crate::turn::RowKind;
    let mut m = model();
    agent(&mut m, edit_call("e1"));
    assert_eq!(stage(&m), ["✓ edit   src/main.rs · +1 −1"]);
    // Enter on the empty prompt unfolds; the diff rows carry their kinds.
    update(&mut m, key(KeyCode::Enter));
    let kinds: Vec<RowKind> = m.turn().rows().iter().map(|r| r.kind).collect();
    assert!(kinds.contains(&RowKind::EditButtons), "{kinds:?}");
    assert!(kinds.contains(&RowKind::DiffAdd), "{kinds:?}");
    assert!(kinds.contains(&RowKind::DiffRemove), "{kinds:?}");
    update(&mut m, key(KeyCode::Enter));
    assert!(
        m.inspector_open(),
        "Enter opens the selected edit in detail"
    );
}

#[test]
fn a_diff_landing_on_an_update_upgrades_the_tool_row() {
    use magistr_application::sessions::{EditDiff, ToolStatus, ToolVerb};
    let mut m = model();
    agent(
        &mut m,
        AgentEvent::ToolCall {
            id: "e1".to_owned(),
            title: "edit src/lib.rs".to_owned(),
            verb: ToolVerb::Edit,
            status: ToolStatus::InProgress,
            diff: None,
            images: Vec::new(),
            record: Default::default(),
        },
    );
    assert_eq!(stage(&m), ["● edit   edit src/lib.rs"]);
    agent(
        &mut m,
        AgentEvent::ToolCallUpdate {
            id: "e1".to_owned(),
            title: None,
            status: Some(ToolStatus::Completed),
            diff: Some(EditDiff {
                path: "src/lib.rs".to_owned(),
                old_text: String::new(),
                new_text: "pub fn one() {}\n".to_owned(),
            }),
            duration_ms: None,
            images: Vec::new(),
            record: Default::default(),
        },
    );
    assert_eq!(stage(&m), ["✓ edit   src/lib.rs · +1 −0"]);
}

#[test]
fn edit_buttons_round_trip_through_a_fake_workbench() {
    use crate::harness::FakeWorkbench;
    use crate::workbench;
    let mut wb = FakeWorkbench::new()
        .with_file("src/main.rs", "fn main() {\n    new();\n}\n")
        .with_edit("e1");
    let mut m = model();
    agent(&mut m, edit_call("e1"));
    update(&mut m, key(KeyCode::Enter));
    let click = |m: &mut Model, id| {
        update(
            m,
            Msg::Mouse {
                event: MouseEvent::new(MouseKind::Down(Button::Left), 10, 5),
                target: Some(id),
            },
        )
    };
    // [Open] reads the file through the port; the answer lands as a note.
    let cmds = click(&mut m, Widget::edit_open(1));
    assert_eq!(
        cmds,
        vec![Cmd::Workbench(WorkbenchCmd::Open {
            path: "src/main.rs".to_owned()
        })]
    );
    let Cmd::Workbench(cmd) = &cmds[0] else {
        unreachable!()
    };
    update(&mut m, Msg::Workbench(workbench::apply(&mut wb, cmd)));
    assert!(
        stage(&m).contains(&"opened src/main.rs · 3 lines"),
        "{:?}",
        stage(&m)
    );
    // [Undo] rewinds the journal; the card reflects it.
    let cmds = click(&mut m, Widget::edit_undo(1));
    assert_eq!(
        cmds,
        vec![Cmd::Workbench(WorkbenchCmd::Undo {
            id: "e1".to_owned()
        })]
    );
    let Cmd::Workbench(cmd) = &cmds[0] else {
        unreachable!()
    };
    update(&mut m, Msg::Workbench(workbench::apply(&mut wb, cmd)));
    assert!(wb.journal_entries().is_empty(), "the journal rewound");
    assert_eq!(
        stage(&m)[0],
        "✓ edit   src/main.rs · +1 −1 · undone",
        "the card wears the rewind"
    );
    assert_eq!(stage(&m)[1], "  [Open]", "the Undo button is gone");
    // A second Undo answers nothing: the card is already undone.
    assert_eq!(click(&mut m, Widget::edit_undo(1)), vec![]);
    assert_eq!(wb.calls(), ["read src/main.rs", "undo e1"]);
}

#[test]
fn a_workbench_refusal_is_an_error_line() {
    let mut m = model();
    update(
        &mut m,
        Msg::Workbench(WorkbenchEvent::Failed {
            cmd: WorkbenchCmd::Undo {
                id: "e9".to_owned(),
            },
            error: "no journal entry for e9".to_owned(),
        }),
    );
    assert_eq!(stage(&m), ["workbench undo e9: no journal entry for e9"]);
    assert_eq!(
        m.changes().refusal("e9"),
        None,
        "an edit the journal never listed carries no refusal"
    );
}

fn files_model() -> Model {
    Model::new(Size::new(80, 24))
        .with_tab(StageTab::Turn)
        .with_workspace_files(
            [
                "src/main.rs",
                "src/cockpit/prompt.rs",
                "src/cockpit/view.rs",
                "README.md",
            ]
            .map(str::to_owned)
            .to_vec(),
        )
}

fn type_chars(m: &mut Model, text: &str) {
    for c in text.chars() {
        update(m, key(KeyCode::Char(c)));
    }
}

#[test]
fn alt_or_shift_enter_breaks_the_line_and_enter_sends_the_whole_text() {
    let mut m = model();
    type_chars(&mut m, "first");
    update(
        &mut m,
        Msg::Key(KeyEvent::new(KeyCode::Enter).with(Modifiers::ALT)),
    );
    type_chars(&mut m, "second");
    update(
        &mut m,
        Msg::Key(KeyEvent::new(KeyCode::Enter).with(Modifiers::SHIFT)),
    );
    type_chars(&mut m, "third");
    assert_eq!(m.prompt().text(), "first\nsecond\nthird");
    assert_eq!(m.prompt_rows(), 3);
    assert_eq!(
        m.stage_rows(),
        7,
        "the growing box costs the stage its rows"
    );
    let cmds = update(&mut m, key(KeyCode::Enter));
    assert_eq!(
        cmds,
        vec![
            Cmd::Prompt("first\nsecond\nthird".to_owned()),
            Cmd::Animate(Model::TICK_PERIOD),
        ]
    );
    assert!(m.prompt().is_empty());
    assert_eq!(m.stage_rows(), 9, "the box shrank back");
}

#[test]
fn a_paste_with_newlines_inserts_verbatim_and_sends_nothing() {
    let mut m = model();
    let cmds = update(&mut m, Msg::Paste("one\ntwo\nthree".to_owned()));
    assert_eq!(cmds, vec![]);
    assert_eq!(m.prompt().text(), "one\ntwo\nthree");
    assert!(m.turn().is_empty(), "nothing echoed, nothing sent");
    assert_eq!(m.prompt_rows(), 3);
}

#[test]
fn at_opens_file_completion_filters_and_tab_accepts_into_the_prompt() {
    let mut m = files_model();
    type_chars(&mut m, "read @");
    let c = m.completion().expect("the popup opened");
    assert_eq!(c.items().len(), 4, "an empty query lists the workspace");
    assert_eq!(m.stage_rows(), 5, "the popup costs the stage its rows");
    type_chars(&mut m, "prom");
    let c = m.completion().expect("still open");
    assert_eq!(c.items(), ["src/cockpit/prompt.rs"]);
    update(&mut m, key(KeyCode::Tab));
    assert!(m.completion().is_none(), "accepting closes the popup");
    assert_eq!(m.prompt().text(), "read @src/cockpit/prompt.rs ");
    let cmds = update(&mut m, key(KeyCode::Enter));
    assert_eq!(
        cmds,
        vec![
            Cmd::Prompt("read @src/cockpit/prompt.rs ".to_owned()),
            Cmd::Animate(Model::TICK_PERIOD),
        ],
        "with the popup closed, enter sends"
    );
}

#[test]
fn the_popup_takes_up_down_enter_and_esc_ahead_of_history_and_send() {
    let mut m = files_model();
    type_chars(&mut m, "@");
    assert_eq!(m.completion().expect("open").selected(), 0);
    update(&mut m, key(KeyCode::Down));
    assert_eq!(m.completion().expect("open").selected(), 1);
    let cmds = update(&mut m, key(KeyCode::Enter));
    assert_eq!(cmds, vec![], "enter accepted a candidate, sent nothing");
    assert_eq!(m.prompt().text(), "@src/cockpit/prompt.rs ");
    assert!(m.turn().is_empty());
    type_chars(&mut m, "@");
    assert!(m.completion().is_some());
    update(&mut m, key(KeyCode::Esc));
    assert!(m.completion().is_none(), "esc closes without accepting");
    assert_eq!(m.prompt().text(), "@src/cockpit/prompt.rs @");
}

#[test]
fn slash_at_a_line_start_opens_commands_and_midline_does_not() {
    let mut m = model();
    type_chars(&mut m, "/ne");
    let c = m.completion().expect("open at the prompt's start");
    assert_eq!(c.items(), ["/next", "/next-loop"]);
    update(&mut m, key(KeyCode::Enter));
    assert!(m.completion().is_none());
    assert_eq!(m.prompt().text(), "/next ");

    let mut m = model();
    type_chars(&mut m, "a/");
    assert!(m.completion().is_none(), "mid-line, `/` is a character");

    let mut m = model();
    type_chars(&mut m, "hi");
    update(
        &mut m,
        Msg::Key(KeyEvent::new(KeyCode::Enter).with(Modifiers::ALT)),
    );
    type_chars(&mut m, "/va");
    assert_eq!(
        m.completion().expect("open on the second line").items(),
        ["/validate"]
    );
}

#[test]
fn the_popup_closes_on_no_match_whitespace_or_a_lost_trigger() {
    let mut m = files_model();
    type_chars(&mut m, "@zzz");
    assert!(m.completion().is_none(), "no match closes");
    let mut m = files_model();
    type_chars(&mut m, "@src ");
    assert!(m.completion().is_none(), "a space closes");
    let mut m = files_model();
    type_chars(&mut m, "@");
    update(&mut m, key(KeyCode::Backspace));
    assert!(m.completion().is_none(), "the trigger died");
    assert!(m.prompt().is_empty());
    let mut m = model();
    type_chars(&mut m, "@");
    assert!(
        m.completion().is_none(),
        "no workspace files, nothing to list"
    );
}

#[test]
fn a_click_on_a_popup_row_accepts_that_candidate() {
    use crate::input::{Button, MouseEvent, MouseKind};
    let mut m = files_model();
    type_chars(&mut m, "@");
    update(
        &mut m,
        Msg::Mouse {
            event: MouseEvent::new(MouseKind::Down(Button::Left), 5, 20),
            target: Some(Widget::completion(3)),
        },
    );
    assert!(m.completion().is_none());
    assert_eq!(m.prompt().text(), "@README.md ");
}

fn journal_entry(id: &str, path: &str) -> crate::workbench::JournalEntry {
    crate::workbench::JournalEntry {
        at_ms: None,
        id: id.to_owned(),
        path: path.to_owned(),
        added: 1,
        removed: 1,
        parse_ok: true,
        diff: "@@ -2 +2 @@\n-    old();\n+    new();".to_owned(),
    }
}

fn change_click(m: &mut Model, id: crate::widget_id::WidgetId) -> Vec<Cmd> {
    update(
        m,
        Msg::Mouse {
            event: MouseEvent::new(MouseKind::Down(Button::Left), 10, 5),
            target: Some(id),
        },
    )
}

#[test]
fn an_agent_edit_lists_its_journal_entry_within_the_frame() {
    use crate::harness::FakeWorkbench;
    use crate::workbench;
    let mut wb = FakeWorkbench::new().with_change(
        journal_entry("e1", "src/main.rs"),
        "fn main() {\n    old();\n}\n",
    );
    let mut m = model();
    // The agent's edit answers a journal refresh in the same update…
    let cmds = update(&mut m, Msg::Agent(edit_call("e1")));
    assert_eq!(
        cmds,
        vec![
            Cmd::Workbench(WorkbenchCmd::Journal),
            Cmd::Animate(Model::TICK_PERIOD),
        ],
        "the turn started streaming, so the header's spinner asks for a tick"
    );
    let Cmd::Workbench(cmd) = &cmds[0] else {
        unreachable!()
    };
    // …and the answer lands before any frame is drawn.
    update(&mut m, Msg::Workbench(workbench::apply(&mut wb, cmd)));
    assert_eq!(m.frames(), 0, "no frame between the edit and the listing");
    assert_eq!(m.changes().entries().len(), 1);
    assert_eq!(m.changes().entries()[0].path, "src/main.rs");
}

#[test]
fn undo_on_a_change_removes_it_from_disk_and_the_list() {
    use crate::harness::FakeWorkbench;
    use crate::workbench;
    let edited = "fn main() {\n    new();\n}\n";
    let original = "fn main() {\n    old();\n}\n";
    let mut wb = FakeWorkbench::new()
        .with_file("src/main.rs", edited)
        .with_change(journal_entry("e1", "src/main.rs"), original);
    let mut m = model();
    update(
        &mut m,
        Msg::Workbench(WorkbenchEvent::Journal {
            entries: vec![journal_entry("e1", "src/main.rs")],
        }),
    );
    assert_eq!(m.changes().entries().len(), 1);
    let cmds = change_click(&mut m, Widget::change_undo(0));
    assert_eq!(
        cmds,
        vec![Cmd::Workbench(WorkbenchCmd::Undo {
            id: "e1".to_owned()
        })]
    );
    let Cmd::Workbench(cmd) = &cmds[0] else {
        unreachable!()
    };
    update(&mut m, Msg::Workbench(workbench::apply(&mut wb, cmd)));
    assert!(
        m.changes().entries().is_empty(),
        "the list no longer has it"
    );
    assert_eq!(
        wb.file("src/main.rs"),
        Some(original),
        "the change left the disk"
    );
}

#[test]
fn a_change_row_click_expands_and_approve_marks_reviewed() {
    let mut m = model();
    update(
        &mut m,
        Msg::Workbench(WorkbenchEvent::Journal {
            entries: vec![journal_entry("e1", "src/main.rs")],
        }),
    );
    assert!(m.selected().is_none());
    change_click(&mut m, Widget::change_row(0));
    assert_eq!(
        m.selected(),
        Some(&super::Selected::Change(
            journal_entry("e1", "src/main.rs").identity()
        ))
    );
    assert_eq!(change_click(&mut m, Widget::change_approve(0)), vec![]);
    assert!(
        m.changes()
            .is_approved(&journal_entry("e1", "src/main.rs").identity())
    );
    change_click(&mut m, Widget::change_row(0));
    assert_eq!(
        m.selected(),
        Some(&super::Selected::Change(
            journal_entry("e1", "src/main.rs").identity()
        ))
    );
}

#[test]
fn open_on_a_change_reads_into_the_viewer_pane() {
    use crate::harness::FakeWorkbench;
    use crate::workbench;
    let mut wb = FakeWorkbench::new().with_file("src/main.rs", "fn main() {}\n");
    let mut m = model();
    update(
        &mut m,
        Msg::Workbench(WorkbenchEvent::Journal {
            entries: vec![journal_entry("e1", "src/main.rs")],
        }),
    );
    let cmds = change_click(&mut m, Widget::change_open(0));
    assert_eq!(
        cmds,
        vec![Cmd::Workbench(WorkbenchCmd::Open {
            path: "src/main.rs".to_owned()
        })]
    );
    let Cmd::Workbench(cmd) = &cmds[0] else {
        unreachable!()
    };
    update(&mut m, Msg::Workbench(workbench::apply(&mut wb, cmd)));
    let (title, lines) = m.changes().viewer().expect("the pane opened");
    assert_eq!(title, "src/main.rs");
    assert_eq!(lines, ["fn main() {}"]);
    assert_eq!(m.tab(), StageTab::Session, "the stage turned to Changes");
    // Esc closes the pane and stays in the cockpit.
    assert_eq!(update(&mut m, key(KeyCode::Esc)), vec![]);
    assert!(m.changes().viewer().is_none());
    assert!(!m.quit_requested());
}

#[test]
fn palette_search_map_read_and_run_reach_the_workbench_not_the_agent() {
    use crate::harness::FakeWorkbench;
    use crate::workbench;
    let mut wb = FakeWorkbench::new()
        .with_file("src/lib.rs", "pub fn alpha() {}\n")
        .with_run(
            "test",
            vec![
                crate::workbench::RunRow {
                    name: "a::passes".to_owned(),
                    passed: true,
                    detail: String::new(),
                },
                crate::workbench::RunRow {
                    name: "b::fails".to_owned(),
                    passed: false,
                    detail: "assertion failed".to_owned(),
                },
            ],
        );
    let mut m = model();
    let send = |m: &mut Model, text: &str| {
        type_chars(m, text);
        update(m, key(KeyCode::Enter))
    };
    // /search goes to the workbench; the turn is not started.
    let cmds = send(&mut m, "/search alpha");
    assert_eq!(
        cmds,
        vec![
            Cmd::Workbench(WorkbenchCmd::Search {
                query: "alpha".to_owned()
            }),
            // The echoed command is a row arriving, which fades in.
            Cmd::Animate(Model::TICK_PERIOD),
        ]
    );
    assert_ne!(m.turn_state(), TurnState::Streaming, "no agent turn");
    let Cmd::Workbench(cmd) = &cmds[0] else {
        unreachable!()
    };
    update(&mut m, Msg::Workbench(workbench::apply(&mut wb, cmd)));
    let (title, lines) = m.changes().viewer().expect("the hits opened");
    assert_eq!(title, "search alpha · 1 hits");
    assert_eq!(lines, ["src/lib.rs:1: pub fn alpha() {}"]);
    update(&mut m, key(KeyCode::Esc));
    // /read opens the file itself.
    let cmds = send(&mut m, "/read src/lib.rs");
    assert_eq!(
        cmds,
        vec![Cmd::Workbench(WorkbenchCmd::Open {
            path: "src/lib.rs".to_owned()
        })]
    );
    // /map answers the workspace map.
    update(&mut m, key(KeyCode::Esc));
    let cmds = send(&mut m, "/map ");
    assert_eq!(cmds, vec![Cmd::Workbench(WorkbenchCmd::Map)]);
    let Cmd::Workbench(cmd) = &cmds[0] else {
        unreachable!()
    };
    update(&mut m, Msg::Workbench(workbench::apply(&mut wb, cmd)));
    let (title, lines) = m.changes().viewer().expect("the map opened");
    assert_eq!(title, "map");
    assert_eq!(lines, ["src/lib.rs"]);
    // /run lands rows on the tab, failing first.
    update(&mut m, key(KeyCode::Esc));
    let cmds = send(&mut m, "/run test");
    assert_eq!(
        cmds,
        vec![Cmd::Workbench(WorkbenchCmd::Run {
            command: "test".to_owned()
        })]
    );
    let Cmd::Workbench(cmd) = &cmds[0] else {
        unreachable!()
    };
    update(&mut m, Msg::Workbench(workbench::apply(&mut wb, cmd)));
    let (command, rows) = m.changes().runs().expect("the rows landed");
    assert_eq!(command, "test");
    let names: Vec<&str> = rows.iter().map(|row| row.name.as_str()).collect();
    assert_eq!(names, ["b::fails", "a::passes"], "failing first");
    assert_eq!(m.tab(), StageTab::Session);
}

#[test]
fn a_journal_refresh_keeps_approvals_for_surviving_entries() {
    let mut m = model();
    update(
        &mut m,
        Msg::Workbench(WorkbenchEvent::Journal {
            entries: vec![
                journal_entry("e1", "src/a.rs"),
                journal_entry("e2", "src/b.rs"),
            ],
        }),
    );
    change_click(&mut m, Widget::change_approve(1));
    update(
        &mut m,
        Msg::Workbench(WorkbenchEvent::Journal {
            entries: vec![journal_entry("e2", "src/b.rs")],
        }),
    );
    assert!(
        m.changes()
            .is_approved(&journal_entry("e2", "src/b.rs").identity())
    );
    assert_eq!(m.changes().entries().len(), 1);
}

// ── The Trace tab ──────────────────────────────────────────────────────

fn think_step_frame(number: u32, payload_extra: serde_json::Value) -> Msg {
    let mut step = serde_json::json!({
        "step_number": number,
        "purpose": "hypothesis",
        "thought": "the cache races the refresh",
        "outcome": "narrowed",
        "confidence": 0.7,
    });
    if let (Some(step), Some(extra)) = (step.as_object_mut(), payload_extra.as_object()) {
        step.extend(extra.clone());
    }
    frame(
        "think",
        "step_appended",
        serde_json::json!({ "step": step }),
    )
}

fn trace_click(m: &mut Model, id: crate::widget_id::WidgetId) -> Vec<Cmd> {
    update(
        m,
        Msg::Mouse {
            event: MouseEvent::new(MouseKind::Down(Button::Left), 10, 5),
            target: Some(id),
        },
    )
}

#[test]
fn a_think_step_frame_lands_in_the_trace_and_a_click_expands_it() {
    let mut m = model();
    update(&mut m, think_step_frame(7, serde_json::json!({})));
    assert_eq!(m.trace().steps().len(), 1, "the step landed on the frame");
    assert_eq!(m.trace().steps()[0].purpose, "hypothesis");
    assert_eq!(trace_click(&mut m, Widget::trace_row(0)), vec![]);
    assert!(m.trace().is_expanded(7));
    trace_click(&mut m, Widget::trace_row(0));
    assert!(m.trace().is_expanded(7), "a second click keeps selection");
}

#[test]
fn a_revision_link_click_navigates_to_its_target() {
    let mut m = model();
    update(&mut m, think_step_frame(4, serde_json::json!({})));
    update(
        &mut m,
        think_step_frame(9, serde_json::json!({"revises_step": 4})),
    );
    assert_eq!(
        m.trace().step(4).expect("step 4").revised_by,
        Some(9),
        "the revising step marked its target"
    );
    trace_click(&mut m, Widget::trace_row(0));
    assert!(m.trace().is_expanded(4));
    assert_eq!(trace_click(&mut m, Widget::trace_link(9)), vec![]);
    assert!(m.trace().is_expanded(9), "the link navigated");
    assert!(!m.trace().is_expanded(4));
}

#[test]
fn pin_clicks_ask_the_engine_and_only_the_answer_flips_the_star() {
    use crate::cockpit::EngineCmd;
    let mut m = model();
    update(&mut m, think_step_frame(7, serde_json::json!({})));
    assert_eq!(
        trace_click(&mut m, Widget::trace_pin(7)),
        vec![Cmd::Engine(EngineCmd::Pin {
            step_number: 7,
            pinned: true,
        })]
    );
    assert!(!m.trace().steps()[0].pinned, "no flip before the answer");
    update(
        &mut m,
        Msg::Engine(EngineEvent::Pinned {
            step_number: 7,
            pinned: true,
        }),
    );
    assert!(m.trace().steps()[0].pinned);
    assert_eq!(
        trace_click(&mut m, Widget::trace_pin(7)),
        vec![Cmd::Engine(EngineCmd::Pin {
            step_number: 7,
            pinned: false,
        })],
        "the next ask is the opposite flag"
    );
    assert_eq!(
        trace_click(&mut m, Widget::trace_pin(99)),
        vec![],
        "an unknown step asks nothing"
    );
}

#[test]
fn checkpoint_round_trips_into_the_viewer_and_esc_closes_it() {
    use crate::cockpit::EngineCmd;
    let mut m = model();
    assert_eq!(
        trace_click(&mut m, Widget::Checkpoint.id()),
        vec![Cmd::Engine(EngineCmd::Checkpoint)]
    );
    update(
        &mut m,
        Msg::Engine(EngineEvent::Checkpoint {
            lines: vec!["open hypotheses: 1".to_owned()],
        }),
    );
    assert_eq!(m.tab(), StageTab::Session, "the report turned the stage");
    assert_eq!(
        m.trace().viewer().expect("the pane opened"),
        ["open hypotheses: 1".to_owned()]
    );
    update(&mut m, key(KeyCode::Esc));
    assert_eq!(m.trace().viewer(), None, "esc closed the pane");
}

#[test]
fn a_port_refusal_is_an_error_line_on_the_stage() {
    let mut m = model();
    update(
        &mut m,
        Msg::Engine(EngineEvent::PortFailed {
            cmd: EngineCmd::Pin {
                step_number: 9,
                pinned: false,
            },
            what: "pin 9".to_owned(),
            error: "no such step".to_owned(),
        }),
    );
    let rows: Vec<&str> = m.turn().rows().iter().map(|r| r.text.as_str()).collect();
    assert_eq!(rows, ["engine pin 9: no such step"]);
}

#[test]
fn ship_action_and_check_frames_join_the_timeline() {
    use crate::cockpit::ShipMoment;
    let mut m = model();
    update(
        &mut m,
        frame(
            "ship",
            "action_recorded",
            serde_json::json!({
                "task_id": "wire",
                "action": {"type": "code", "description": "wired the port"},
            }),
        ),
    );
    update(
        &mut m,
        frame(
            "ship",
            "check_recorded",
            serde_json::json!({
                "check": {"name": "test", "passed": true, "exit_code": 0,
                          "results": {"duration_ms": 5400}},
            }),
        ),
    );
    assert_eq!(m.trace().moments().len(), 2);
    assert_eq!(
        m.trace().moments()[1],
        ShipMoment::Check {
            task_id: String::new(),
            timestamp: String::new(),
            name: "test".to_owned(),
            passed: true,
            verified: true,
            exit_code: Some(0),
            duration_ms: Some(5400),
        }
    );
}

#[test]
fn pin_unpin_and_checkpoint_palette_commands_go_to_the_engine() {
    use crate::cockpit::EngineCmd;
    let mut m = model();
    for (typed, expected) in [
        (
            "/pin 7",
            EngineCmd::Pin {
                step_number: 7,
                pinned: true,
            },
        ),
        (
            "/unpin 7",
            EngineCmd::Pin {
                step_number: 7,
                pinned: false,
            },
        ),
        ("/checkpoint", EngineCmd::Checkpoint),
    ] {
        m.session.prompt.insert(typed);
        // The echo is a row arriving, so the fold may also ask for the
        // tick — only the first does, the rest already have it running.
        let cmds = update(&mut m, key(KeyCode::Enter));
        assert_eq!(cmds.first(), Some(&Cmd::Engine(expected)), "{typed}");
        assert!(
            cmds[1..].iter().all(|cmd| matches!(cmd, Cmd::Animate(_))),
            "{typed}: {cmds:?}"
        );
        assert!(m.session.prompt.is_empty(), "{typed} was taken");
    }
    let echoed: Vec<&str> = m.turn().rows().iter().map(|r| r.text.as_str()).collect();
    assert_eq!(echoed, ["/pin 7", "/unpin 7", "/checkpoint"]);
    // A bare /pin, one without a number, goes to the agent as a prompt.
    m.session.prompt.insert("/pin");
    assert_eq!(
        update(&mut m, key(KeyCode::Enter)),
        vec![Cmd::Prompt("/pin".to_owned())],
        "the tick is already running: the echoes above are still fading in"
    );
    assert!(m.is_animating());
}

/// A model on the Roadmap tab, seeded with three chunks (one in
/// progress), an open reprioritize proposal and a researched signal.
fn roadmap_model() -> Model {
    let mut m = model();
    m = m.with_tab(StageTab::Roadmap);
    let mut proposed = serde_json::json!({
        "id": "ui", "name": "ui", "title": "The UI", "status": "pending",
        "priority": 200, "group": "Views", "deps": ["mid"],
    });
    proposed["reprioritize"] = serde_json::json!({"suggested_priority": 100, "reason": "sooner"});
    update(
        &mut m,
        Msg::Engine(EngineEvent::Seed(
            magistr_workflow_contracts::NativeSnapshot {
                progress: crate::harness::fixtures::progress(1, 3),
                chunks: crate::harness::fixtures::chunks_from(vec![
                    serde_json::json!({"id": "base", "status": "done", "priority": 100, "group": "Core"}),
                    serde_json::json!({"id": "mid", "name": "Run the work", "status": "in_progress", "priority": 100, "group": "Core", "deps": ["base"]}),
                    proposed,
                ]),
                signals: crate::harness::fixtures::signals_from(vec![serde_json::json!({
                    "id": "s1", "kind": "idea", "from": "alice", "body": "faster",
                    "status": "researched",
                })]),
                ..magistr_workflow_contracts::NativeSnapshot::default()
            },
        )),
    );
    m
}

fn left_click(target: WidgetId) -> Msg {
    Msg::Mouse {
        event: MouseEvent::new(MouseKind::Down(Button::Left), 10, 10),
        target: Some(target),
    }
}

/// A hangup marks the projection stale and the reattachment asks the
/// re-seed for a snapshot; over an unchanged store the answer is a
/// confirmation, which restores the projection without touching a view
/// — the open detail stays open, nothing is re-seeded, no frame is
/// counted and nothing is announced.
#[test]
fn a_reconnect_over_an_unchanged_store_is_confirmed_current_and_moves_no_selection() {
    use crate::cockpit::Freshness;

    let mut m = roadmap_model();
    assert_eq!(update(&mut m, left_click(Widget::roadmap_row(0))), vec![]);
    assert_eq!(m.roadmap().selected().map(|c| c.id.as_str()), Some("mid"));
    update(
        &mut m,
        Msg::Engine(EngineEvent::Feed(FeedStatus::Connected {
            reconnect: false,
        })),
    );
    assert_eq!(m.projection().freshness(), Freshness::Current);
    update(
        &mut m,
        Msg::Engine(EngineEvent::Feed(FeedStatus::Disconnected)),
    );
    assert_eq!(m.projection().freshness(), Freshness::Stale);
    update(
        &mut m,
        Msg::Engine(EngineEvent::Feed(FeedStatus::Connected { reconnect: true })),
    );
    assert_eq!(
        m.projection().freshness(),
        Freshness::Stale,
        "reattaching promises nothing; the snapshot it asks for does"
    );

    let (rail, roadmap, trace) = (m.rail().clone(), m.roadmap().clone(), m.trace().clone());
    let frames = m.engine_frames();
    assert_eq!(
        update(&mut m, Msg::Engine(EngineEvent::Confirmed { cursor: None })),
        vec![]
    );
    assert_eq!(m.projection().freshness(), Freshness::Current);
    assert_eq!(
        m.roadmap().selected().map(|c| c.id.as_str()),
        Some("mid"),
        "the selection is stable across a confirmation"
    );
    assert_eq!(m.rail(), &rail);
    assert_eq!(m.roadmap(), &roadmap);
    assert_eq!(m.trace(), &trace);
    assert_eq!(m.engine_frames(), frames, "a confirmation is not a frame");
}

/// A new owner behind the feed marks the projection stale (its frame is
/// folded all the same) and the re-seed is asked; in-order frames after
/// it conceal nothing, and the confirmation that the store stands where
/// the last seed left it is what restores current.
#[test]
fn an_owner_change_is_stale_until_the_resync_confirms_and_in_order_frames_conceal_nothing() {
    use crate::cockpit::Freshness;
    use crate::harness::fixtures;
    use magistr_workflow_contracts::Delivery;

    let changed = |id: &str| {
        fixtures::frame(
            "roadmap",
            "chunk_changed",
            serde_json::json!({"chunk": {"id": id}}),
        )
    };
    let mut m = roadmap_model();
    update(
        &mut m,
        Msg::Engine(EngineEvent::Event {
            event: changed("mid"),
            delivery: Delivery::First,
        }),
    );
    assert_eq!(m.projection().freshness(), Freshness::Current);
    update(
        &mut m,
        Msg::Engine(EngineEvent::Event {
            event: changed("ui"),
            delivery: Delivery::NewEpoch,
        }),
    );
    assert_eq!(m.projection().freshness(), Freshness::Stale);
    update(
        &mut m,
        Msg::Engine(EngineEvent::Event {
            event: changed("base"),
            delivery: Delivery::InOrder,
        }),
    );
    assert_eq!(
        m.projection().freshness(),
        Freshness::Stale,
        "in-order frames after a new owner do not conceal what it may have written"
    );
    update(&mut m, Msg::Engine(EngineEvent::Confirmed { cursor: None }));
    assert_eq!(m.projection().freshness(), Freshness::Current);
}

#[test]
fn roadmap_rows_toggle_and_the_detail_buttons_ask_the_engine() {
    let mut m = roadmap_model();
    // Row 0 is the in-progress chunk; its card offers [Complete].
    assert_eq!(update(&mut m, left_click(Widget::roadmap_row(0))), vec![]);
    assert_eq!(m.roadmap().selected().map(|c| c.id.as_str()), Some("mid"));
    assert_eq!(
        update(&mut m, left_click(Widget::RoadmapComplete.id())),
        vec![Cmd::Engine(EngineCmd::CompleteChunk {
            id: "mid".to_owned(),
        })]
    );
    // Row 1 is pending behind unfinished work; no Start request is available.
    update(&mut m, left_click(Widget::roadmap_row(1)));
    assert_eq!(m.roadmap().selected().map(|c| c.id.as_str()), Some("ui"));
    assert_eq!(
        update(&mut m, left_click(Widget::RoadmapStart.id())),
        vec![]
    );
    // Repeating a selection keeps the inspector stable.
    update(&mut m, left_click(Widget::roadmap_row(1)));
    assert_eq!(m.roadmap().selected().map(|c| c.id.as_str()), Some("ui"));
    m.roadmap.toggle_row(1, 0.0, false);
    m.selected = None;
    assert_eq!(
        update(&mut m, left_click(Widget::RoadmapStart.id())),
        vec![]
    );
}

/// opening a row off the list moves the tech-tree's camera onto
/// its node, which is a glide like any other — it declares the tick on
/// the fold that starts it and gives it back on the step it lands.
#[test]
fn a_row_opened_off_the_list_glides_the_tech_trees_camera() {
    let mut m = roadmap_model();
    // A block far too small for the tree, so the far node is off screen
    // and the camera has somewhere to go.
    m.set_roadmap_graph_rect(Some(crate::layout::Rect::new(2, 4, 8, 4)));
    assert_eq!(
        update(&mut m, left_click(Widget::roadmap_row(1))),
        vec![Cmd::Animate(Model::TICK_PERIOD)],
        "the camera asks for the tick"
    );
    assert!(m.roadmap().is_gliding((m.ticks()) as f32));
    for _ in 0..Eased::STEPS - 1 {
        assert_eq!(update(&mut m, Msg::Tick), vec![]);
    }
    assert_eq!(
        update(&mut m, Msg::Tick),
        vec![Cmd::Still],
        "and gives it back the step it lands"
    );
    assert!(!m.roadmap().is_gliding((m.ticks()) as f32));
}

/// The same click under reduced motion: the camera is already where it
/// was asked to be, and no tick is ever taken.
#[test]
fn a_row_opened_under_reduced_motion_never_starts_the_cameras_tick() {
    let mut m = roadmap_model().with_theme(Theme::default().with_motion(MotionMode::Reduced));
    m.set_roadmap_graph_rect(Some(crate::layout::Rect::new(2, 4, 8, 4)));
    assert_eq!(update(&mut m, left_click(Widget::roadmap_row(1))), vec![]);
    assert!(!m.roadmap().is_gliding((m.ticks()) as f32));
    assert_eq!(m.ticks(), 0);
}

#[test]
fn roadmap_proposal_and_signal_buttons_ask_the_engine() {
    use crate::cockpit::ProposalKind;
    let mut m = roadmap_model();
    assert_eq!(
        update(&mut m, left_click(Widget::proposal_accept(0))),
        vec![Cmd::Engine(EngineCmd::ResolveProposal {
            chunk: "ui".to_owned(),
            kind: ProposalKind::Reprioritize,
            accept: true,
        })]
    );
    assert_eq!(
        update(&mut m, left_click(Widget::proposal_reject(0))),
        vec![Cmd::Engine(EngineCmd::ResolveProposal {
            chunk: "ui".to_owned(),
            kind: ProposalKind::Reprioritize,
            accept: false,
        })]
    );
    assert_eq!(
        update(&mut m, left_click(Widget::signal_promote(0))),
        vec![Cmd::Engine(EngineCmd::PromoteSignal {
            id: "s1".to_owned(),
        })]
    );
    assert_eq!(
        update(&mut m, left_click(Widget::signal_ignore(0))),
        vec![Cmd::Engine(EngineCmd::IgnoreSignal {
            id: "s1".to_owned(),
        })]
    );
    assert_eq!(
        update(&mut m, left_click(Widget::signal_snooze(0))),
        vec![Cmd::Engine(EngineCmd::SnoozeSignal {
            id: "s1".to_owned(),
            minutes: 60,
        })]
    );
    // Past the lists, nothing is asked.
    assert_eq!(
        update(&mut m, left_click(Widget::proposal_accept(9))),
        vec![]
    );
    assert_eq!(
        update(&mut m, left_click(Widget::signal_promote(9))),
        vec![]
    );
}

/// \[Research\] is the agent's work: the click sends a prompt naming the
/// signal in its own words, echoed on the stage like a typed one, and
/// while that turn streams a second press asks nothing — the engine is
/// never handed an enrichment it did not earn.
#[test]
fn the_research_button_asks_the_agent_not_the_engine() {
    let mut m = roadmap_model();
    let rows = m.turn().len();
    let cmds = update(&mut m, left_click(Widget::signal_research(0)));
    // The echo row fades in, so the tick rides along with the prompt.
    let Some(Cmd::Prompt(text)) = cmds.iter().find(|c| matches!(c, Cmd::Prompt(_))) else {
        panic!("a prompt, got {cmds:?}");
    };
    assert!(
        !cmds.iter().any(|c| matches!(c, Cmd::Engine(_))),
        "nothing is asked of the engine: {cmds:?}"
    );
    assert!(text.starts_with("Research signal s1"), "{text}");
    assert!(text.contains("faster"), "the signal's own words: {text}");
    assert!(text.contains("signal_research"), "{text}");
    assert!(m.turn().len() > rows, "the ask is echoed on the stage");
    assert_eq!(
        update(&mut m, left_click(Widget::signal_research(0))),
        vec![],
        "refused while the turn streams"
    );
    assert_eq!(
        update(&mut m, left_click(Widget::signal_research(9))),
        vec![],
        "past the list, nothing is asked"
    );
}

#[test]
fn a_port_answer_updates_the_tab_and_the_rail_within_one_frame() {
    let mut m = roadmap_model();
    assert_eq!(m.rail().chunk(), Some("mid"));
    // The port's synthesized frame after [Complete] on mid.
    update(
        &mut m,
        Msg::Engine(EngineEvent::local(crate::harness::fixtures::frame(
            "roadmap",
            "chunk_completed",
            serde_json::json!({"chunk": {"id": "mid", "status": "done"}}),
        ))),
    );
    assert_eq!(
        m.roadmap().chunk("mid").map(|c| c.state),
        Some(crate::cockpit::ChunkState::Done),
        "the tab folded the answer"
    );
    assert_eq!(m.rail().chunk(), None, "the rail let go of you-are-here");
    assert_eq!(m.rail().progress(), (2, 3), "done advanced");
    // A signal answer likewise empties the inbox.
    update(
        &mut m,
        Msg::Engine(EngineEvent::local(crate::harness::fixtures::frame(
            "signal",
            "signal_dismissed",
            serde_json::json!({"signal": {"id": "s1", "status": "dismissed"}}),
        ))),
    );
    assert!(m.roadmap().signals().is_empty());
}

#[test]
fn a_mouse_move_hovers_its_target_and_a_bare_move_clears_it() {
    let mut m = Model::new(Size::new(120, 40));
    update(
        &mut m,
        Msg::Mouse {
            event: MouseEvent::new(MouseKind::Move, 10, 10),
            target: Some(Widget::launch_session(1)),
        },
    );
    assert_eq!(m.hovered(), Some(Widget::launch_session(1)));
    update(
        &mut m,
        Msg::Mouse {
            event: MouseEvent::new(MouseKind::Move, 11, 10),
            target: None,
        },
    );
    assert_eq!(m.hovered(), None);
}

#[test]
fn the_tech_tree_takes_clicks_pans_and_hovers_through_its_reported_rect() {
    let mut m = roadmap_model();
    m.set_roadmap_graph_rect(Some(crate::layout::Rect::new(2, 4, 60, 12)));
    let nodes = m.roadmap().dag_nodes();
    // Click the cell whose centre is nearest the second node (mid).
    let (col, row) = (
        (f32::from(2u16) + nodes[1].x - 0.5) as u16,
        (f32::from(4u16) + nodes[1].y - 0.5) as u16,
    );
    update(
        &mut m,
        Msg::Mouse {
            event: MouseEvent::new(MouseKind::Down(Button::Left), col, row),
            target: Some(Widget::RoadmapGraph.id()),
        },
    );
    assert_eq!(
        m.roadmap().selected().map(|c| c.id.as_str()),
        Some("mid"),
        "the click selected the node"
    );
    // In a roomy viewport the fitted tree cannot pan — the drag holds.
    update(
        &mut m,
        Msg::Mouse {
            event: MouseEvent::new(MouseKind::Drag(Button::Left), col - 4, row),
            target: Some(Widget::RoadmapGraph.id()),
        },
    );
    assert_eq!(m.roadmap().pan(), (0.0, 0.0), "a fitted tree stays put");
    update(
        &mut m,
        Msg::Mouse {
            event: MouseEvent::new(MouseKind::Up(Button::Left), col - 4, row),
            target: Some(Widget::RoadmapGraph.id()),
        },
    );
    // A tiny viewport floors the pitch; the overflow pans on drag. The
    // press lands nearest the deepest node and moves the selection.
    m.set_roadmap_graph_rect(Some(crate::layout::Rect::new(2, 4, 8, 4)));
    let before = m.roadmap().dag_nodes()[0].x;
    update(
        &mut m,
        Msg::Mouse {
            event: MouseEvent::new(MouseKind::Down(Button::Left), 8, 5),
            target: Some(Widget::RoadmapGraph.id()),
        },
    );
    assert_eq!(m.roadmap().selected().map(|c| c.id.as_str()), Some("ui"));
    update(
        &mut m,
        Msg::Mouse {
            event: MouseEvent::new(MouseKind::Drag(Button::Left), 4, 5),
            target: Some(Widget::RoadmapGraph.id()),
        },
    );
    assert!(m.roadmap().dag_nodes()[0].x < before, "the overflow panned");
    update(
        &mut m,
        Msg::Mouse {
            event: MouseEvent::new(MouseKind::Up(Button::Left), 4, 5),
            target: Some(Widget::RoadmapGraph.id()),
        },
    );
    // A hover over a node shows its title line.
    m.set_roadmap_graph_rect(Some(crate::layout::Rect::new(2, 4, 60, 12)));
    let nodes = m.roadmap().dag_nodes();
    let (col, row) = (
        (f32::from(2u16) + nodes[2].x - 0.5) as u16,
        (f32::from(4u16) + nodes[2].y - 0.5) as u16,
    );
    update(
        &mut m,
        Msg::Mouse {
            event: MouseEvent::new(MouseKind::Move, col, row),
            target: Some(Widget::RoadmapGraph.id()),
        },
    );
    assert_eq!(
        m.roadmap().hovered_title().as_deref(),
        Some("ui — The UI"),
        "hover shows the title"
    );
    // The legend gutter sits left of the reported rect: a click or a
    // move there must neither select nor hover a node.
    update(
        &mut m,
        Msg::Mouse {
            event: MouseEvent::new(MouseKind::Move, 0, row),
            target: Some(Widget::RoadmapGraph.id()),
        },
    );
    assert_eq!(
        m.roadmap().hovered_title().as_deref(),
        Some("ui — The UI"),
        "a gutter move changes nothing"
    );
    update(
        &mut m,
        Msg::Mouse {
            event: MouseEvent::new(MouseKind::Down(Button::Left), 0, row),
            target: Some(Widget::RoadmapGraph.id()),
        },
    );
    assert_eq!(
        m.roadmap().selected().map(|c| c.id.as_str()),
        Some("ui"),
        "a gutter click selects nothing new"
    );
}

#[test]
fn a_phase_moving_frame_announces_the_phase_once() {
    let mut m = model();
    let payload = serde_json::json!({"objective": {"description": "goal"}});
    let cmds = update(&mut m, frame("ship", "objective_set", payload.clone()));
    assert_eq!(
        cmds,
        vec![
            Cmd::Announce("phase plan".to_owned()),
            // The frame left an event row on the stage, and a row that
            // arrives fades in — so the fold asks for the tick.
            Cmd::Animate(Model::TICK_PERIOD),
        ]
    );
    assert_eq!(
        update(&mut m, frame("ship", "objective_set", payload)),
        Vec::new(),
        "the same phase again is quiet"
    );
}

#[test]
fn a_backwards_phase_is_quiet() {
    let mut m = model();
    let cmds = update(
        &mut m,
        frame(
            "ship",
            "task_started",
            serde_json::json!({"task": {"id": "t"}}),
        ),
    );
    assert_eq!(
        cmds,
        vec![
            Cmd::Announce("phase execute".to_owned()),
            Cmd::Animate(Model::TICK_PERIOD),
        ]
    );
    assert_eq!(
        update(
            &mut m,
            frame(
                "ship",
                "objective_set",
                serde_json::json!({"objective": {"description": "goal"}}),
            ),
        ),
        Vec::new(),
        "a frame implying an earlier phase announces nothing"
    );
}

#[test]
fn a_new_needs_you_card_announces_itself_and_its_upsert_stays_quiet() {
    let mut m = model();
    let payload = serde_json::json!({"chunk": {
        "id": "c1",
        "status": "pending",
        "title_proposal": {"suggested_title": "better", "reason": "clearer"},
    }});
    let cmds = update(&mut m, frame("roadmap", "chunk_changed", payload.clone()));
    assert_eq!(
        cmds,
        vec![
            Cmd::Announce("needs you: ◆ c1: title → better (clearer)".to_owned()),
            Cmd::Animate(Model::TICK_PERIOD),
        ],
        "the card's arrival transitions, so the tick starts"
    );
    assert_eq!(
        update(&mut m, frame("roadmap", "chunk_changed", payload)),
        Vec::new(),
        "the frame re-stating the same card is quiet"
    );
}

#[test]
fn a_permission_request_announces_itself() {
    let mut m = model();
    let cmds = update(&mut m, Msg::Agent(request(1, "Edit main.rs")));
    assert_eq!(
        cmds,
        vec![
            Cmd::Announce("needs you: Edit main.rs".to_owned()),
            Cmd::Animate(Model::TICK_PERIOD),
        ]
    );
}

/// The transport denied an ask nobody answered (fail closed): the card
/// leaves the dock by its id — even behind the front — the stage says
/// so in the wait's own words, and a lapse no card wears is quiet.
#[test]
fn a_lapsed_permission_leaves_the_dock_and_the_stage_says_it_was_denied() {
    let mut m = model();
    agent(&mut m, request(1, "Bash"));
    agent(&mut m, request(2, "Edit main.rs"));
    let lapse = |id, title: &str| AgentEvent::PermissionLapsed {
        id,
        title: title.to_owned(),
        after_secs: 120,
    };
    assert_eq!(
        update(&mut m, Msg::Agent(lapse(2, "Edit main.rs"))),
        vec![Cmd::Announce(
            "denied after 2 min: nobody answered Edit main.rs".to_owned()
        )]
    );
    assert_eq!(m.permissions_pending(), 1);
    assert_eq!(front_title(&m), Some("Bash"));
    // The note wraps at the 80-column stage; read it whole.
    assert!(
        stage(&m)
            .join(" ")
            .contains("denied after 2 min: nobody answered Edit main.rs"),
        "{:?}",
        stage(&m)
    );
    assert_eq!(update(&mut m, Msg::Agent(lapse(9, "nobody"))), vec![]);
    assert_eq!(m.permissions_pending(), 1);
}

#[test]
fn cancelling_a_permission_removes_its_card_without_claiming_a_timeout_denial() {
    let mut m = model();
    agent(&mut m, request(1, "Bash"));
    agent(&mut m, request(2, "Edit main.rs"));
    m.decision.stand_down(2);
    agent(&mut m, AgentEvent::PermissionCancelled { id: 2 });
    assert_eq!(m.permissions_pending(), 1);
    assert_eq!(front_title(&m), Some("Bash"));
    assert!(!m.decision().is_stood_down(2));
    assert!(!stage(&m).join(" ").contains("denied after"));
    agent(&mut m, AgentEvent::PermissionCancelled { id: 1 });
    assert_eq!(m.permissions_pending(), 0);
    assert_eq!(
        update(
            &mut m,
            Msg::Agent(AgentEvent::PermissionCancelled { id: 99 })
        ),
        vec![],
        "late cancellation of an already-removed card stays quiet"
    );
}

// ── The motion layer ─────────────────────────────────────────────────

/// a scroll asks the runtime for a tick and gets it back the
/// step the paint catches up — the same lifecycle the transitions have,
/// so an idle cockpit is still at zero frames a second after a scroll.
#[test]
fn a_scroll_starts_the_tick_and_the_last_step_of_the_glide_takes_it_back() {
    let mut m = model();
    for i in 0..100 {
        agent(&mut m, AgentEvent::Text(format!("line {i}\n")));
    }
    // The turn is over and its rows have finished arriving, so the glide
    // is the only thing that could be moving.
    agent(
        &mut m,
        AgentEvent::TurnEnded(magistr_application::sessions::TurnEnd::EndTurn),
    );
    settle(&mut m);
    assert!(!m.is_animating(), "nothing moves before the scroll");
    assert_eq!(
        update(&mut m, key(KeyCode::PageUp)),
        vec![Cmd::Animate(Model::TICK_PERIOD)],
        "the scroll asks for the tick"
    );
    let mut still = Vec::new();
    for _ in 0..Viewport::GLIDE_STEPS {
        still = update(&mut m, Msg::Tick);
    }
    assert_eq!(still, vec![Cmd::Still], "the last step takes the tick back");
    assert!(!m.is_animating());
}

/// One scrolling viewport under test: its name, how the model reports
/// the row it paints from, and a model showing it with room to scroll.
type Scrolled = (&'static str, fn(&Model) -> usize, fn() -> Model);

/// Every viewport glides, and each one lands exactly where the jump used
/// to put it. The four are wired through `scrolled_viewport`, so this
/// walks all four rather than trusting one to stand for the rest.
#[test]
fn a_wheel_notch_glides_every_tab_to_the_window_the_jump_used_to_cut_to() {
    let turn: fn() -> Model = || {
        let mut m = model();
        for i in 0..200 {
            agent(&mut m, AgentEvent::Text(format!("line {i}\n")));
        }
        agent(
            &mut m,
            AgentEvent::TurnEnded(magistr_application::sessions::TurnEnd::EndTurn),
        );
        // Off the tail, so a notch down has somewhere to go.
        update(&mut m, key(KeyCode::PageUp));
        m
    };
    let changes: fn() -> Model = || {
        let mut m = model();
        m.tab = StageTab::Session;
        let entries: Vec<crate::workbench::JournalEntry> = (0..200)
            .map(|n| journal_entry(&format!("e{n}"), &format!("src/f{n}.rs")))
            .collect();
        update(&mut m, Msg::Workbench(WorkbenchEvent::Journal { entries }));
        update(&mut m, key(KeyCode::PageUp));
        m
    };
    let trace: fn() -> Model = || {
        let mut m = long_trace_tab(60);
        // Off the tail, like the turn: this viewport follows it.
        update(&mut m, key(KeyCode::PageUp));
        m
    };
    let cases: [Scrolled; 4] = [
        ("turn", Model::top_row, turn),
        ("trace", Model::session_top, trace),
        ("changes", Model::session_top, changes),
        ("roadmap", Model::roadmap_top, || long_roadmap(110)),
    ];
    for (name, top, make) in cases {
        let mut m = make();
        settle(&mut m);
        assert!(!m.is_animating(), "{name}: nothing moves before the scroll");
        let before = top(&m);
        let wheel = Msg::Mouse {
            event: MouseEvent::new(MouseKind::ScrollDown, 5, 20),
            target: Some(Widget::Stage.id()),
        };
        update(&mut m, wheel);
        // A cell surface holds each frame for a tick, so the notch's
        // own frame already shows its rows (stage-scroll-anchor-and-rate)
        // — and the glide's tick runs its two steps, then stops.
        assert_eq!(
            top(&m),
            before + Viewport::WHEEL,
            "{name}: on screen the frame it was turned"
        );
        assert!(m.is_animating(), "{name}: the glide's tick runs");
        for _ in 0..Viewport::GLIDE_STEPS {
            update(&mut m, Msg::Tick);
        }
        assert_eq!(
            top(&m),
            before + Viewport::WHEEL,
            "{name}: lands a notch on, exactly where the cut used to"
        );
        assert!(!m.is_animating(), "{name}: and the tick stops");
    }
    // A page has further to go than a notch: the frame held on the way
    // is one intermediate window, and the next is the page's own.
    let mut m = turn();
    settle(&mut m);
    let before = m.top_row();
    update(&mut m, key(KeyCode::PageUp));
    let landed = m.viewport().top(m.turn().len(), m.stage_rows());
    let first = m.top_row();
    assert!(
        first > landed && first < before,
        "one intermediate frame: {first} between {landed} and {before}"
    );
    update(&mut m, Msg::Tick);
    assert_eq!(m.top_row(), landed, "and the page's own window next");
}

/// Under reduced motion a scroll is one frame: the window and the paint
/// move together, and no tick is ever asked for.
#[test]
fn reduced_motion_scrolls_the_stage_in_one_frame() {
    let mut m = model().with_theme(Theme::default().with_motion(MotionMode::Reduced));
    for i in 0..200 {
        agent(&mut m, AgentEvent::Text(format!("line {i}\n")));
    }
    let before = m.top_row();
    let cmds = update(&mut m, key(KeyCode::PageUp));
    assert!(
        !cmds.contains(&Cmd::Animate(Model::TICK_PERIOD)),
        "no tick asked for: {cmds:?}"
    );
    assert!(m.top_row() < before, "there in one frame");
    assert_eq!(
        m.top_row(),
        m.viewport().top(m.turn().len(), m.stage_rows())
    );
    assert!(!m.is_animating());
}

#[test]
fn a_tab_switch_transitions_on_the_tick_and_goes_still_when_it_is_over() {
    let mut m = model();
    assert_eq!(
        update(
            &mut m,
            Msg::Key(KeyEvent::new(KeyCode::Tab).with(Modifiers::CTRL))
        ),
        vec![Cmd::Animate(Model::TICK_PERIOD)],
        "the switch starts a transition"
    );
    assert_eq!(m.motion().tab_transition((m.ticks()) as f32), Some(0.0));
    let mut still = Vec::new();
    for _ in 0..Motion::TRANSITION_STEPS {
        still = update(&mut m, Msg::Tick);
    }
    assert_eq!(still, vec![Cmd::Still], "the last step takes the tick back");
    assert_eq!(m.motion().tab_transition((m.ticks()) as f32), None);
    assert!(!m.is_animating());
}

#[test]
fn progress_eases_on_the_tick_and_seeds_in_place() {
    let mut m = model();
    let seed = Msg::Engine(EngineEvent::Seed(
        magistr_workflow_contracts::NativeSnapshot {
            progress: crate::harness::fixtures::progress(2, 4),
            ..magistr_workflow_contracts::NativeSnapshot::default()
        },
    ));
    assert_eq!(
        update(&mut m, seed),
        vec![],
        "a seed lands in place: nothing animates"
    );
    assert_eq!(m.motion().rail_fill((m.ticks()) as f32), 0.5);
    assert_eq!(
        update(
            &mut m,
            frame(
                "roadmap",
                "chunk_completed",
                serde_json::json!({"chunk": {"id": "x"}})
            )
        ),
        vec![Cmd::Animate(Model::TICK_PERIOD)],
        "a completion glides the fill"
    );
    assert_eq!(
        m.motion().rail_fill((m.ticks()) as f32),
        0.5,
        "from where it was"
    );
    let mut last = Vec::new();
    for _ in 0..Eased::STEPS {
        last = update(&mut m, Msg::Tick);
    }
    assert_eq!(
        m.motion().rail_fill((m.ticks()) as f32),
        0.75,
        "the fill lands"
    );
    // The fill is not the only thing a completion started: the ship
    // moment on the Sky outlasts it, so the tick keeps running until
    // the flare and the edge-run are both over.
    assert!(m.is_animating());
    for _ in 0..ShipRun::STEPS - Eased::STEPS {
        last = update(&mut m, Msg::Tick);
    }
    assert_eq!(last, vec![Cmd::Still], "and stops when the last one does");
    assert_eq!(m.motion().rail_fill((m.ticks()) as f32), 0.75);
    assert!(!m.is_animating());
}

#[test]
fn reduced_motion_never_starts_the_tick() {
    let mut m = model().with_theme(Theme::default().with_motion(MotionMode::Reduced));
    assert_eq!(
        update(
            &mut m,
            Msg::Key(KeyEvent::new(KeyCode::Tab).with(Modifiers::CTRL))
        ),
        vec![],
        "no transition"
    );
    type_text(&mut m, "go");
    assert_eq!(
        update(&mut m, key(KeyCode::Enter)),
        vec![Cmd::Prompt("go".to_owned())],
        "streaming asks for no spinner"
    );
    assert!(!m.is_animating());
    assert_eq!(
        update(&mut m, Msg::Agent(request(1, "Edit main.rs"))),
        vec![Cmd::Announce("needs you: Edit main.rs".to_owned())]
    );
    assert_eq!(m.motion().dock_transition((m.ticks()) as f32), None);
    assert_eq!(m.ticks(), 0);
}

// ── Native launcher, terminal host commands and keyboard help ───────────────

#[test]
fn a_launcher_job_populates_the_cards() {
    let mut m = model();
    assert_eq!(m.launcher(), None);
    let data = crate::cockpit::LauncherData {
        project: "magistr".to_owned(),
        has_config: true,
        stack: "Rust / Cargo".to_owned(),
        git: "master, clean".to_owned(),
        backend: "ACP (available)".to_owned(),
        sessions: vec![crate::cockpit::LauncherSession {
            id: Some("sess-1".to_owned()),
            when: "Aug 28 14:02".to_owned(),
            duration: "2m 30s".to_owned(),
            outcome: crate::cockpit::SessionOutcome::Success,
            preview: "shipped the launcher".to_owned(),
        }],
    };
    update(
        &mut m,
        Msg::Job {
            id: JobId::new(0),
            output: JobOutput::Launcher(Box::new(data.clone())),
        },
    );
    assert_eq!(m.launcher(), Some(&data), "one round-trip fills the cards");
}

#[test]
fn terminal_launch_actions_dispatch_from_footer_palette_and_chords() {
    for (index, c, query, action) in [
        (0, 'n', "next", LaunchAction::Next),
        (1, 'l', "continue work", LaunchAction::Loop),
        (2, 'g', "run checks", LaunchAction::Gates),
    ] {
        for tab in StageTab::ALL {
            let mut m = crate::harness::Scenes::launcher(Size::new(120, 40)).with_tab(tab);
            let commands = change_click(&mut m, Widget::footer_chip(index));
            assert_eq!(commands, vec![Cmd::Quit]);
            assert_eq!(m.launch(), Some(&action));

            let mut m = crate::harness::Scenes::launcher(Size::new(120, 40)).with_tab(tab);
            type_text(&mut m, "keep this draft");
            update(&mut m, ctrl('p'));
            type_text(&mut m, query);
            assert_eq!(update(&mut m, key(KeyCode::Enter)), vec![Cmd::Quit]);
            assert_eq!(m.launch(), Some(&action));
            assert_eq!(m.prompt().text(), "keep this draft");

            let mut m = crate::harness::Scenes::launcher(Size::new(120, 40)).with_tab(tab);
            assert_eq!(
                update(
                    &mut m,
                    Msg::Key(KeyEvent::new(KeyCode::Char(c)).with(Modifiers::ALT))
                ),
                vec![Cmd::Quit]
            );
            assert_eq!(m.launch(), Some(&action));
        }
    }
}

#[test]
fn terminal_palette_closes_without_mutating_draft_or_view() {
    let mut m = model().with_tab(StageTab::Roadmap);
    type_text(&mut m, "draft");
    update(&mut m, ctrl('p'));
    type_text(&mut m, "turn");
    update(&mut m, key(KeyCode::Esc));
    assert!(m.palette().is_none());
    assert_eq!(m.prompt().text(), "draft");
    assert_eq!(m.tab(), StageTab::Roadmap);
}

#[test]
fn launcher_buttons_click() {
    let click = |widget: Widget| Msg::Mouse {
        event: MouseEvent::new(MouseKind::Down(Button::Left), 10, 10),
        target: Some(widget.id()),
    };
    let mut m = model().with_tab(StageTab::Roadmap);
    assert_eq!(update(&mut m, click(Widget::LaunchNext)), vec![Cmd::Quit]);
    assert_eq!(m.launch(), Some(&LaunchAction::Next));

    let mut m = model().with_tab(StageTab::Roadmap);
    assert_eq!(update(&mut m, click(Widget::LaunchLoop)), vec![Cmd::Quit]);
    assert_eq!(m.launch(), Some(&LaunchAction::Loop));

    let mut m = model().with_tab(StageTab::Roadmap);
    assert_eq!(update(&mut m, click(Widget::LaunchGates)), vec![Cmd::Quit]);
    assert_eq!(m.launch(), Some(&LaunchAction::Gates));

    // The roadmap button is a tab switch, not an exit.
    let mut m = model().with_tab(StageTab::Roadmap);
    assert_eq!(update(&mut m, click(Widget::LaunchRoadmap)), vec![]);
    assert_eq!(m.tab(), StageTab::Roadmap);
    assert_eq!(m.launch(), None);
}

#[test]
fn a_session_row_resumes_by_click_and_by_enter_on_the_selection() {
    // A left click on the row hands the entry point the row's session
    // id beside the quit.
    let mut m = launcher_model();
    assert_eq!(
        update(&mut m, session_click(0)),
        vec![Cmd::Quit],
        "the row quits the launcher"
    );
    assert_eq!(m.launch(), Some(&LaunchAction::Resume("sess-1".to_owned())));
    assert!(m.quit_requested());

    // The arrows walk the card: down opens on the first row, down
    // again moves to the second, up comes back — and Enter resumes
    // whatever is selected.
    let mut m = launcher_model();
    assert_eq!(m.session_selected(), None, "nothing is selected at rest");
    update(&mut m, key(KeyCode::Down));
    assert_eq!(m.session_selected(), Some(0));
    update(&mut m, key(KeyCode::Down));
    assert_eq!(m.session_selected(), Some(1));
    update(&mut m, key(KeyCode::Down));
    assert_eq!(m.session_selected(), Some(1), "the last row is the end");
    update(&mut m, key(KeyCode::Up));
    assert_eq!(m.session_selected(), Some(0));
    assert_eq!(update(&mut m, key(KeyCode::Enter)), vec![Cmd::Quit]);
    assert_eq!(m.launch(), Some(&LaunchAction::Resume("sess-1".to_owned())));

    // Up from nothing selected opens on the last row.
    let mut m = launcher_model();
    update(&mut m, key(KeyCode::Up));
    assert_eq!(m.session_selected(), Some(1));
}

#[test]
fn a_session_row_with_no_id_reports_on_the_card_instead_of_quitting() {
    // The second row's session is gone from disk: the entry point gave
    // it no id, so activating it says so on the card and the cockpit
    // stays up.
    let mut m = launcher_model();
    assert_eq!(update(&mut m, session_click(1)), vec![]);
    assert!(!m.quit_requested(), "an unresumable row never quits");
    assert_eq!(m.launch(), None);
    assert_eq!(
        m.session_note(),
        Some("Aug 27 19:41 is no longer resumable")
    );

    // Moving the selection clears the note.
    update(&mut m, key(KeyCode::Up));
    assert_eq!(m.session_note(), None);

    // And a row that does resume clears it too.
    let mut m = launcher_model();
    update(&mut m, session_click(1));
    assert!(m.session_note().is_some());
    update(&mut m, session_click(0));
    assert_eq!(m.session_note(), None);
    assert_eq!(m.launch(), Some(&LaunchAction::Resume("sess-1".to_owned())));
}

#[test]
fn the_arrows_stay_the_prompts_while_the_card_lists_no_sessions() {
    // With no launcher data at all the native launcher's arrows are the
    // prompt's history, exactly as before.
    let mut m = model().with_tab(StageTab::Roadmap);
    update(&mut m, key(KeyCode::Down));
    assert_eq!(m.session_selected(), None);
    assert_eq!(update(&mut m, key(KeyCode::Enter)), vec![]);
    assert_eq!(m.launch(), None);

    // An empty sessions card is the same.
    let mut m = launcher_model_with(Vec::new());
    update(&mut m, key(KeyCode::Down));
    assert_eq!(m.session_selected(), None);
}

/// A Home-tab model whose launcher job already delivered two rows: one
/// resumable, one the entry point could give no id.
fn launcher_model() -> Model {
    launcher_model_with(vec![
        crate::cockpit::LauncherSession {
            id: Some("sess-1".to_owned()),
            when: "Aug 28 14:02".to_owned(),
            duration: "2m 30s".to_owned(),
            outcome: crate::cockpit::SessionOutcome::Success,
            preview: "shipped the launcher".to_owned(),
        },
        crate::cockpit::LauncherSession {
            id: None,
            when: "Aug 27 19:41".to_owned(),
            duration: "3m 05s".to_owned(),
            outcome: crate::cockpit::SessionOutcome::Cancelled,
            preview: "cancelled mid-turn".to_owned(),
        },
    ])
}

/// The same, with the rows named.
fn launcher_model_with(sessions: Vec<crate::cockpit::LauncherSession>) -> Model {
    let mut m = model()
        .with_surface(Surface::Native)
        .with_tab(StageTab::Roadmap);
    update(
        &mut m,
        Msg::Job {
            id: JobId::new(0),
            output: JobOutput::Launcher(Box::new(crate::cockpit::LauncherData {
                project: "magistr".to_owned(),
                has_config: true,
                stack: "Rust / Cargo".to_owned(),
                git: "master, clean".to_owned(),
                backend: "ACP (available)".to_owned(),
                sessions,
            })),
        },
    );
    m
}

/// A left click landing on the launcher's session row at `index`.
fn session_click(index: usize) -> Msg {
    Msg::Mouse {
        event: MouseEvent::new(MouseKind::Down(Button::Left), 10, 10),
        target: Some(Widget::launch_session(index)),
    }
}

#[test]
fn the_sheet_opens_on_question_mark_and_closes_on_esc() {
    let mut m = model();
    assert!(!m.is_sheet_open());
    update(&mut m, key(KeyCode::Char('?')));
    assert!(m.is_sheet_open(), "? on an empty prompt opens the sheet");
    update(&mut m, key(KeyCode::Char('x')));
    assert!(m.is_sheet_open(), "other keys are swallowed while open");
    assert_eq!(m.prompt().text(), "", "the swallowed key did not type");
    update(&mut m, key(KeyCode::Esc));
    assert!(!m.is_sheet_open(), "esc closes it");

    // With text in the prompt, ? types.
    let mut m = model();
    update(&mut m, key(KeyCode::Char('a')));
    update(&mut m, key(KeyCode::Char('?')));
    assert!(!m.is_sheet_open());
    assert_eq!(m.prompt().text(), "a?");

    // ctrl-d still quits while the sheet is open.
    let mut m = model();
    update(&mut m, key(KeyCode::Char('?')));
    let ctrl_d = Msg::Key(KeyEvent::new(KeyCode::Char('d')).with(Modifiers::CTRL));
    assert_eq!(update(&mut m, ctrl_d), vec![Cmd::Quit]);
}

#[test]
fn next_ready_follows_the_engines_pick() {
    let mut m = model();
    update(
        &mut m,
        Msg::Engine(EngineEvent::Seed(
            magistr_workflow_contracts::NativeSnapshot {
                progress: crate::harness::fixtures::progress(1, 3),
                chunks: crate::harness::fixtures::chunks_from(vec![
                    serde_json::json!({"id": "base", "status": "done", "priority": 100}),
                    serde_json::json!({"id": "gated", "status": "pending", "priority": 100, "deps": ["open"]}),
                    serde_json::json!({"id": "open", "status": "pending", "priority": 300, "deps": ["base"]}),
                ]),
                ..magistr_workflow_contracts::NativeSnapshot::default()
            },
        )),
    );
    let next = m.roadmap().next_ready().expect("one chunk is ready");
    assert_eq!(
        next.id, "open",
        "the dep-gated chunk is not ready even at a better priority"
    );
    assert_eq!(m.roadmap().done_count(), 1);
    assert_eq!(m.roadmap().active_count(), 3);
}

/// A Roadmap tab holding `n` chunks in one dependency chain — more rows
/// than any stage window fits.
/// The Trace tab showing `n` think steps — enough that its body runs
/// past the stage under the pinned heading.
fn long_trace_tab(n: u32) -> Model {
    let mut m = model().with_tab(StageTab::Session);
    for number in 1..=n {
        update(
            &mut m,
            frame(
                "think",
                "step_appended",
                serde_json::json!({"step": {
                    "step_number": number,
                    "purpose": format!("step {number}"),
                }}),
            ),
        );
    }
    m
}

fn long_roadmap(n: usize) -> Model {
    let mut m = model().with_tab(StageTab::Roadmap);
    let chunks: Vec<serde_json::Value> = (0..n)
        .map(|i| {
            let deps: Vec<String> = if i == 0 {
                Vec::new()
            } else {
                vec![format!("c{}", i - 1)]
            };
            serde_json::json!({
                "id": format!("c{i}"), "name": format!("c{i}"),
                "title": format!("chunk {i}"), "status": "pending",
                "priority": 100, "group": "Core", "deps": deps,
            })
        })
        .collect();
    update(
        &mut m,
        Msg::Engine(EngineEvent::Seed(
            magistr_workflow_contracts::NativeSnapshot {
                chunks: crate::harness::fixtures::chunks_from(chunks),
                ..magistr_workflow_contracts::NativeSnapshot::default()
            },
        )),
    );
    m
}

#[test]
fn the_roadmap_list_scrolls_with_the_wheel_and_the_page_keys() {
    let mut m = long_roadmap(110);
    let (total, height) = m.roadmap_window();
    assert!(
        total > height,
        "110 chunks run past the stage: {total} > {height}"
    );
    assert_eq!(m.roadmap_top(), 0, "the list starts at its working set");

    let wheel = |kind| Msg::Mouse {
        event: MouseEvent::new(kind, 5, 20),
        target: Some(Widget::Stage.id()),
    };
    update(&mut m, wheel(MouseKind::ScrollDown));
    settle(&mut m);
    assert_eq!(m.roadmap_top(), 3, "one notch");
    update(&mut m, wheel(MouseKind::ScrollUp));
    settle(&mut m);
    assert_eq!(m.roadmap_top(), 0);

    update(&mut m, key(KeyCode::End));
    settle(&mut m);
    assert_eq!(m.roadmap_top(), total - height, "the far end is reachable");
    update(&mut m, key(KeyCode::Home));
    settle(&mut m);
    assert_eq!(m.roadmap_top(), 0);
    update(&mut m, key(KeyCode::PageDown));
    settle(&mut m);
    assert_eq!(m.roadmap_top(), height - 1);

    // The turn's own window never moved while the tab scrolled.
    assert!(m.viewport().is_following());
}

#[test]
fn the_last_chunks_detail_card_is_reachable_on_a_long_roadmap() {
    let mut m = long_roadmap(110);
    let last = m.roadmap().rows()[109].id.clone();
    update(&mut m, left_click(Widget::roadmap_row(109)));
    assert_eq!(
        m.roadmap().selected().map(|c| c.id.as_str()),
        Some(last.as_str())
    );
    let (total, height) = m.roadmap_window();
    update(&mut m, key(KeyCode::End));
    settle(&mut m);
    assert_eq!(m.roadmap_top(), total - height);
    // The window ends past the last row, so the open card is on screen.
    assert_eq!(m.roadmap_top() + height, total);
}

#[test]
fn a_wheel_notch_over_the_tech_tree_zooms_instead_of_scrolling() {
    let mut m = long_roadmap(110);
    m.set_roadmap_graph_rect(Some(crate::layout::Rect {
        x: 30,
        y: 6,
        width: 40,
        height: 10,
    }));
    let over_graph = |kind| Msg::Mouse {
        event: MouseEvent::new(kind, 40, 10),
        target: Some(Widget::RoadmapGraph.id()),
    };
    let before = m.roadmap().zoom();
    update(&mut m, over_graph(MouseKind::ScrollUp));
    assert!(m.roadmap().zoom() > before, "the notch zoomed in");
    assert_eq!(m.roadmap_top(), 0, "and the list did not move");
    update(&mut m, over_graph(MouseKind::ScrollDown));
    assert!((m.roadmap().zoom() - before).abs() < 1e-5, "back out again");

    // A notch beside the block scrolls the list as everywhere else.
    let beside = Msg::Mouse {
        event: MouseEvent::new(MouseKind::ScrollDown, 5, 20),
        target: Some(Widget::Stage.id()),
    };
    update(&mut m, beside);
    settle(&mut m);
    assert_eq!(m.roadmap_top(), 3);
    assert!(
        (m.roadmap().zoom() - before).abs() < 1e-5,
        "and did not zoom"
    );
}

/// A window's report, at the pixel a 10x20 cell puts `(col, row)` at
/// plus an offset inside the cell.
fn windowed(kind: MouseKind, col: u16, row: u16, dx: u32, dy: u32) -> MouseEvent {
    MouseEvent::new(kind, col, row).at_px(u32::from(col) * 10 + dx, u32::from(row) * 20 + dy)
}

/// A model on a window whose cell is 10x20.
fn on_a_window(mut model: Model) -> Model {
    model.set_surface(crate::cockpit::Surface::Native);
    update(
        &mut model,
        Msg::CellSize(Some(crate::pixel::CellSize {
            width: 10,
            height: 20,
        })),
    );
    model
}

#[test]
fn a_terminal_reads_the_pointer_at_the_cells_centre_and_a_window_at_its_pixel() {
    let terminal = model();
    let plain = MouseEvent::new(MouseKind::Move, 4, 3);
    assert_eq!(terminal.pointer_at(&plain), (4.5, 3.5));

    let window = on_a_window(model());
    // Halfway down the row, at the column's second pixel.
    let exact = window.pointer_at(&windowed(MouseKind::Move, 4, 3, 1, 10));
    assert!((exact.0 - 4.1).abs() < 1e-6, "{exact:?}");
    assert!((exact.1 - 3.5).abs() < 1e-6, "{exact:?}");
    // A report with no pixel reads as a terminal's even on a window.
    assert_eq!(window.pointer_at(&plain), (4.5, 3.5));
}

/// The tech-tree zoomed in past its fit, so the pan has room to move.
fn zoomed_tree(mut model: Model) -> Model {
    model.set_roadmap_graph_rect(Some(crate::layout::Rect {
        x: 30,
        y: 6,
        width: 40,
        height: 10,
    }));
    for _ in 0..4 {
        update(
            &mut model,
            Msg::Mouse {
                event: MouseEvent::new(MouseKind::ScrollUp, 40, 10),
                target: Some(Widget::RoadmapGraph.id()),
            },
        );
    }
    model
}

#[test]
fn on_a_window_the_tech_tree_pans_by_the_exact_pixel_delta() {
    let mut window = zoomed_tree(on_a_window(long_roadmap(110)));
    let before = window.roadmap().pan();
    // Press at the column's middle, drag to its left edge: the same
    // cell twice, half a column apart in pixels.
    update(
        &mut window,
        Msg::Mouse {
            event: windowed(MouseKind::Down(Button::Left), 40, 10, 5, 10),
            target: Some(Widget::RoadmapGraph.id()),
        },
    );
    update(
        &mut window,
        Msg::Mouse {
            event: windowed(MouseKind::Drag(Button::Left), 40, 10, 0, 10),
            target: Some(Widget::RoadmapGraph.id()),
        },
    );
    let after = window.roadmap().pan();
    assert!(
        ((after.0 - before.0).abs() - 0.5).abs() < 1e-5,
        "a drag inside one cell moved the pan by exactly half a cell on a window: {before:?} -> {after:?}"
    );

    // The same gesture on a terminal is the same cell twice: no move.
    let mut terminal = zoomed_tree(long_roadmap(110));
    let before = terminal.roadmap().pan();
    for kind in [MouseKind::Down(Button::Left), MouseKind::Drag(Button::Left)] {
        update(
            &mut terminal,
            Msg::Mouse {
                event: MouseEvent::new(kind, 40, 10),
                target: Some(Widget::RoadmapGraph.id()),
            },
        );
    }
    assert_eq!(terminal.roadmap().pan(), before);
}

#[test]
fn without_a_graph_rect_the_wheel_scrolls_the_list_everywhere() {
    let mut m = long_roadmap(110);
    assert!(m.roadmap_graph_rect().is_none(), "cell-only mode");
    update(
        &mut m,
        Msg::Mouse {
            event: MouseEvent::new(MouseKind::ScrollDown, 40, 10),
            target: Some(Widget::RoadmapGraph.id()),
        },
    );
    settle(&mut m);
    assert_eq!(m.roadmap_top(), 3);
    assert!((m.roadmap().zoom() - 1.0).abs() < f32::EPSILON);
}

/// A base64 PNG of `width × height`, as an image block carries it.
fn png_base64(width: u32, height: u32) -> String {
    let mut pixmap = tiny_skia::Pixmap::new(width, height).expect("a pixmap allocates");
    pixmap.fill(tiny_skia::Color::from_rgba8(200, 40, 40, 255));
    let png = pixmap.encode_png().expect("a pixmap encodes");
    let mut out = Vec::new();
    crate::base64::encode(&png, &mut out);
    String::from_utf8(out).expect("base64 is ascii")
}

fn image_event(data: String, mime_type: &str) -> Msg {
    Msg::Agent(AgentEvent::Image(ImageBlock {
        mime_type: mime_type.to_owned(),
        data,
        uri: None,
    }))
}

#[test]
fn an_image_block_becomes_image_rows_in_the_turn() {
    let mut m = model();
    // 40×60 px at the default 10×20 cell is 4 columns by 3 rows.
    update(&mut m, image_event(png_base64(40, 60), "image/png"));
    assert_eq!(m.turn().len(), 3);
    assert!(
        m.turn().rows().iter().all(|r| r.kind == RowKind::Image),
        "every row of the block is an image row"
    );
    let (image, offset, size) = m.turn().image_at_row(0).expect("the image is in the turn");
    assert_eq!(image.alt(), "[image image/png]");
    assert_eq!((offset, size), (0, (4, 3)));
    assert_eq!(m.turn_state(), TurnState::Streaming);
}

/// The same 40x60 image a real JPEG encoder wrote — see
/// `fixtures/README.md`.
fn jpeg_base64() -> String {
    let mut out = Vec::new();
    crate::base64::encode(include_bytes!("../../fixtures/half.jpg"), &mut out);
    String::from_utf8(out).expect("base64 is ascii")
}

#[test]
fn a_jpeg_block_lands_on_the_same_rows_a_png_block_does() {
    let mut m = model();
    update(&mut m, image_event(jpeg_base64(), "image/jpeg"));
    assert_eq!(m.turn().len(), 3, "40x60 at the default cell is three rows");
    assert!(m.turn().rows().iter().all(|r| r.kind == RowKind::Image));
    let (image, offset, size) = m.turn().image_at_row(0).expect("the image is in the turn");
    assert_eq!(image.alt(), "[image image/jpeg]");
    assert_eq!((offset, size), (0, (4, 3)));
}

#[test]
fn a_block_is_decoded_by_its_bytes_not_by_the_mime_type_it_claims() {
    let mut m = model();
    // A JPEG an agent labelled a PNG still shows; the label only names it.
    update(&mut m, image_event(jpeg_base64(), "image/png"));
    assert_eq!(m.turn().len(), 3);
    let (image, _, _) = m.turn().image_at_row(0).expect("the image is in the turn");
    assert_eq!(image.alt(), "[image image/png]");
}

#[test]
fn an_image_the_cockpit_cannot_read_falls_back_to_one_note_row() {
    let mut m = model();
    update(&mut m, image_event("bm90IGEgcG5n".to_owned(), "image/jpeg"));
    assert_eq!(m.turn().len(), 1);
    assert_eq!(m.turn().rows()[0].kind, RowKind::Note);
    assert_eq!(
        m.turn().rows()[0].text,
        "[image image/jpeg] could not be shown"
    );
    // Base64 that is not base64 lands the same way, never silently.
    update(&mut m, image_event("!!!!".to_owned(), "image/png"));
    assert_eq!(m.turn().len(), 2);
    assert_eq!(m.turn().rows()[1].kind, RowKind::Note);
}

#[test]
fn an_images_uri_labels_it() {
    let mut m = model();
    update(
        &mut m,
        Msg::Agent(AgentEvent::Image(ImageBlock {
            mime_type: "image/png".to_owned(),
            data: png_base64(20, 20),
            uri: Some("file:///tmp/shot.png".to_owned()),
        })),
    );
    let (image, _, _) = m.turn().image_at_row(0).expect("the image is in the turn");
    assert_eq!(image.alt(), "[image image/png: file:///tmp/shot.png]");
}

#[test]
fn an_image_a_tool_call_returned_lands_after_its_card_and_survives_its_updates() {
    use magistr_application::sessions::{ToolStatus, ToolVerb};
    let mut m = model();
    let shot = ImageBlock {
        mime_type: "image/png".to_owned(),
        // 40×60 px at the default 10×20 cell is 4 columns by 3 rows.
        data: png_base64(40, 60),
        uri: None,
    };
    update(
        &mut m,
        Msg::Agent(AgentEvent::ToolCall {
            id: "tc-1".to_owned(),
            title: "browser screenshot".to_owned(),
            verb: ToolVerb::Tool,
            status: ToolStatus::InProgress,
            diff: None,
            images: vec![shot.clone()],
            record: Default::default(),
        }),
    );
    assert_eq!(m.turn().len(), 4, "the card's row and three image rows");
    let (image, offset, size) = m.turn().image_at_row(1).expect("the image is in the turn");
    assert_eq!(image.alt(), "[image image/png]");
    assert_eq!((offset, size), (0, (4, 3)));

    // The completion update re-sends the block, as ACP's replace
    // semantics let it: the card's row count does not move and no
    // second copy appears.
    update(
        &mut m,
        Msg::Agent(AgentEvent::ToolCallUpdate {
            id: "tc-1".to_owned(),
            title: None,
            status: Some(ToolStatus::Completed),
            diff: None,
            duration_ms: Some(120),
            images: vec![shot],
            record: Default::default(),
        }),
    );
    assert_eq!(m.turn().len(), 4, "still one card and one image");
    assert_eq!(
        m.turn().rows()[0].kind,
        RowKind::Tool(ToolStatus::Completed)
    );

    // A block the cockpit cannot read is noted, once, not dropped.
    update(
        &mut m,
        Msg::Agent(AgentEvent::ToolCallUpdate {
            id: "tc-1".to_owned(),
            title: None,
            status: Some(ToolStatus::Completed),
            diff: None,
            duration_ms: None,
            images: vec![
                ImageBlock {
                    mime_type: "image/png".to_owned(),
                    data: png_base64(40, 60),
                    uri: None,
                },
                ImageBlock {
                    mime_type: "image/jpeg".to_owned(),
                    data: "bm90IGEgcG5n".to_owned(),
                    uri: None,
                },
            ],
            record: Default::default(),
        }),
    );
    assert_eq!(m.turn().len(), 5, "the note is the call's second block");
    assert_eq!(m.turn().rows()[4].kind, RowKind::Note);
    assert_eq!(
        m.turn().rows()[4].text,
        "[image image/jpeg] could not be shown"
    );
}

#[test]
fn a_tool_calls_record_rides_the_events_onto_its_line_without_a_row() {
    use magistr_application::sessions::{ToolRecord, ToolStatus, ToolVerb};
    let mut m = model();
    update(
        &mut m,
        Msg::Agent(AgentEvent::ToolCall {
            id: "tc-7".to_owned(),
            title: "cargo test".to_owned(),
            verb: ToolVerb::Run,
            status: ToolStatus::InProgress,
            diff: None,
            images: Vec::new(),
            record: Box::new(ToolRecord {
                kind: Some(ToolVerb::Run),
                input: Some(magistr_application::sessions::ToolPayload::Json(
                    r#"{"command":"cargo test"}"#.into(),
                )),
                ..ToolRecord::default()
            }),
        }),
    );
    assert_eq!(m.turn().len(), 1, "the card's row and nothing else");
    update(
        &mut m,
        Msg::Agent(AgentEvent::ToolCallUpdate {
            id: "tc-7".to_owned(),
            title: None,
            status: Some(ToolStatus::Completed),
            diff: None,
            duration_ms: Some(40),
            images: Vec::new(),
            record: Box::new(ToolRecord {
                output: Some(magistr_application::sessions::ToolPayload::Text(
                    "test result: ok".into(),
                )),
                text: vec!["test result: ok".to_owned()],
                ..ToolRecord::default()
            }),
        }),
    );
    assert_eq!(m.turn().len(), 1, "still one row");
    let record = m.turn().tool_record("tc-7").expect("kept on the line");
    assert_eq!(record.kind, Some(ToolVerb::Run));
    assert_eq!(
        record.input,
        Some(magistr_application::sessions::ToolPayload::Json(
            r#"{"command":"cargo test"}"#.into()
        ))
    );
    assert_eq!(
        record.output,
        Some(magistr_application::sessions::ToolPayload::Text(
            "test result: ok".into()
        ))
    );
    assert_eq!(record.text, ["test result: ok".to_owned()]);
}

#[test]
fn a_late_cell_size_answer_re_measures_the_images_in_the_turn() {
    let mut m = model();
    update(&mut m, image_event(png_base64(40, 60), "image/png"));
    assert_eq!(m.turn().len(), 3, "measured against the 10x20 guess");
    update(
        &mut m,
        Msg::CellSize(Some(crate::pixel::CellSize {
            width: 20,
            height: 40,
        })),
    );
    assert_eq!(m.turn().cell_size(), crate::pixel::CellSize::new(20, 40));
    assert_eq!(m.turn().len(), 2, "bigger cells, fewer of them");
}

/// A diff-approval request for `path`, as the session posts one when a
/// policy holds an edit for review.
fn edit_request(id: u64, path: &str) -> AgentEvent {
    use magistr_application::sessions::{PermissionChoice, PermissionKind};
    AgentEvent::PermissionRequest {
        id,
        title: format!("Edit {path}"),
        choices: vec![
            PermissionChoice::new("allow", PermissionKind::AllowOnce),
            PermissionChoice::new("reject", PermissionKind::RejectOnce),
        ],
        edit: true,
        risk: RiskTier::Medium,
        plan: None,
    }
}

#[test]
fn approve_answers_the_docks_diff_approval_card_for_the_same_file() {
    let mut m = model();
    update(
        &mut m,
        Msg::Workbench(WorkbenchEvent::Journal {
            entries: vec![journal_entry("e1", "src/main.rs")],
        }),
    );
    update(&mut m, Msg::Agent(edit_request(7, "src/main.rs")));
    assert_eq!(m.dock().len(), 1, "the policy held the edit for review");
    assert_eq!(
        change_click(&mut m, Widget::change_approve(0)),
        vec![Cmd::Permission {
            id: 7,
            option_id: "allow".to_owned(),
        }],
        "reviewing the diff on the Changes tab answers the card"
    );
    assert!(
        m.changes()
            .is_approved(&journal_entry("e1", "src/main.rs").identity())
    );
    assert!(m.dock().is_empty(), "the card left the dock");
}

#[test]
fn approve_with_a_card_for_another_file_leaves_it_alone() {
    let mut m = model();
    update(
        &mut m,
        Msg::Workbench(WorkbenchEvent::Journal {
            entries: vec![journal_entry("e1", "src/main.rs")],
        }),
    );
    update(&mut m, Msg::Agent(edit_request(7, "src/other.rs")));
    assert_eq!(change_click(&mut m, Widget::change_approve(0)), vec![]);
    assert!(
        m.changes()
            .is_approved(&journal_entry("e1", "src/main.rs").identity()),
        "still the reviewed mark"
    );
    assert_eq!(m.dock().len(), 1, "the other file's card still waits");
}

#[test]
fn a_long_journal_scrolls_on_the_changes_tab() {
    let mut m = model();
    m.tab = StageTab::Session;
    let entries: Vec<crate::workbench::JournalEntry> = (0..200)
        .map(|n| journal_entry(&format!("e{n}"), &format!("src/f{n}.rs")))
        .collect();
    update(&mut m, Msg::Workbench(WorkbenchEvent::Journal { entries }));
    let (total, height) = m.session_window();
    assert_eq!(total, 200);
    assert!(height < total, "the list runs past the stage");
    assert_eq!(
        m.session_top(),
        total - height,
        "the session follows new activity"
    );
    update(&mut m, key(KeyCode::Home));
    settle(&mut m);
    assert_eq!(m.session_top(), 0);
    update(&mut m, key(KeyCode::PageDown));
    settle(&mut m);
    let paged = m.session_top();
    assert!(paged > 0, "PageDown moved the list, got {paged}");
    update(&mut m, key(KeyCode::End));
    settle(&mut m);
    assert_eq!(
        m.session_top(),
        total - height,
        "End lands on the last window"
    );
    update(&mut m, key(KeyCode::Home));
    settle(&mut m);
    assert_eq!(m.session_top(), 0);
}

#[test]
fn the_wheel_over_the_changes_tab_moves_its_own_list_not_the_turn() {
    let mut m = model();
    m.tab = StageTab::Session;
    let entries: Vec<crate::workbench::JournalEntry> = (0..200)
        .map(|n| journal_entry(&format!("e{n}"), &format!("src/f{n}.rs")))
        .collect();
    update(&mut m, Msg::Workbench(WorkbenchEvent::Journal { entries }));
    let before = m.top_row();
    update(
        &mut m,
        Msg::Mouse {
            event: MouseEvent::new(MouseKind::ScrollDown, 10, 5),
            target: None,
        },
    );
    settle(&mut m);
    assert!(m.session_top() > 0, "the list moved");
    assert_eq!(m.top_row(), before, "the turn stayed where it was");
}

// ── The declared animation set ─────────

/// A running loop keeps the clocks moving on its own. While the motion
/// layer is settling the reconciled interval is the finer of the two;
/// once it settles the tick does not stop — it steps down to a second,
/// which is what keeps the header's `mm:ss` and the loop's elapsed from
/// going stale through a quiet minute.
#[test]
fn a_running_loop_steps_the_tick_down_to_a_second_instead_of_stopping() {
    use crate::cockpit::Animation;
    let mut m = model();
    assert!(
        m.animations().is_empty(),
        "an idle cockpit declares nothing"
    );
    let started = update(
        &mut m,
        loop_msg(LoopEvent::Started {
            iteration: 1,
            max: 3,
        }),
    );
    assert_eq!(
        started,
        vec![Cmd::Animate(Animation::Clock.period())],
        "the start clears the stage and moves nothing but the clock"
    );
    assert!(m.animates(Animation::Clock), "the loop's clock is live");
    assert!(
        !m.animates(crate::cockpit::Animation::Motion),
        "nothing reads ticks yet"
    );
    assert_eq!(m.animations().period(), Some(Animation::Clock.period()));

    // A tick at a second costs nothing more; the declaration holds.
    assert_eq!(update(&mut m, Msg::Tick), vec![], "already declared");

    // A note on the stage arrives, which does read ticks: the interval
    // steps up to the motion period and back down when it has settled.
    let noted = update(&mut m, loop_msg(LoopEvent::Note("gate ok".to_owned())));
    assert_eq!(noted, vec![Cmd::Animate(Model::TICK_PERIOD)]);
    let mut stepped: Vec<Cmd> = Vec::new();
    for _ in 0..Eased::STEPS {
        stepped.extend(update(&mut m, Msg::Tick));
    }
    assert_eq!(
        stepped,
        vec![Cmd::Animate(Animation::Clock.period())],
        "the arrival settles and the interval steps back down to the clock's"
    );
    assert!(
        !m.animates(crate::cockpit::Animation::Motion),
        "nothing reads ticks now"
    );
    assert!(m.animates(Animation::Clock));

    // The loop ending is what finally takes the tick back.
    let ended = update(&mut m, loop_msg(LoopEvent::Ended("done".to_owned())));
    assert!(
        ended.contains(&Cmd::Animate(Model::TICK_PERIOD)),
        "the ending line arrives first: {ended:?}"
    );
    settle(&mut m);
    assert!(m.animations().is_empty(), "the loop ended: nothing moves");
    assert!(!m.is_animating());
}

/// A paused loop is still a running one as far as the clocks go — the
/// elapsed keeps counting through a pause, so it must keep being shown.
#[test]
fn a_paused_loop_still_declares_its_clock() {
    use crate::cockpit::Animation;
    let mut m = model();
    update(
        &mut m,
        loop_msg(LoopEvent::Started {
            iteration: 1,
            max: 3,
        }),
    );
    update(
        &mut m,
        loop_msg(LoopEvent::Paused("gates failed".to_owned())),
    );
    settle(&mut m);
    assert!(m.animates(Animation::Clock));
    assert!(!m.animates(crate::cockpit::Animation::Motion));
}

/// A working *window* declares the motion period through the loop's
/// own gaps — connecting, judging — because it breathes there;
/// the same state on a terminal
/// declares only the clock, the measured quiet-loop contract.
#[test]
fn a_working_window_breathes_where_a_terminal_keeps_the_clock() {
    use crate::cockpit::{LoopStage, Surface};
    let mut terminal = model();
    let mut window = model().with_surface(Surface::Native);
    for m in [&mut terminal, &mut window] {
        update(
            m,
            loop_msg(LoopEvent::Started {
                iteration: 1,
                max: 3,
            }),
        );
        update(m, loop_msg(LoopEvent::Stage(LoopStage::Connecting)));
        settle(m);
        assert!(m.is_working());
    }
    assert!(
        !terminal.animates(crate::cockpit::Animation::Motion),
        "a terminal's quiet loop"
    );
    assert!(
        window.animates(crate::cockpit::Animation::Motion),
        "the window breathes"
    );
    update(
        &mut window,
        loop_msg(LoopEvent::Paused("gates failed".to_owned())),
    );
    settle(&mut window);
    assert!(!window.is_working(), "a pause is not work");
    assert!(!window.animates(crate::cockpit::Animation::Motion));
}

/// A streaming turn declares both: the spinner and shimmer want the
/// motion period, the session clock wants its second. The set is
/// reconciled to the finer of the two, so the spinner is never slowed
/// by the clock sharing the tick with it.
#[test]
fn a_streaming_turn_declares_both_and_reconciles_to_the_finer() {
    use crate::cockpit::Animation;
    let mut m = model();
    type_text(&mut m, "go");
    update(&mut m, key(KeyCode::Enter));
    assert_eq!(m.turn_state(), TurnState::Streaming);
    assert!(m.animates(crate::cockpit::Animation::Motion));
    assert!(m.animates(Animation::Clock));
    assert_eq!(m.animations().period(), Some(Model::TICK_PERIOD));
}

/// Reduced motion freezes the clocks' tick too: the set is empty however
/// much is live, so the runtime never starts an interval and every
/// surface — clocks included — draws only when other news forces a
/// frame.
#[test]
fn reduced_motion_declares_nothing_even_while_a_loop_runs() {
    use crate::cockpit::Animation;
    let mut m = model().with_theme(Theme::default().with_motion(MotionMode::Reduced));
    assert_eq!(
        update(
            &mut m,
            loop_msg(LoopEvent::Started {
                iteration: 1,
                max: 3,
            }),
        ),
        vec![],
        "no tick is asked for"
    );
    assert!(m.has_running_loop(), "the loop is live all the same");
    assert!(m.animations().is_empty());
    assert!(!m.animates(Animation::Clock));
    assert_eq!(m.animations().period(), None);
}

// ---------------------------------------------------------------------
// the tech-tree's keyboard zoom. The gate requires a camera: a
// cell-only tab has no camera, so there the keys stay the prompt's.
// ---------------------------------------------------------------------

/// A Roadmap tab with the graph rect the driver reports in graphics
/// mode: a camera to zoom, and an empty prompt to zoom it from.
fn zoomable_roadmap() -> Model {
    let mut m = roadmap_model();
    m.set_roadmap_graph_rect(Some(crate::layout::Rect::new(2, 4, 60, 12)));
    m
}

#[test]
fn plus_and_minus_zoom_the_tech_tree_from_the_keyboard() {
    let mut m = zoomable_roadmap();
    update(&mut m, key(KeyCode::Char('+')));
    assert!(m.roadmap().zoom() > 1.0, "the tree zoomed in");
    assert!(
        m.session.prompt.is_empty(),
        "and the key never reached the prompt"
    );
    let zoomed = m.roadmap().zoom();
    update(&mut m, key(KeyCode::Char('-')));
    assert!(m.roadmap().zoom() < zoomed, "and back out");
    assert!(m.session.prompt.is_empty());
}

#[test]
fn an_unshifted_equals_zooms_in_too() {
    // `+` is shift-`=` on every keyboard the cockpit runs on; a reader
    // who does not reach for shift means the same thing.
    let mut m = zoomable_roadmap();
    update(&mut m, key(KeyCode::Char('=')));
    assert!(m.roadmap().zoom() > 1.0);
    assert!(m.session.prompt.is_empty());
}

#[test]
fn zero_puts_the_tech_tree_back_to_the_fit() {
    let mut m = zoomable_roadmap();
    update(&mut m, key(KeyCode::Char('+')));
    update(&mut m, key(KeyCode::Char('+')));
    assert!(m.roadmap().zoom() > 1.0);
    update(&mut m, key(KeyCode::Char('0')));
    assert!((m.roadmap().zoom() - 1.0).abs() < 1e-5, "back at the fit");
    assert_eq!(m.roadmap().pan(), (0.0, 0.0));
    assert!(m.session.prompt.is_empty());
}

#[test]
fn the_zoom_keys_declare_the_motion_that_carries_them() {
    let mut m = zoomable_roadmap();
    let cmds = update(&mut m, key(KeyCode::Char('+')));
    assert!(
        cmds.iter().any(|cmd| matches!(cmd, Cmd::Animate(_))),
        "a glide with no tick would freeze half way: {cmds:?}"
    );
}

#[test]
fn a_zoom_key_on_a_cell_only_tab_is_the_prompts_letter() {
    // No graph rect means no camera, and a key that changes
    // nothing must not be swallowed.
    let mut m = roadmap_model();
    assert!(m.roadmap_graph_rect().is_none(), "cell-only mode");
    update(&mut m, key(KeyCode::Char('-')));
    update(&mut m, key(KeyCode::Char('0')));
    assert_eq!(m.session.prompt.text(), "-0");
    assert!((m.roadmap().zoom() - 1.0).abs() < f32::EPSILON);
}

#[test]
fn a_zoom_key_with_text_in_the_prompt_stays_the_prompts() {
    let mut m = zoomable_roadmap();
    update(&mut m, key(KeyCode::Char('x')));
    update(&mut m, key(KeyCode::Char('-')));
    update(&mut m, key(KeyCode::Char('0')));
    assert_eq!(m.session.prompt.text(), "x-0");
    assert!((m.roadmap().zoom() - 1.0).abs() < f32::EPSILON);
}

#[test]
fn the_zoom_keys_belong_to_the_roadmap_tab_alone() {
    let mut m = model();
    update(&mut m, key(KeyCode::Char('+')));
    assert_eq!(m.session.prompt.text(), "+", "the turn tab types it");
}

/// A cockpit wide enough to undock a third column, carrying a think
/// trace longer than that column and a turn longer than the stage —
/// with the runtime's report of where the column landed,
/// the geometry `snap_undock_placement_180x50`
/// pins.
fn undocked(steps: u32, lines: usize) -> Model {
    let mut m = Model::new(Size::new(180, 50)).with_tab(StageTab::Turn);
    let steps = (1..=steps)
        .map(|n| {
            crate::harness::fixtures::step_from(
                serde_json::json!({"step_number": n, "purpose": "open"}),
            )
        })
        .collect();
    update(
        &mut m,
        Msg::Engine(EngineEvent::Seed(
            magistr_workflow_contracts::NativeSnapshot {
                steps,
                ..magistr_workflow_contracts::NativeSnapshot::default()
            },
        )),
    );
    for n in 0..lines {
        update(&mut m, Msg::Agent(AgentEvent::Text(format!("line {n}\n"))));
    }
    m.set_undock_rect(Some(crate::layout::Rect::new(132, 2, 48, 45)));
    m
}

/// A notch at `(col, row)` with no widget target — the coordinates alone
/// decide which column it moves.
fn notch(kind: MouseKind, col: u16, row: u16) -> Msg {
    Msg::Mouse {
        event: MouseEvent::new(kind, col, row),
        target: None,
    }
}

fn recorded_workspace() -> Model {
    let mut model = undocked(60, 200);
    model.pixels = true;
    model.sync_flight_wrap();
    model.show(StageTab::Session);
    model
}
fn area_notch(kind: MouseKind, widget: Widget) -> Msg {
    Msg::Mouse {
        event: MouseEvent::new(kind, 0, 0),
        target: Some(widget.id()),
    }
}
#[test]
fn recorded_work_and_conversation_scroll_independently_without_an_extra_column() {
    let mut m = recorded_workspace();
    assert_eq!(m.undocked_tab(), None);
    let (total, height) = m.session_window();
    assert!(total > height);
    let turn_before = m.conversation_top();
    let tail = m.session_top();
    update(
        &mut m,
        area_notch(MouseKind::ScrollUp, Widget::InspectorFields),
    );
    settle(&mut m);
    assert_eq!(m.session_top(), tail - Viewport::WHEEL);
    assert_eq!(m.conversation_top(), turn_before);
    let feed = m.session_top();
    update(&mut m, area_notch(MouseKind::ScrollUp, Widget::Stage));
    settle(&mut m);
    assert_eq!(
        m.conversation_top(),
        turn_before - m.conversation_wheel() as f32
    );
    assert_eq!(m.session_top(), feed);
}
#[test]
fn the_recorded_work_list_reaches_its_head_and_tail() {
    let mut m = recorded_workspace();
    let (total, height) = m.session_window();
    let conversation = m.top_row();
    for _ in 0..total.div_ceil(Viewport::WHEEL) {
        update(
            &mut m,
            area_notch(MouseKind::ScrollUp, Widget::InspectorFields),
        );
    }
    settle(&mut m);
    assert_eq!(m.session_top(), 0);
    for _ in 0..total.div_ceil(Viewport::WHEEL) {
        update(
            &mut m,
            area_notch(MouseKind::ScrollDown, Widget::InspectorFields),
        );
    }
    settle(&mut m);
    assert_eq!(m.session_top(), total - height);
    assert!(m.session_viewport.is_following());
    assert_eq!(m.top_row(), conversation);
}
#[test]
fn records_use_their_own_extent_and_a_stale_column_rect_is_ignored() {
    let mut m = recorded_workspace();
    let over = MouseEvent::new(MouseKind::ScrollUp, 150, 20);
    assert_eq!(wheeled_undock(&m, &over), None);
    assert_eq!(scroll_extent(&m), m.session_window());
    m.show(StageTab::Turn);
    assert_eq!(scroll_extent(&m), m.conversation_window());
    assert_eq!(m.undocked_tab(), None);
}

#[test]
fn without_an_undock_rect_the_wheel_moves_the_stage_everywhere() {
    let mut m = undocked(60, 200);
    m.set_undock_rect(None);
    let turn = m.top_row();
    let feed = m.session_top();
    update(&mut m, notch(MouseKind::ScrollUp, 150, 20));
    settle(&mut m);
    assert_eq!(m.top_row(), turn - Viewport::WHEEL, "the turn took it");
    assert_eq!(m.session_top(), feed, "the feed is untouched");
    // And a screen too narrow to undock anything never answers, however
    // stale a rect it is holding.
    let mut narrow = Model::new(Size::new(120, 40));
    narrow.set_undock_rect(Some(crate::layout::Rect::new(132, 2, 48, 45)));
    assert_eq!(narrow.undocked_tab(), None);
    assert_eq!(
        wheeled_undock(&narrow, &MouseEvent::new(MouseKind::ScrollUp, 150, 20)),
        None
    );
}

/// Seed one roadmap chunk that names `reference` in its cross_refs.
fn chunk_frame(id: &str, reference: &str) -> Msg {
    frame(
        "roadmap",
        "chunk_changed",
        serde_json::json!({"chunk": {
            "id": id, "name": id, "title": "a chunk", "status": "pending",
            "priority": 100, "group": "TUI rewrite", "cross_refs": [reference],
        }}),
    )
}

#[test]
fn a_chunk_link_click_turns_the_stage_to_the_roadmap_tab_with_the_chunk_selected() {
    let mut m = model();
    update(&mut m, chunk_frame("tui2-5b", "task:trace-jump"));
    update(
        &mut m,
        think_step_frame(7, serde_json::json!({"execution_ref": "task:trace-jump"})),
    );
    m.tab = StageTab::Session;
    trace_click(&mut m, Widget::trace_row(0));
    assert!(m.trace().is_expanded(7));

    assert_eq!(trace_click(&mut m, Widget::trace_chunk(7)), vec![]);
    assert_eq!(m.tab(), StageTab::Roadmap, "the stage turned");
    assert_eq!(
        m.roadmap().selected().map(|c| c.id.as_str()),
        Some("tui2-5b"),
        "the chunk it named is the one open"
    );
}

#[test]
fn a_chunk_link_click_on_a_ref_no_chunk_answers_moves_nothing() {
    let mut m = model();
    update(
        &mut m,
        think_step_frame(7, serde_json::json!({"execution_ref": "task:nobody"})),
    );
    m.tab = StageTab::Session;
    trace_click(&mut m, Widget::trace_row(0));
    assert_eq!(trace_click(&mut m, Widget::trace_chunk(7)), vec![]);
    assert_eq!(m.tab(), StageTab::Session, "the stage stayed");
    assert!(m.roadmap().selected().is_none());
}

#[test]
fn a_verdict_lands_on_the_strip_and_the_stage_and_its_count_survives_the_next_iteration() {
    let mut m = model();
    update(
        &mut m,
        loop_msg(LoopEvent::Started {
            iteration: 1,
            max: 3,
        }),
    );
    update(
        &mut m,
        loop_msg(LoopEvent::Verdict {
            iteration: 1,
            ok: false,
            impossible: false,
            reason: "the timeout has no test".to_owned(),
        }),
    );
    let status = m.loop_status().expect("a loop");
    assert_eq!(status.verdicts(), 1);
    assert_eq!(
        status
            .verdict()
            .map(crate::cockpit::LoopVerdict::text)
            .as_deref(),
        Some("not yet — the timeout has no test")
    );
    assert_eq!(
        status
            .verdict_text(&crate::theme::Icons::UNICODE)
            .as_deref(),
        Some("✗ judge")
    );
    // The note wraps at the stage's width; its head is what to look for.
    assert!(
        m.turn().rows().iter().any(|r| r.kind == RowKind::Note
            && r.text
                .starts_with("judge, iteration 1: not yet — the timeout")),
        "the reason is on the stage: {:?}",
        m.turn().rows().iter().map(|r| &r.text).collect::<Vec<_>>()
    );

    // The next iteration keeps the count and the last verdict, then
    // records its own.
    update(
        &mut m,
        loop_msg(LoopEvent::Started {
            iteration: 2,
            max: 3,
        }),
    );
    assert_eq!(m.loop_status().map(LoopStatus::verdicts), Some(1));
    update(
        &mut m,
        loop_msg(LoopEvent::Verdict {
            iteration: 2,
            ok: true,
            impossible: false,
            reason: "all four hold".to_owned(),
        }),
    );
    let status = m.loop_status().expect("a loop");
    assert_eq!(status.verdicts(), 2);
    assert_eq!(
        status
            .verdict_text(&crate::theme::Icons::UNICODE)
            .as_deref(),
        Some("✓ judge")
    );

    // A verdict with no loop running has nowhere to land and is dropped.
    let mut idle = model();
    update(
        &mut idle,
        loop_msg(LoopEvent::Verdict {
            iteration: 1,
            ok: true,
            impossible: false,
            reason: String::new(),
        }),
    );
    assert!(idle.loop_status().is_none());
}

#[test]
fn a_phase_moves_where_the_curves_are_read_and_nothing_else() {
    //  `Msg::Phase` is a sampling parameter, not news: it moves
    // `Model::now` inside the tick and touches nothing the motion key
    // watches, so a window can fold one before every display frame
    // without a fold's worth of consequences.
    let mut model = model();
    update(&mut model, Msg::Tick);
    assert_eq!(model.now(), 1.0, "no phase folded: now() is the tick");
    let before = model.motion_key();
    let cmds = update(&mut model, Msg::Phase(0.25));
    assert!(cmds.is_empty(), "a phase declares nothing: {cmds:?}");
    assert_eq!(model.now(), 1.25);
    assert_eq!(model.ticks(), 1, "the tick is the tick");
    assert_eq!(model.motion_key(), before, "no news");
    // Clamped rather than trusted: a frame that ran long must not read
    // the curves past a tick that has not happened.
    update(&mut model, Msg::Phase(4.0));
    assert_eq!(model.now(), 2.0);
    update(&mut model, Msg::Phase(-1.0));
    assert_eq!(model.now(), 1.0);
}

#[test]
fn a_cockpit_that_is_never_phased_reads_its_curves_exactly_where_the_ticks_are() {
    // The terminal's byte-identity, structurally: it paints on the tick
    // and sends no phase, so `now()` is `ticks` for every step and every
    // curve is sampled where an integer clock sampled it.
    let mut model = model();
    for step in 0..24u64 {
        assert_eq!(model.now(), step as f32);
        update(&mut model, Msg::Tick);
    }
}

/// A pointer resting on a control, on a window.
fn dwelling(target: Widget) -> Model {
    let mut model = model();
    model.set_surface(crate::cockpit::Surface::Native);
    update(
        &mut model,
        Msg::Mouse {
            event: MouseEvent {
                kind: MouseKind::Move,
                col: 10,
                row: 3,
                mods: Modifiers::NONE,
                px: None,
            },
            target: Some(target.id()),
        },
    );
    model
}

#[test]
fn a_pointer_that_rests_on_a_control_earns_its_card_and_one_that_crosses_does_not() {
    //  The dwell is a transient like every other: the tick runs
    // for the steps it takes and stops, so a window with the pointer
    // parked on a chip costs nothing once the card is up.
    let mut model = dwelling(Widget::TabTurn);
    assert_eq!(model.tooltip(), None, "the pointer has only just arrived");
    assert!(model.is_animating(), "the dwell asked for the tick");
    for _ in 0..Motion::DWELL_STEPS {
        update(&mut model, Msg::Tick);
    }
    let card = model.tooltip().expect("the card");
    assert_eq!(card.text, "Turn");
    assert_eq!((card.col, card.row), (10, 3));
    assert!(
        !model.is_animating(),
        "the card is up, so the window goes quiet again"
    );

    // Structure is not a control: naming a panel would be describing the
    // scaffolding rather than anything you can do something to.
    let mut inert = dwelling(Widget::Stage);
    for _ in 0..Motion::DWELL_STEPS {
        update(&mut inert, Msg::Tick);
    }
    assert_eq!(inert.tooltip(), None);
}

#[test]
fn a_terminal_never_times_a_dwell_and_reduced_motion_has_no_card_at_all() {
    // Both structural rather than a caller remembering to ask: the
    // terminal's `MotionKey::hover` is always `None`, so no stamp is
    // ever taken and the idle guarantee is untouched; reduced motion has
    // no tick, and a timed reveal with no tick would either never come
    // or come instantly.
    let mut terminal = model();
    update(
        &mut terminal,
        Msg::Mouse {
            event: MouseEvent {
                kind: MouseKind::Move,
                col: 10,
                row: 3,
                mods: Modifiers::NONE,
                px: None,
            },
            target: Some(Widget::TabTurn.id()),
        },
    );
    assert!(!terminal.is_animating(), "no dwell to time");
    for _ in 0..Motion::DWELL_STEPS {
        update(&mut terminal, Msg::Tick);
    }
    assert_eq!(terminal.tooltip(), None);

    let mut reduced = dwelling(Widget::TabTurn);
    reduced.theme = Theme::default().with_motion(MotionMode::Reduced);
    for _ in 0..Motion::DWELL_STEPS {
        update(&mut reduced, Msg::Tick);
    }
    assert_eq!(reduced.tooltip(), None);
    assert!(!reduced.is_animating());
}

/// The image row cap is a per-surface number: a terminal keeps the
/// twenty rows one screenshot may take, and a window — which has the
/// room — lets a picture stand at most of its height, so a 720 px
/// screenshot is not squashed into 400 px there.
#[test]
fn the_terminal_caps_an_image_at_twenty_rows_and_a_window_at_most_of_its_height() {
    use crate::cockpit::Surface;
    use crate::turn::MAX_IMAGE_ROWS;
    // 100×2000 px at the 10×20 guess is 100 rows naturally.
    let tall = || image_event(png_base64(100, 2000), "image/png");

    let mut terminal = Model::new(Size::new(80, 100));
    assert_eq!(terminal.image_row_cap(), MAX_IMAGE_ROWS);
    update(&mut terminal, tall());
    assert_eq!(terminal.turn().len(), usize::from(MAX_IMAGE_ROWS));

    let mut window = Model::new(Size::new(80, 100)).with_surface(Surface::Native);
    assert_eq!(
        window.image_row_cap(),
        100 - Model::NATIVE_IMAGE_MARGIN_ROWS,
        "the window keeps a margin for the chrome and the card"
    );
    update(&mut window, tall());
    assert_eq!(
        window.turn().len(),
        usize::from(100 - Model::NATIVE_IMAGE_MARGIN_ROWS)
    );

    // A resize re-derives the cap and re-measures the picture.
    update(&mut window, Msg::Resize(Size::new(80, 40)));
    assert_eq!(window.image_row_cap(), 40 - Model::NATIVE_IMAGE_MARGIN_ROWS);
    assert_eq!(
        window.turn().len(),
        usize::from(40 - Model::NATIVE_IMAGE_MARGIN_ROWS)
    );
    // Never below the terminal's own allowance, whatever the window.
    update(&mut window, Msg::Resize(Size::new(80, 10)));
    assert_eq!(window.image_row_cap(), MAX_IMAGE_ROWS);

    // Declaring the surface on a model already holding a picture
    // re-measures it too — the renderer declares late, on its first frame.
    let mut late = Model::new(Size::new(80, 100));
    update(&mut late, tall());
    late.set_surface(Surface::Native);
    assert_eq!(
        late.turn().len(),
        usize::from(100 - Model::NATIVE_IMAGE_MARGIN_ROWS)
    );
}

// ── The checkpoint spine ────────────────────

fn risky(id: u64, title: &str) -> AgentEvent {
    use magistr_application::sessions::{PermissionChoice, PermissionKind};
    AgentEvent::PermissionRequest {
        id,
        title: title.to_owned(),
        choices: vec![
            PermissionChoice::new("allow", PermissionKind::AllowOnce),
            PermissionChoice::new("reject", PermissionKind::RejectOnce),
        ],
        edit: false,
        risk: RiskTier::High,
        plan: None,
    }
}

fn card_text(m: &Model) -> String {
    m.dock().front().expect("a card").text(m.theme().icons())
}

fn spine_rows(m: &Model) -> Vec<String> {
    m.rail()
        .spine()
        .entries()
        .iter()
        .map(crate::cockpit::Checkpoint::row)
        .collect()
}

#[test]
fn a_risky_permission_requests_a_checkpoint_and_keeps_its_identity_internal() {
    let mut m = model();
    let cmds = update(&mut m, Msg::Agent(risky(1, "rm -rf build")));

    assert_eq!(
        cmds,
        vec![
            Cmd::Announce("needs you: rm -rf build".to_owned()),
            Cmd::Checkpoint(CheckpointCmd::Save {
                id: "cp-1".to_owned(),
                reason: CheckpointReason::Risk,
                label: "rm -rf build".to_owned(),
            }),
            // The card is a row arriving, which fades in.
            Cmd::Animate(Model::TICK_PERIOD),
        ],
        "the announcement first, then the state to fall back to"
    );
    assert!(
        card_text(&m).ends_with("· checkpoint requested"),
        "the card names it: {}",
        card_text(&m)
    );
    assert_eq!(
        spine_rows(&m),
        ["cp-1 · saving"],
        "and the rail has the row the card is pointing at, before the \
         port has answered"
    );
}

#[test]
fn a_permission_below_high_risk_takes_no_checkpoint() {
    let mut m = model();
    let cmds = update(&mut m, Msg::Agent(request(1, "Write src/main.rs")));
    assert_eq!(
        cmds,
        vec![
            Cmd::Announce("needs you: Write src/main.rs".to_owned()),
            Cmd::Animate(Model::TICK_PERIOD),
        ],
        "a reversible change is not what the ledger is for"
    );
    assert!(m.rail().spine().is_empty());
    assert!(
        !card_text(&m).contains("cp-"),
        "and the card is the card it always was: {}",
        card_text(&m)
    );
}

#[test]
fn the_ports_answer_lands_on_the_row_the_card_named() {
    let mut m = model();
    update(&mut m, Msg::Agent(risky(1, "curl | sh")));
    update(
        &mut m,
        Msg::Checkpoint(CheckpointEvent::Saved {
            id: "cp-1".to_owned(),
            head: Some("0123456789ab".to_owned()),
            verified: true,
        }),
    );
    assert_eq!(spine_rows(&m), ["cp-1 · curl | sh"]);
    assert!(m.rail().spine().head().expect("a head").is_verified());
}

#[test]
fn a_save_that_failed_marks_its_row_rather_than_erasing_it() {
    let mut m = model();
    update(&mut m, Msg::Agent(risky(1, "curl | sh")));
    update(
        &mut m,
        Msg::Checkpoint(CheckpointEvent::Failed {
            id: Some("cp-1".to_owned()),
            what: "save cp-1".to_owned(),
            error: "not a git repository".to_owned(),
        }),
    );
    assert_eq!(
        spine_rows(&m),
        ["cp-1 · not saved"],
        "the card on screen still says cp-1"
    );
    assert!(
        stage(&m)
            .iter()
            .any(|row| row.contains("not a git repository")),
        "{:?}",
        stage(&m)
    );
}

#[test]
fn a_click_on_a_ledger_row_previews_it_and_the_same_click_folds_it_back() {
    let mut m = model();
    update(&mut m, Msg::Agent(risky(1, "a")));
    update(&mut m, Msg::Agent(risky(2, "b")));
    let click = |m: &mut Model, index: usize| {
        update(
            m,
            Msg::Mouse {
                event: MouseEvent::new(MouseKind::Down(Button::Left), 2, 6),
                target: Some(Widget::spine_row(index)),
            },
        )
    };
    assert_eq!(click(&mut m, 1), vec![], "preview asks the port nothing");
    assert_eq!(
        m.rail()
            .spine()
            .previewed()
            .map(crate::cockpit::Checkpoint::id),
        Some("cp-2")
    );
    click(&mut m, 1);
    assert!(m.rail().spine().previewed().is_none());
}

#[test]
fn the_ledger_palette_takes_a_checkpoint_restores_and_forks() {
    let mut m = model();
    let send = |m: &mut Model, text: &str| {
        type_chars(m, text);
        update(m, key(KeyCode::Enter))
    };

    // A manual checkpoint mints its own name, exactly as the risky
    // permission does.
    assert_eq!(
        send(&mut m, "/checkpoint-now before the refactor"),
        vec![
            Cmd::Checkpoint(CheckpointCmd::Save {
                id: "cp-1".to_owned(),
                reason: CheckpointReason::Manual,
                label: "before the refactor".to_owned(),
            }),
            // The echoed command is a row arriving, which fades in.
            Cmd::Animate(Model::TICK_PERIOD),
        ],
    );
    assert_eq!(m.rail().spine().len(), 1);

    assert_eq!(
        send(&mut m, "/restore cp-1"),
        vec![Cmd::Checkpoint(CheckpointCmd::Restore {
            id: "cp-1".to_owned()
        })],
    );
    assert_eq!(
        send(&mut m, "/fork cp-1"),
        vec![Cmd::Checkpoint(CheckpointCmd::Fork {
            id: "cp-1".to_owned(),
            branch: None,
        })],
        "no branch named means the port's default"
    );
    assert_eq!(
        send(&mut m, "/fork cp-1 try/other-way"),
        vec![Cmd::Checkpoint(CheckpointCmd::Fork {
            id: "cp-1".to_owned(),
            branch: Some("try/other-way".to_owned()),
        })],
    );
}

#[test]
fn a_ledger_verb_naming_a_state_the_spine_does_not_hold_goes_to_the_agent() {
    let mut m = model();
    let cmds = {
        type_chars(&mut m, "/restore cp-9");
        update(&mut m, key(KeyCode::Enter))
    };
    assert_eq!(
        cmds,
        vec![
            Cmd::Prompt("/restore cp-9".to_owned()),
            Cmd::Animate(Model::TICK_PERIOD),
        ],
        "the port is never asked about a state that was never on the rail"
    );
}

#[test]
fn a_restore_and_a_fork_land_on_the_stage() {
    let mut m = model();
    update(
        &mut m,
        Msg::Checkpoint(CheckpointEvent::Restored {
            id: "cp-2".to_owned(),
            iteration: 7,
            severed: vec!["the ACP agent process".to_owned()],
        }),
    );
    update(
        &mut m,
        Msg::Checkpoint(CheckpointEvent::Forked {
            id: "cp-2".to_owned(),
            branch: "try/other-way".to_owned(),
            path: "/work/project-try-other-way".to_owned(),
        }),
    );
    // The source lines, not the rows: an 80-column stage wraps both of
    // these, and where the wrap falls is not what the test is about.
    let lines: Vec<&str> = m.turn().lines().iter().map(|l| l.text()).collect();
    assert_eq!(
        lines,
        [
            "restored cp-2 · iteration 7 · left behind: the ACP agent process",
            "forked cp-2 · try/other-way at /work/project-try-other-way",
        ],
        "what the resume could not bring is named, not dropped"
    );
}

/// Esc goes up an altitude on the window and keeps every
/// terminal meaning: on `Surface::Native` the Session climbs to the
/// Sky, the Sky to the Galaxy, the Galaxy holds — and the shown tab
/// follows the place — while a terminal's Esc moves no tab at all and
/// has no place.
#[test]
fn esc_goes_up_an_altitude_on_the_window_only() {
    use crate::cockpit::Place;
    let mut w = model().with_surface(crate::cockpit::Surface::Native);
    assert_eq!(w.place(), Some(Place::Session));
    assert_eq!(w.shown_tab(), StageTab::Turn);
    // The move may start its transition — the motion layer's business.
    let _ = update(&mut w, key(KeyCode::Esc));
    assert_eq!(w.place(), Some(Place::Sky), "up to the Sky");
    assert_eq!(w.shown_tab(), StageTab::Roadmap);
    let _ = update(&mut w, key(KeyCode::Esc));
    assert_eq!(w.place(), Some(Place::Galaxy), "up to the Galaxy");
    let _ = update(&mut w, key(KeyCode::Esc));
    assert_eq!(w.place(), Some(Place::Galaxy), "the Galaxy is the top");
    let mut t = model();
    update(&mut t, key(KeyCode::Esc));
    assert_eq!(t.tab(), StageTab::Turn, "a terminal's Esc moves no tab");
    assert_eq!(t.place(), None, "a terminal has no place");
}

/// A crumb click moves the camera along the altitude path: `projects`
/// climbs to the Galaxy, the project crumb to the Sky, the session
/// crumb dives back to the Session, and `^` is one altitude up.
#[test]
fn a_crumb_click_climbs_the_altitude_path_and_the_session_crumb_dives_back() {
    use crate::cockpit::Place;
    use crate::input::{Button, MouseEvent, MouseKind};
    let click = |widget: Widget| Msg::Mouse {
        event: MouseEvent::new(MouseKind::Down(Button::Left), 12, 1),
        target: Some(widget.id()),
    };
    // A camera move may start its transition animation — the commands
    // are the motion layer's business, the *place* is this test's.
    let mut m = model().with_surface(crate::cockpit::Surface::Native);
    let _ = update(&mut m, click(Widget::CrumbProjects));
    assert_eq!(
        m.place(),
        Some(Place::Galaxy),
        "projects climbs to the Galaxy"
    );
    let _ = update(&mut m, click(Widget::CrumbSession));
    assert_eq!(
        m.place(),
        Some(Place::Session),
        "the session crumb returns to the Session"
    );
    let _ = update(&mut m, click(Widget::CrumbUp));
    assert_eq!(m.place(), Some(Place::Sky), "^ is one altitude up");
    let _ = update(&mut m, click(Widget::CrumbProject));
    assert_eq!(m.place(), Some(Place::Sky), "the project crumb is the Sky");
    let _ = update(&mut m, click(Widget::CrumbUp));
    let _ = update(&mut m, click(Widget::CrumbUp));
    assert_eq!(m.place(), Some(Place::Galaxy), "^ holds at the Galaxy");
}

/// An answer never moves the camera:
/// a key on the Moment's card, a click on its button, and a key on the
/// dock's strip each answer the ask and leave the place exactly where
/// it was — at the Sky, where the card came to you, and at the Session.
#[test]
fn an_answer_never_moves_the_camera() {
    use crate::cockpit::Place;
    use crate::input::{Button, MouseEvent, MouseKind};
    let click = |widget: Widget| Msg::Mouse {
        event: MouseEvent::new(MouseKind::Down(Button::Left), 12, 1),
        target: Some(widget.id()),
    };
    for place in [Place::Sky, Place::Session] {
        // A key on the card.
        let mut m = model().with_surface(crate::cockpit::Surface::Native);
        m.place = place;
        agent(&mut m, plan_request(1));
        assert!(m.moment().is_some());
        assert!(update(&mut m, key(KeyCode::Char('a'))).contains(&answered(1, "allow")[0]));
        assert_eq!(m.place(), Some(place), "a key answer held the camera");
        // A click on the card's button.
        agent(&mut m, plan_request(2));
        assert!(m.moment().is_some());
        assert!(update(&mut m, click(Widget::Deny)).contains(&answered(2, "reject")[0]));
        assert_eq!(m.place(), Some(place), "a click answer held the camera");
        // A key on the dock's strip — a plan-less ask never floats.
        agent(&mut m, request(3, "Bash"));
        assert!(m.moment().is_none());
        assert!(update(&mut m, key(KeyCode::Enter)).contains(&answered(3, "allow")[0]));
        assert_eq!(m.place(), Some(place), "a strip answer held the camera");
    }
}

/// The loop's boundary: the driver says which iteration ended and how
/// its gates went, the cockpit mints the row and asks the port for it
/// under that name, and the answer lands on the same row.
#[test]
fn an_iteration_boundary_mints_a_row_wearing_the_gates_and_asks_the_port_for_it() {
    let mut m = model();
    let gates = vec![
        ("check".to_owned(), true),
        ("test".to_owned(), true),
        ("lint".to_owned(), true),
    ];
    let cmds = update(
        &mut m,
        loop_msg(LoopEvent::Boundary {
            iteration: 3,
            gates: gates.clone(),
        }),
    );
    assert_eq!(
        cmds,
        vec![Cmd::Checkpoint(CheckpointCmd::Save {
            id: "cp-1".to_owned(),
            reason: CheckpointReason::Iteration,
            label: "iter 3".to_owned(),
        })],
        "the ask carries the name the cockpit minted"
    );
    let cp = m
        .rail()
        .spine()
        .get("cp-1")
        .expect("the row is on the rail before the port answers");
    assert_eq!(cp.label(), "iter 3");
    assert_eq!(cp.gates(), gates.as_slice());
    assert!(!cp.is_verified(), "a promise until the port answers");
    let notes: Vec<&str> = m
        .turn()
        .rows()
        .iter()
        .filter(|r| matches!(r.kind, RowKind::Note | RowKind::Error))
        .map(|r| r.text.as_str())
        .collect();
    assert!(
        notes.is_empty(),
        "a boundary is a row, not a sentence: {notes:?}"
    );
    update(
        &mut m,
        Msg::Checkpoint(CheckpointEvent::Saved {
            id: "cp-1".to_owned(),
            head: Some("0123456".to_owned()),
            verified: true,
        }),
    );
    assert!(
        m.rail().spine().get("cp-1").expect("cp-1").is_verified(),
        "the answer lands on the row the ask named"
    );
    assert_eq!(
        update(
            &mut m,
            loop_msg(LoopEvent::Boundary {
                iteration: 4,
                gates: Vec::new(),
            }),
        ),
        vec![Cmd::Checkpoint(CheckpointCmd::Save {
            id: "cp-2".to_owned(),
            reason: CheckpointReason::Iteration,
            label: "iter 4".to_owned(),
        })],
        "the next boundary takes the next name"
    );
}

#[test]
fn a_completed_call_mints_a_receipt_on_the_spines_own_counter() {
    use magistr_application::sessions::{EditDiff, ToolStatus, ToolVerb};
    let mut m = model();
    // The session's history: 239 receipts before this window opened,
    // then cp-1 — so the card's ordinal and the rail's after-head count
    // are two readings of the one counter.
    m.rail.spine_mut().seed_receipts(239);
    let cp = m.rail.spine_mut().open(
        magistr_domain::workflow::CheckpointReason::Iteration,
        "iter 1",
    );
    agent(
        &mut m,
        AgentEvent::ToolCall {
            id: "e1".to_owned(),
            title: "edit src/lib.rs".to_owned(),
            verb: ToolVerb::Edit,
            status: ToolStatus::InProgress,
            diff: Some(EditDiff {
                path: "src/lib.rs".to_owned(),
                old_text: String::new(),
                new_text: "pub fn one() {}\npub fn two() {}\n".to_owned(),
            }),
            images: Vec::new(),
            record: Default::default(),
        },
    );
    assert!(
        m.turn().receipt("e1").is_none(),
        "a call still running has earned nothing"
    );
    agent(
        &mut m,
        AgentEvent::ToolCallUpdate {
            id: "e1".to_owned(),
            title: None,
            status: Some(ToolStatus::Completed),
            diff: None,
            duration_ms: Some(180),
            images: Vec::new(),
            record: Default::default(),
        },
    );
    let receipt = m
        .turn()
        .receipt("e1")
        .expect("completion mints the receipt");
    assert_eq!((receipt.added, receipt.removed), (2, 0));
    assert_eq!(receipt.ordinal, 240, "the counter's next number");
    assert_eq!(receipt.reach.as_deref(), Some(cp.as_str()));
    assert_eq!(
        m.rail().spine().receipts_after_head(),
        Some(1),
        "the same counter the rail reads"
    );
}

#[test]
fn a_call_with_no_diff_still_gets_its_ordinal_and_an_uncheckpointed_session_has_no_reach() {
    use magistr_application::sessions::{ToolStatus, ToolVerb};
    let mut m = model();
    agent(
        &mut m,
        AgentEvent::ToolCall {
            id: "t1".to_owned(),
            title: "cargo test".to_owned(),
            verb: ToolVerb::Run,
            status: ToolStatus::Completed,
            diff: None,
            images: Vec::new(),
            record: Default::default(),
        },
    );
    let receipt = m
        .turn()
        .receipt("t1")
        .expect("arriving completed is a receipt too");
    assert_eq!((receipt.added, receipt.removed), (0, 0));
    assert_eq!(receipt.ordinal, 1);
    assert_eq!(
        receipt.reach, None,
        "nothing checkpointed, nothing to reach"
    );
}

// ── The review ────────────────────────────

/// A native cockpit with a verified state, a journal and an edit ask
/// waiting — the review surface's own model.
fn reviewing() -> Model {
    use crate::workbench::JournalEntry;
    use magistr_application::sessions::{PermissionChoice, PermissionKind};
    let mut m = Model::new(Size::new(125, 32)).with_surface(crate::cockpit::Surface::Native);
    {
        let spine = m.rail_mut().spine_mut();
        for label in ["iter 1", "iter 2", "iter 3"] {
            let id = spine.open(magistr_domain::workflow::CheckpointReason::Iteration, label);
            spine.saved(&id, Some("abc123".to_owned()), true);
        }
    }
    update(
        &mut m,
        Msg::Workbench(WorkbenchEvent::Journal {
            entries: vec![
                JournalEntry {
                    at_ms: None,
                    id: "e1".to_owned(),
                    path: "theme/ink.rs".to_owned(),
                    added: 31,
                    removed: 1,
                    parse_ok: true,
                    diff: "@@ -1,1 +1,1 @@\n-old\n+new".to_owned(),
                },
                JournalEntry {
                    at_ms: None,
                    id: "e2".to_owned(),
                    path: "theme/css.rs".to_owned(),
                    added: 9,
                    removed: 0,
                    parse_ok: true,
                    diff: "+added".to_owned(),
                },
            ],
        }),
    );
    update(
        &mut m,
        Msg::Agent(AgentEvent::PermissionRequest {
            id: 7,
            title: "Apply the change".to_owned(),
            choices: vec![
                PermissionChoice::new("allow", PermissionKind::AllowOnce),
                PermissionChoice::new("reject", PermissionKind::RejectOnce),
            ],
            edit: true,
            risk: RiskTier::Medium,
            plan: None,
        }),
    );
    m
}

fn click(id: crate::widget_id::WidgetId) -> Msg {
    Msg::Mouse {
        event: MouseEvent::new(MouseKind::Down(Button::Left), 0, 0),
        target: Some(id),
    }
}

#[test]
fn approve_answers_the_permission_and_brings_the_worktree_home() {
    let mut m = reviewing();
    let cmds = update(&mut m, click(Widget::Allow.id()));
    assert_eq!(
        cmds,
        vec![
            Cmd::Permission {
                id: 7,
                option_id: "allow".to_owned(),
            },
            Cmd::Checkpoint(CheckpointCmd::Merge {
                id: "cp-3".to_owned(),
            }),
        ],
        "the answer the strip sends, and the merge beside it"
    );
    assert!(m.review().is_none(), "the card left the dock");
}

#[test]
fn requesting_changes_denies_and_merges_nothing() {
    let mut m = reviewing();
    assert_eq!(
        update(&mut m, click(Widget::Deny.id())),
        vec![Cmd::Permission {
            id: 7,
            option_id: "reject".to_owned(),
        }]
    );
}

#[test]
fn a_review_answered_by_the_key_is_the_answer_the_button_sends() {
    let by_key = update(&mut reviewing(), key(KeyCode::Enter));
    let by_click = update(&mut reviewing(), click(Widget::Allow.id()));
    assert_eq!(by_key, by_click);
    assert_eq!(
        update(&mut reviewing(), key(KeyCode::Char('a'))),
        by_click,
        "the accelerator too"
    );
}

#[test]
fn a_review_with_nothing_verified_approves_without_a_merge() {
    let mut m = reviewing();
    // A spine whose rows are all unverified names no state to merge
    // from, so the approve is only an approve.
    *m.rail_mut().spine_mut() = crate::cockpit::Spine::default();
    assert_eq!(m.review_since(), None);
    assert_eq!(
        update(&mut m, click(Widget::Allow.id())),
        vec![Cmd::Permission {
            id: 7,
            option_id: "allow".to_owned(),
        }]
    );
}

#[test]
fn an_ordinary_permission_answered_on_a_window_merges_nothing() {
    let mut m = reviewing();
    // The same spine and the same window, but the ask is not an edit.
    m.dock.pop_permission();
    update(
        &mut m,
        Msg::Agent(AgentEvent::PermissionRequest {
            id: 8,
            title: "Bash".to_owned(),
            choices: vec![magistr_application::sessions::PermissionChoice::new(
                "allow",
                magistr_application::sessions::PermissionKind::AllowOnce,
            )],
            edit: false,
            risk: RiskTier::Medium,
            plan: None,
        }),
    );
    assert!(m.review().is_none());
    assert_eq!(
        update(&mut m, click(Widget::Allow.id())),
        vec![Cmd::Permission {
            id: 8,
            option_id: "allow".to_owned(),
        }]
    );
}

#[test]
fn a_click_on_a_reviewed_file_shows_that_files_diff() {
    let mut m = reviewing();
    assert_eq!(m.review_index(), 0);
    assert_eq!(update(&mut m, click(Widget::review_file(1))), vec![]);
    assert_eq!(m.review_index(), 1);
    assert_eq!(
        m.review_file().map(|entry| entry.path.as_str()),
        Some("theme/css.rs")
    );
    // A row that is not there changes nothing.
    update(&mut m, click(Widget::review_file(9)));
    assert_eq!(m.review_index(), 1);
}

#[test]
fn a_merge_the_port_carried_out_lands_on_the_stage() {
    let mut m = reviewing();
    update(
        &mut m,
        Msg::Checkpoint(CheckpointEvent::Merged {
            id: "cp-3".to_owned(),
            branch: "magistr/cp-3".to_owned(),
            head: "def456".to_owned(),
        }),
    );
    let rows: Vec<String> = m.turn().rows().iter().map(|row| row.text.clone()).collect();
    assert!(
        rows.iter()
            .any(|r| r.contains("merged cp-3 · magistr/cp-3")),
        "{rows:?}"
    );
}

// ── Rest: the sky's chrome fade ─

/// A window on the Sky, with a roadmap under it and no input yet.
fn resting_sky() -> Model {
    Model::new(Size::new(120, 40))
        .with_surface(Surface::Native)
        .with_tab(StageTab::Roadmap)
}

/// `n` ticks of nothing happening.
fn idle(model: &mut Model, n: u64) {
    for _ in 0..n {
        update(model, Msg::Tick);
    }
}

/// Rest is `ticks - last_input` and nothing else, so it is a pure
/// function of [`Model::ticks`] with no clock in reach — and it arrives
/// exactly when the countdown plus the fade are up, never earlier.
#[test]
fn the_sky_rests_once_the_countdown_and_the_fade_are_up() {
    let mut model = resting_sky();
    assert!(!model.sky_at_rest(), "a sky that just opened is awake");
    idle(
        &mut model,
        Motion::REST_STEPS + Motion::CHROME_FADE_STEPS - 1,
    );
    assert!(!model.sky_at_rest(), "the chrome is still exhaling");
    idle(&mut model, 1);
    assert!(model.sky_at_rest(), "the exhale is over");
}

/// Intent brings the chrome back over 150 ms, and the
/// countdown starts again from the input's own tick.
#[test]
fn input_restores_the_chrome_over_the_fade_and_restarts_the_countdown() {
    let mut model = resting_sky();
    idle(&mut model, Motion::REST_STEPS + Motion::CHROME_FADE_STEPS);
    assert!(model.sky_at_rest());
    update(&mut model, key(KeyCode::Char('x')));
    assert!(!model.sky_at_rest(), "intent overrides rest at once");
    assert_eq!(
        model.sky_chrome_fade(),
        Some(0.0),
        "the chrome comes back from nothing"
    );
    idle(&mut model, 1);
    assert_eq!(
        model.sky_chrome_fade(),
        Some(1.0 / Motion::CHROME_FADE_STEPS as f32)
    );
    idle(&mut model, Motion::CHROME_FADE_STEPS);
    assert_eq!(model.sky_chrome_fade(), None, "the chrome is simply there");
    idle(&mut model, Motion::REST_STEPS + Motion::CHROME_FADE_STEPS);
    assert!(model.sky_at_rest(), "and the countdown ran again");
}

/// The exhale is the same curve read the other way: 1 down to 0 over
/// the fade, once the countdown has run out.
#[test]
fn the_exhale_walks_the_fade_from_whole_to_gone() {
    let mut model = resting_sky();
    idle(&mut model, Motion::REST_STEPS);
    assert_eq!(model.sky_chrome_fade(), Some(1.0));
    idle(&mut model, 1);
    assert_eq!(
        model.sky_chrome_fade(),
        Some(1.0 - 1.0 / Motion::CHROME_FADE_STEPS as f32)
    );
    idle(&mut model, Motion::CHROME_FADE_STEPS);
    assert_eq!(model.sky_chrome_fade(), None, "the chrome is simply gone");
    assert!(model.sky_at_rest());
}

/// A question overrides rest — "the card comes to you". The chrome the
/// decision needs cannot be allowed to have exhaled under it.
#[test]
fn a_pending_question_overrides_rest() {
    let mut model = resting_sky();
    idle(&mut model, Motion::REST_STEPS + Motion::CHROME_FADE_STEPS);
    assert!(model.sky_at_rest());
    update(&mut model, Msg::Agent(request(1, "Write src/main.rs")));
    assert!(model.has_question());
    assert!(!model.sky_at_rest(), "a question is not a quiet sky");
    assert!(!model.is_sky());
    assert_eq!(model.sky_chrome_fade(), None);
}

/// A terminal never rests: rest is an altitude-1 window state, and a
/// cell cockpit is steered by the chrome the fade would take away.
#[test]
fn a_terminal_never_rests() {
    let mut model = Model::new(Size::new(80, 24)).with_tab(StageTab::Roadmap);
    idle(&mut model, 4 * Motion::REST_STEPS);
    assert!(!model.is_sky());
    assert!(!model.sky_at_rest());
    assert_eq!(model.sky_chrome_fade(), None);
}

/// Under reduced motion the tick never runs, so the difference never
/// grows and the chrome stays — the settled state read
/// as the end frame that shows the most.
#[test]
fn reduced_motion_keeps_the_chrome_and_asks_for_no_tick() {
    let model =
        resting_sky().with_theme(Theme::forced(ThemeMode::Dark).with_motion(MotionMode::Reduced));
    assert!(model.animations().is_empty());
    assert_eq!(model.sky_chrome_fade(), None);
}

/// The countdown is bounded: a window on the sky ticks until rest and
/// then stops, which is what keeps the idle guarantee intact.
#[test]
fn the_countdown_ends_and_a_rested_sky_asks_for_no_tick() {
    let mut model = resting_sky();
    assert!(model.is_animating(), "the countdown wants the tick");
    idle(&mut model, Motion::REST_STEPS + Motion::CHROME_FADE_STEPS);
    assert!(
        !model.is_animating(),
        "a rested sky is back to zero frames a second"
    );
}

// ── The Brief: the deal and its one exit ────────────────────────────

/// A window with news to report, opened after six hours away.
fn briefed() -> Model {
    let mut model = Model::new(Size::new(120, 40))
        .with_surface(Surface::Native)
        .with_tab(StageTab::Roadmap);
    update(
        &mut model,
        Msg::Engine(EngineEvent::local(crate::harness::fixtures::frame(
            "roadmap",
            "chunk_completed",
            serde_json::json!({
                "chunk": {"id": "e10-fleet", "name": "e10-fleet", "status": "done"}
            }),
        ))),
    );
    model.with_away(std::time::Duration::from_secs(6 * 3600 + 12 * 60))
}

/// Esc puts the Brief away, ahead of every other meaning Esc has — and
/// once, so the second Esc is the sky's again.
#[test]
fn esc_dismisses_the_brief_and_then_means_what_it_meant() {
    let mut model = briefed();
    assert!(model.brief().is_some());
    update(&mut model, key(KeyCode::Esc));
    assert!(model.brief().is_none(), "esc dismissed it");
    assert!(model.is_sky(), "and the sky is underneath");
}

/// The deal is the rise: cards land `RISE_STAGGER` apart, each over
/// `ARRIVAL_STEPS`, and the whole thing is over — so a settled still is
/// a still and not a frame of a motion.
#[test]
fn the_brief_deals_its_cards_a_stagger_apart_and_then_settles() {
    let mut model = briefed();
    let cards = model.brief().expect("there is news").cards().len();
    assert!(cards >= 1);
    assert_eq!(model.brief_rise(0), Some(0.0), "the first card is landing");
    assert_eq!(
        model.brief_rise(1),
        Some(0.0),
        "and the second has not been dealt"
    );
    assert!(model.brief_dealing());
    idle(&mut model, Motion::ARRIVAL_STEPS);
    assert_eq!(model.brief_rise(0), None, "the first card has landed");
    let last = (cards as u64 - 1) * Motion::RISE_STAGGER + Motion::ARRIVAL_STEPS;
    idle(&mut model, last);
    assert!(!model.brief_dealing(), "the deal is over");
}

/// The Brief is a native overlay, and a terminal has no altitude to
/// raise one over: the same model on a cell grid answers `None`.
#[test]
fn a_terminal_raises_no_brief() {
    let model = briefed().with_surface(Surface::Terminal);
    assert!(model.brief().is_none());
}

/// A question outranks the catch-up: the card comes to you, and the
/// summary of what already happened waits.
#[test]
fn a_question_takes_the_brief_off_the_screen() {
    let mut model = briefed();
    assert!(model.brief().is_some());
    update(&mut model, Msg::Agent(request(7, "Write src/main.rs")));
    assert!(model.brief().is_none(), "the question has the screen");
}

/// Every one of the Brief's controls puts the overlay away, and the
/// verbs then dive: Review climbs to the journal, Answer and View
/// reports frame their star on the constellation the summary was raised
/// over. The stuck star the card names is the stuck star the sky draws
/// — one id, read from one place.
#[test]
fn a_briefs_verb_dismisses_it_and_dives_to_the_thing() {
    let mut model = crate::harness::Scenes::briefed(Size::new(120, 40));
    assert!(model.brief().is_some());
    update(&mut model, click(Widget::BriefReview.id()));
    assert!(model.brief().is_none(), "the card was read");
    assert_eq!(
        model.place(),
        Some(crate::cockpit::Place::Changes),
        "and it dived to the journal"
    );

    let mut model = crate::harness::Scenes::briefed(Size::new(120, 40));
    update(&mut model, click(Widget::BriefAnswer.id()));
    assert!(model.brief().is_none());
    assert_eq!(
        model.roadmap().selected().map(|c| c.id.as_str()),
        Some("e6b"),
        "the stuck chunk the card named is the one the sky framed"
    );
    assert_eq!(
        model.place(),
        Some(crate::cockpit::Place::Sky),
        "no decision is on this window's dock, so its card is the place"
    );

    let mut model = crate::harness::Scenes::briefed(Size::new(120, 40));
    update(&mut model, click(Widget::BriefReports.id()));
    assert!(model.brief().is_none());
    assert_eq!(
        model.roadmap().selected().map(|c| c.id.as_str()),
        Some("e10-fleet"),
        "the first shipped chunk, opened on its Report"
    );
    assert_eq!(model.place(), Some(crate::cockpit::Place::Report));

    let mut model = crate::harness::Scenes::briefed(Size::new(120, 40));
    update(&mut model, click(Widget::BriefDismiss.id()));
    assert!(model.brief().is_none(), "dismiss is dismiss");
    assert_eq!(
        model.place(),
        Some(crate::cockpit::Place::Sky),
        "and nothing else moved"
    );
}

// ---------------------------------------------------------------------
// altitude 0 on the window — the clusters, the dive, the
// zoom.
// ---------------------------------------------------------------------

/// A window at the Galaxy with three projects delivered by the job —
/// the capture's own: magistr running, think-and-ship (this window's)
/// asking, sample-project idle.
fn galaxy_window() -> Model {
    use crate::cockpit::{GalaxyData, GalaxyProject, Place, Surface};
    let mut w = model()
        .with_surface(Surface::Native)
        .with_place(Place::Galaxy);
    let data = GalaxyData {
        projects: vec![
            GalaxyProject {
                name: "magistr".to_owned(),
                done: Some(211),
                running: 1,
                ..GalaxyProject::default()
            },
            GalaxyProject {
                name: "think-and-ship".to_owned(),
                here: true,
                running: 1,
                asking: 1,
                asked_secs: Some(11 * 60),
                ..GalaxyProject::default()
            },
            GalaxyProject {
                name: "sample-project".to_owned(),
                idle_secs: Some(3 * 86_400),
                ..GalaxyProject::default()
            },
        ],
        spent_cents: 245,
        cap_cents: Some(1200),
        stop_cents: Some(1000),
    };
    update(
        &mut w,
        Msg::Job {
            id: JobId::new(9),
            output: JobOutput::Galaxy(Box::new(data)),
        },
    );
    w
}

/// The Galaxy's job seeds the clusters through the one `update`, and
/// `↵` with nothing selected dives into the window's own project — the
/// camera moves to the Sky through the spine — and Esc climbs back to
/// the Galaxy.
#[test]
fn a_galaxy_job_seeds_the_clusters_and_enter_dives_home_and_esc_returns() {
    use crate::cockpit::Place;
    let mut w = galaxy_window();
    assert_eq!(w.galaxy().projects().len(), 3);
    assert_eq!(w.galaxy().running(), 2, "the header's 2 running");
    assert_eq!(w.galaxy().asking(), 1, "the header's 1 action required");
    assert_eq!(w.galaxy().home(), Some(1));
    assert_eq!(
        w.shown_tab(),
        StageTab::Roadmap,
        "the Galaxy stands in the launcher's slot"
    );
    let _ = update(&mut w, key(KeyCode::Enter));
    assert_eq!(
        w.place(),
        Some(Place::Sky),
        "↵ dives into this window's project"
    );
    assert_eq!(w.shown_tab(), StageTab::Roadmap);
    let _ = update(&mut w, key(KeyCode::Esc));
    assert_eq!(w.place(), Some(Place::Galaxy), "Esc returns to the Galaxy");
}

/// A click on another project's cluster selects it and never moves the
/// camera — this window is rooted on one project's engines, and a dive
/// that renamed this roadmap would lie; a click on the window's own
/// cluster dives. The arrows walk the selection.
#[test]
fn a_foreign_cluster_selects_and_the_home_cluster_dives() {
    use crate::cockpit::Place;
    let mut w = galaxy_window();
    let _ = update(&mut w, click(Widget::galaxy_cluster(0)));
    assert_eq!(w.galaxy().selected(), Some(0), "magistr is selected");
    assert_eq!(w.place(), Some(Place::Galaxy), "and the camera holds");
    let _ = update(&mut w, key(KeyCode::Right));
    assert_eq!(
        w.galaxy().selected(),
        Some(1),
        "→ walks onto the next cluster"
    );
    let _ = update(&mut w, key(KeyCode::Left));
    assert_eq!(w.galaxy().selected(), Some(0));
    let _ = update(&mut w, click(Widget::galaxy_cluster(1)));
    assert_eq!(
        w.place(),
        Some(Place::Sky),
        "this window's own cluster dives"
    );
    let _ = update(&mut w, click(Widget::galaxy_cluster(7)));
    assert_eq!(
        w.place(),
        Some(Place::Sky),
        "a cluster that is not there does nothing"
    );
}

/// The wheel over the field zooms it, `+`/`-`/`0` do the same from the
/// keys, and none of it reaches the Sky's own camera or a terminal.
#[test]
fn the_wheel_and_the_keys_zoom_the_galaxy_field() {
    use crate::input::{MouseEvent, MouseKind};
    let mut w = galaxy_window();
    let wheel = |kind: MouseKind| Msg::Mouse {
        event: MouseEvent::new(kind, 40, 12),
        target: Some(Widget::GalaxyField.id()),
    };
    let _ = update(&mut w, wheel(MouseKind::ScrollUp));
    assert!(w.galaxy().zoom() > 1.0, "a notch up zooms in");
    let _ = update(&mut w, wheel(MouseKind::ScrollDown));
    assert!(
        (w.galaxy().zoom() - 1.0).abs() < 1e-4,
        "a notch down zooms back out"
    );
    let _ = update(&mut w, key(KeyCode::Char('+')));
    assert!(w.galaxy().zoom() > 1.0);
    let _ = update(&mut w, key(KeyCode::Char('0')));
    assert!(
        (w.galaxy().zoom() - 1.0).abs() < f32::EPSILON,
        "0 rests the field"
    );
    let _ = update(&mut w, key(KeyCode::Char('-')));
    assert!(w.galaxy().zoom() < 1.0);
    assert!(
        (w.roadmap().zoom() - 1.0).abs() < f32::EPSILON,
        "the Sky's camera never moved"
    );
    // A terminal has no Galaxy: `+` with an empty prompt is the
    // prompt's own character there, as it always was.
    let mut t = model();
    let _ = update(&mut t, key(KeyCode::Char('+')));
    assert!((t.galaxy().zoom() - 1.0).abs() < f32::EPSILON);
    assert_eq!(t.prompt().text(), "+");
}

/// `＋ New project` is Birth F1's door: the camera
/// moves to Birth on the ground, nothing is sent yet, and the path is
/// the Galaxy's own — F1 floats over the Galaxy.
#[test]
fn new_project_opens_birth_on_the_ground() {
    use crate::cockpit::{BirthMoment, Place};
    let mut w = galaxy_window();
    let cmds = update(&mut w, click(Widget::GalaxyNew.id()));
    assert!(
        !cmds.iter().any(|cmd| matches!(cmd, Cmd::Birth(_))),
        "the door asks nothing yet: {cmds:?}"
    );
    assert_eq!(w.place(), Some(Place::Birth));
    assert_eq!(w.birth().moment(), BirthMoment::Ground);
    let path: Vec<String> = w.path().into_iter().map(|s| s.label).collect();
    assert_eq!(path, ["projects"]);
    // Esc on the ground rises to the Galaxy.
    let _ = update(&mut w, key(KeyCode::Esc));
    assert_eq!(w.place(), Some(Place::Galaxy));
    // A terminal has no Birth: the press falls through to nothing.
    let mut t = model();
    let _ = update(&mut t, click(Widget::GalaxyNew.id()));
    assert_eq!(t.place(), None);
    assert_eq!(t.birth().moment(), BirthMoment::Ground);
}

/// The whole flow through the one `update`: a folder dropped from the
/// desktop opens Birth on the survey and asks for it; the report lands
/// its rows by tick and the interview opens; three answers ask for the
/// chart; the proposal lands and Accept asks for the write; the
/// written roadmap seeds the Sky, names the window and shows first
/// light; Claim quits with the new folder's launch ask.
#[test]
fn a_dropped_folder_walks_the_five_moments() {
    use crate::cockpit::{
        BirthCmd, BirthMoment, BornData, Ground, Place, Proposal, ProposedChunk, ScaffoldKind,
        SurveyKind, SurveyReport,
    };
    use crate::input::{Button, MouseEvent, MouseKind};
    let mut w = galaxy_window();
    let cmds = update(&mut w, Msg::DragHover(true));
    assert!(
        !cmds.iter().any(|cmd| matches!(cmd, Cmd::Birth(_))),
        "a hover asks nothing yet: {cmds:?}"
    );
    assert_eq!(
        w.place(),
        Some(Place::Birth),
        "a hovering drag opens the ground"
    );
    assert!(w.birth().is_armed());
    let _ = update(&mut w, Msg::DragHover(false));
    assert!(!w.birth().is_armed(), "and leaving disarms it");
    let cmds = update(&mut w, Msg::Dropped("/tmp/tidewatch".to_owned()));
    assert!(
        cmds.contains(&Cmd::Birth(BirthCmd::Survey {
            ground: Ground::Folder("/tmp/tidewatch".to_owned())
        })),
        "{cmds:?}"
    );
    assert_eq!(w.birth().moment(), BirthMoment::Survey);
    let path: Vec<String> = w.path().into_iter().map(|s| s.label).collect();
    assert_eq!(path, ["projects", "new project \u{b7} /tmp/tidewatch"]);
    assert!(w.is_animating(), "the sweep runs while the survey is read");
    // The report lands.
    let mut report = SurveyReport::named("tidewatch");
    report.detected = vec![
        (SurveyKind::Stack, "Rust".to_owned()),
        (SurveyKind::Git, "main".to_owned()),
    ];
    report.written = vec![(
        ScaffoldKind::Config,
        ".magistr/project.json".to_owned(),
        true,
    )];
    report.never_break = vec!["the public API".to_owned()];
    let _ = update(
        &mut w,
        Msg::Job {
            id: JobId::new(3),
            output: JobOutput::Survey(Box::new(report)),
        },
    );
    assert_eq!(
        w.birth().moment(),
        BirthMoment::Survey,
        "the rows still land"
    );
    let steps = 3 * Motion::RISE_STAGGER + crate::cockpit::Birth::SURVEY_BEAT + 1;
    for _ in 0..steps {
        let _ = update(&mut w, Msg::Tick);
    }
    assert_eq!(w.birth().moment(), BirthMoment::Interview);
    assert!(!w.is_animating(), "the survey settled: no tick owed");
    let path: Vec<String> = w.path().into_iter().map(|s| s.label).collect();
    assert_eq!(path, ["projects", "new project \u{b7} tidewatch"]);
    // Esc on a later moment stands down to the ground with the survey
    // kept; choosing the same folder again asks nothing.
    let _ = update(&mut w, key(KeyCode::Esc));
    assert_eq!(w.birth().moment(), BirthMoment::Ground);
    assert_eq!(w.place(), Some(Place::Birth));
    let cmds = update(&mut w, Msg::Dropped("/tmp/tidewatch".to_owned()));
    assert!(
        !cmds.iter().any(|cmd| matches!(cmd, Cmd::Birth(_))),
        "the survey is kept: {cmds:?}"
    );
    for _ in 0..steps {
        let _ = update(&mut w, Msg::Tick);
    }
    assert_eq!(w.birth().moment(), BirthMoment::Interview);
    // The interview: typed words, a pick, a pick.
    for c in "a tide service".chars() {
        let _ = update(&mut w, key(KeyCode::Char(c)));
    }
    assert_eq!(w.prompt().text(), "", "the answer never reaches the prompt");
    let _ = update(&mut w, key(KeyCode::Enter));
    let _ = update(&mut w, click(Widget::birth_chip(0)));
    let _ = update(&mut w, click(Widget::BirthContinue.id()));
    let _ = update(&mut w, click(Widget::birth_chip(2)));
    let cmds = update(&mut w, key(KeyCode::Enter));
    assert!(
        cmds.iter().any(
            |cmd| matches!(cmd, Cmd::Birth(BirthCmd::Chart { answers, .. })
            if answers[1] == "the public API")
        ),
        "{cmds:?}"
    );
    assert_eq!(w.birth().moment(), BirthMoment::Charting);
    assert!(
        !update(&mut w, key(KeyCode::Enter))
            .iter()
            .any(|cmd| matches!(cmd, Cmd::Birth(_))),
        "nothing to accept while drafting"
    );
    let proposal = Proposal {
        chunks: vec![
            ProposedChunk::new("harmonic-engine", "core", 100),
            ProposedChunk::new("forecast-endpoint", "api", 110).standing_on(&["harmonic-engine"]),
        ],
        sources: vec!["Code scan".to_owned()],
    };
    let _ = update(
        &mut w,
        Msg::Job {
            id: JobId::new(4),
            output: JobOutput::Proposal(Box::new(proposal.clone())),
        },
    );
    assert!(w.is_animating(), "the stars are being born");
    let path: Vec<String> = w.path().into_iter().map(|s| s.label).collect();
    assert_eq!(path, ["projects", "tidewatch \u{b7} Proposed roadmap"]);
    // Drag the second row over the first, then rename it.
    let _ = update(&mut w, click(Widget::birth_row(1)));
    let _ = update(
        &mut w,
        Msg::Mouse {
            event: MouseEvent::new(MouseKind::Up(Button::Left), 0, 0),
            target: Some(Widget::birth_row(0)),
        },
    );
    assert_eq!(
        w.birth().proposal().map(|p| p.chunks[0].id.as_str()),
        Some("forecast-endpoint")
    );
    let _ = update(
        &mut w,
        Msg::Mouse {
            event: MouseEvent::new(MouseKind::Down(Button::Right), 0, 0),
            target: Some(Widget::birth_row(0)),
        },
    );
    assert_eq!(w.birth().editing().map(|(i, _)| i), Some(0));
    let _ = update(&mut w, key(KeyCode::Backspace));
    let _ = update(&mut w, key(KeyCode::Char('s')));
    let _ = update(&mut w, key(KeyCode::Enter));
    assert_eq!(
        w.birth().proposal().map(|p| p.chunks[0].name.as_str()),
        Some("forecast endpoins")
    );
    let cmds = update(&mut w, click(Widget::BirthRedraw.id()));
    assert!(
        cmds.iter().any(
            |cmd| matches!(cmd, Cmd::Birth(BirthCmd::Chart { pinned, .. })
            if pinned.len() == 1 && pinned[0].name == "forecast endpoins")
        ),
        "the moved and renamed star is the one pin: {cmds:?}"
    );
    let _ = update(
        &mut w,
        Msg::Job {
            id: JobId::new(5),
            output: JobOutput::Proposal(Box::new(proposal.clone())),
        },
    );
    let cmds = update(&mut w, click(Widget::BirthAccept.id()));
    assert!(
        cmds.iter().any(
            |cmd| matches!(cmd, Cmd::Birth(BirthCmd::Accept { never_break, freedom, .. })
            if never_break == &vec!["the public API".to_owned()] && *freedom == Some(2))
        ),
        "{cmds:?}"
    );
    let born = BornData {
        name: "tidewatch".to_owned(),
        dir: "/tmp/tidewatch".to_owned(),
        chunks: proposal.records(),
        gates: vec!["cargo test".to_owned()],
        protected: Vec::new(),
    };
    let _ = update(
        &mut w,
        Msg::Job {
            id: JobId::new(6),
            output: JobOutput::Born(Box::new(born)),
        },
    );
    assert_eq!(w.birth().moment(), BirthMoment::FirstLight);
    assert!(w.birth_shows_sky());
    assert_eq!(w.title(), Some("tidewatch"));
    assert_eq!(
        w.roadmap().rows().len(),
        2,
        "the Sky is seeded from what was written"
    );
    let path: Vec<String> = w.path().into_iter().map(|s| s.label).collect();
    assert_eq!(path, ["projects", "tidewatch"]);
    let cmds = update(&mut w, click(Widget::BirthClaim.id()));
    assert!(cmds.contains(&Cmd::Quit), "{cmds:?}");
    assert_eq!(
        w.launch(),
        Some(&LaunchAction::Born {
            dir: "/tmp/tidewatch".to_owned(),
            chunk: "harmonic-engine".to_owned()
        })
    );
}

/// `Later` on first light lands on the Sky; a failed survey stands the
/// flow down; the ground card's three ways in each open a field, and
/// Enter on a typed field chooses.
#[test]
fn the_ground_fields_choose_and_later_lands_on_the_sky() {
    use crate::cockpit::{BirthCmd, BirthMoment, BornData, Ground, GroundField, Place};
    let mut w = galaxy_window();
    let _ = update(&mut w, click(Widget::GalaxyNew.id()));
    let _ = update(&mut w, click(Widget::BirthClone.id()));
    assert_eq!(w.birth().field().map(|(f, _)| f), Some(GroundField::Clone));
    assert!(
        !update(&mut w, key(KeyCode::Enter))
            .iter()
            .any(|cmd| matches!(cmd, Cmd::Birth(_))),
        "an empty field chooses nothing"
    );
    let _ = update(&mut w, key(KeyCode::Esc));
    assert_eq!(w.birth().field(), None, "Esc closes the field first");
    assert_eq!(w.place(), Some(Place::Birth));
    let _ = update(&mut w, click(Widget::BirthEmpty.id()));
    for c in "/tmp/harbor".chars() {
        let _ = update(&mut w, key(KeyCode::Char(c)));
    }
    let cmds = update(&mut w, key(KeyCode::Enter));
    assert!(
        cmds.contains(&Cmd::Birth(BirthCmd::Survey {
            ground: Ground::Empty("/tmp/harbor".to_owned())
        })),
        "{cmds:?}"
    );
    let _ = update(
        &mut w,
        Msg::Job {
            id: JobId::new(3),
            output: JobOutput::Failed("no such folder".to_owned()),
        },
    );
    assert_eq!(
        w.birth().moment(),
        BirthMoment::Ground,
        "a failed survey stands down"
    );
    let _ = update(&mut w, click(Widget::BirthSlot.id()));
    assert_eq!(w.birth().field().map(|(f, _)| f), Some(GroundField::Browse));
    // First light's Later.
    let mut w = galaxy_window();
    let _ = update(&mut w, Msg::Dropped("/tmp/x".to_owned()));
    w.birth.born(BornData {
        name: "x".to_owned(),
        ..BornData::default()
    });
    let _ = update(&mut w, click(Widget::BirthLater.id()));
    assert_eq!(w.place(), Some(Place::Sky));
    assert!(
        !update(&mut w, click(Widget::BirthClaim.id())).contains(&Cmd::Quit),
        "nothing to claim without a ready chunk"
    );
}

// ── The deliverables shelf ───────────────────────────

/// Seed one roadmap chunk in `status`, naming a think and a task ref.
fn shelf_chunk_frame(id: &str, status: &str) -> Msg {
    frame(
        "roadmap",
        "chunk_changed",
        serde_json::json!({"chunk": {
            "id": id, "name": id, "title": "a shipped chunk", "status": status,
            "priority": 170, "group": "Desktop GUI",
            "cross_refs": ["think:7", "task:verify"],
        }}),
    )
}

/// The star index of `id` on the sky's graph.
fn star_of(m: &Model, id: &str) -> usize {
    m.roadmap()
        .graph_ids()
        .iter()
        .position(|graph| *graph == id)
        .expect("the chunk is on the graph")
}

#[test]
fn opening_a_done_star_on_a_window_asks_for_its_shelf_once_and_export_asks_for_the_bundle() {
    use crate::cockpit::{Shelf, ShelfCmd};
    let mut w = Model::new(Size::new(120, 40)).with_surface(Surface::Native);
    update(&mut w, shelf_chunk_frame("shipped", "done"));
    let star = star_of(&w, "shipped");
    // The star click also glides the camera (an Animate rides along);
    // the shelf's own answer is the one asked about.
    let shelves = |cmds: Vec<Cmd>| -> Vec<Cmd> {
        cmds.into_iter()
            .filter(|cmd| matches!(cmd, Cmd::Shelf(_)))
            .collect()
    };
    assert_eq!(
        shelves(update(&mut w, click(Widget::sky_star(star)))),
        vec![Cmd::Shelf(ShelfCmd::Collect {
            chunk: "shipped".to_owned()
        })],
        "a Done chunk opened on a window asks for its deliverables"
    );
    assert_eq!(w.shelf(), None, "nothing to show until the job lands");
    // Nothing is exported while there is no shelf to export.
    assert!(shelves(update(&mut w, click(Widget::ShelfExport.id()))).is_empty());

    let shelf = Shelf::new("shipped", &["think:7".to_owned(), "task:verify".to_owned()]);
    update(
        &mut w,
        Msg::Job {
            id: JobId::new(3),
            output: JobOutput::Shelf(Box::new(shelf.clone())),
        },
    );
    assert_eq!(w.shelf(), Some(&shelf), "one round-trip fills the gallery");
    assert!(
        shelves(update(&mut w, click(Widget::sky_star(star)))).is_empty(),
        "a shelf the model already holds is not asked for again"
    );
    let chapter = w.chapter().expect("a shipped chunk selected has a chapter");
    assert_eq!(chapter.title, "a shipped chunk");
    assert_eq!(
        shelves(update(&mut w, click(Widget::ShelfExport.id()))),
        vec![Cmd::Shelf(ShelfCmd::Export {
            chunk: "shipped".to_owned(),
            chapter: chapter.clone(),
        })],
        "the bundle carries the chapter the view composed"
    );
    assert_eq!(
        shelves(update(&mut w, click(Widget::ShelfShare.id()))),
        vec![Cmd::Shelf(ShelfCmd::Share {
            chunk: "shipped".to_owned(),
            chapter,
        })],
        "the page publishes the same document"
    );
}

/// The Report's receipt verbs do one honest thing each: the commit,
/// the gates and the cost go on the stage as notes (the window has no
/// viewer for a record), and the trace's `Open` enters the Trace with
/// the chunk's first reasoning step expanded — the provenance chip.
#[test]
fn the_reports_receipt_verbs_put_the_receipt_on_the_stage_or_open_the_trace() {
    use crate::cockpit::Shelf;
    let mut w = Model::new(Size::new(120, 40)).with_surface(Surface::Native);
    update(&mut w, shelf_chunk_frame("shipped", "done"));
    // The reasoning step the chunk's `` names, on the feed.
    update(
        &mut w,
        frame(
            "think",
            "step_appended",
            serde_json::json!({"step": {"step_number": 7, "purpose": "open shipped"}}),
        ),
    );
    let star = star_of(&w, "shipped");
    // Nothing moves without a shelf to read.
    update(&mut w, click(Widget::sky_star(star)));
    work::open_report(&mut w);
    assert_eq!(w.place(), Some(Place::Report));
    update(&mut w, click(Widget::ReportCommit.id()));
    assert_eq!(
        w.place(),
        Some(Place::Report),
        "no shelf, no receipt to show"
    );

    let mut shelf = Shelf::new("shipped", &["think:7".to_owned(), "task:verify".to_owned()]);
    shelf.commit = Some(("a3f9c21".to_owned(), "feat: shipped".to_owned()));
    shelf.gates = vec![("check".to_owned(), true), ("test".to_owned(), false)];
    shelf.cost_cents = Some(207);
    update(
        &mut w,
        Msg::Job {
            id: JobId::new(3),
            output: JobOutput::Shelf(Box::new(shelf)),
        },
    );
    let notes = |w: &Model| -> Vec<String> {
        w.turn()
            .rows()
            .iter()
            .filter(|row| row.kind == RowKind::Note)
            .map(|row| row.text.clone())
            .collect()
    };
    update(&mut w, click(Widget::ReportGates.id()));
    assert_eq!(w.place(), Some(Place::Session), "the stage shows the log");
    assert_eq!(
        notes(&w),
        vec![
            "gate \u{b7} check \u{b7} passed".to_owned(),
            "gate \u{b7} test \u{b7} failed".to_owned(),
        ]
    );
    update(&mut w, click(Widget::sky_star(star)));
    update(&mut w, click(Widget::ReportCommit.id()));
    update(&mut w, click(Widget::sky_star(star)));
    update(&mut w, click(Widget::ReportCost.id()));
    let notes = notes(&w);
    assert!(
        notes.contains(&"commit \u{b7} a3f9c21 \u{b7} feat: shipped".to_owned()),
        "{notes:?}"
    );
    assert!(
        notes.contains(&"cost \u{b7} 2.07 USD".to_owned()),
        "{notes:?}"
    );

    update(&mut w, click(Widget::sky_star(star)));
    update(&mut w, click(Widget::ReportTrace.id()));
    assert_eq!(
        w.place(),
        Some(Place::Trace),
        "the provenance chip enters the Trace"
    );
    assert!(
        w.trace().is_expanded(7),
        "on the chunk's first reasoning step"
    );
}

#[test]
fn a_terminal_and_an_unshipped_chunk_ask_for_no_shelf() {
    // A terminal has no gallery: opening a Done chunk there turns the
    // stage to the Roadmap tab and asks for nothing.
    let mut t = Model::new(Size::new(120, 40));
    update(&mut t, shelf_chunk_frame("shipped", "done"));
    let star = star_of(&t, "shipped");
    let cmds = update(&mut t, click(Widget::sky_star(star)));
    assert!(
        !cmds.iter().any(|cmd| matches!(cmd, Cmd::Shelf(_))),
        "{cmds:?}"
    );
    assert_eq!(
        t.roadmap().selected().map(|c| c.id.as_str()),
        Some("shipped")
    );

    // A chunk that has not shipped left nothing behind.
    let mut w = Model::new(Size::new(120, 40)).with_surface(Surface::Native);
    update(&mut w, shelf_chunk_frame("planned", "pending"));
    let star = star_of(&w, "planned");
    let cmds = update(&mut w, click(Widget::sky_star(star)));
    assert!(
        !cmds.iter().any(|cmd| matches!(cmd, Cmd::Shelf(_))),
        "{cmds:?}"
    );
}

#[test]
fn a_shelf_shows_only_under_its_own_done_chunk() {
    use crate::cockpit::Shelf;
    let mut w = Model::new(Size::new(120, 40)).with_surface(Surface::Native);
    update(&mut w, shelf_chunk_frame("first", "done"));
    update(&mut w, shelf_chunk_frame("second", "done"));
    let shelf = Shelf::new("first", &[]);
    update(
        &mut w,
        Msg::Job {
            id: JobId::new(0),
            output: JobOutput::Shelf(Box::new(shelf.clone())),
        },
    );
    assert_eq!(w.shelf(), None, "no selection, no shelf");
    let first = star_of(&w, "first");
    let second = star_of(&w, "second");
    update(&mut w, click(Widget::sky_star(first)));
    assert_eq!(w.shelf(), Some(&shelf));
    // Another chunk's card never wears the first one's receipts — and
    // opening it asks for its own.
    let cmds = update(&mut w, click(Widget::sky_star(second)));
    assert_eq!(w.shelf(), None);
    assert!(
        cmds.iter().any(|cmd| matches!(cmd, Cmd::Shelf(_))),
        "{cmds:?}"
    );
    // The shelf is a Done chunk's: the same record under a chunk that
    // went back to work shows nothing.
    update(&mut w, click(Widget::sky_star(first)));
    assert_eq!(w.shelf(), Some(&shelf));
    update(&mut w, shelf_chunk_frame("first", "in_progress"));
    assert_eq!(w.shelf(), None);
}

// ── The motion vocabulary ─────

/// Reduced motion answers the empty set, structurally: nothing is in
/// flight, so there is no hero and no curve pinned at its end.
#[test]
fn reduced_motion_names_nothing_in_flight() {
    let mut m = model().with_theme(Theme::default().with_motion(MotionMode::Reduced));
    agent(&mut m, AgentEvent::Text("hi\n".to_owned()));
    assert!(m.motions_in_flight().is_empty());
    assert_eq!(m.motion_hero(), None);
    let mut w = on_a_window(model().with_theme(Theme::default().with_motion(MotionMode::Reduced)));
    agent(&mut w, AgentEvent::Text("hi\n".to_owned()));
    assert!(w.motions_in_flight().is_empty());
    assert!(!w.moves(MotionName::Breathe));
}

/// The turn streaming is the *sweep* and the rows it lands *arrive*;
/// on a terminal the spinner beside the block *breathes*, on a window
/// the working thing does — and the sweep, being news, is the hero.
#[test]
fn a_streaming_turn_is_the_sweep_and_the_working_thing_breathes() {
    let mut m = model();
    agent(&mut m, AgentEvent::Text("line\n".to_owned()));
    let names = m.motions_in_flight();
    assert!(names.contains(&MotionName::Sweep), "{names:?}");
    assert!(names.contains(&MotionName::Breathe), "{names:?}");
    assert!(names.contains(&MotionName::Arrive), "{names:?}");
    assert_eq!(m.motion_hero(), Some(MotionName::Sweep));
    assert!(m.animates(crate::cockpit::Animation::Motion));
    // The names answer in declaration order, each once.
    let order: Vec<usize> = names
        .iter()
        .map(|n| MotionName::ALL.iter().position(|a| a == n).expect("named"))
        .collect();
    assert!(order.windows(2).all(|w| w[0] < w[1]), "{names:?}");

    let mut w = on_a_window(model());
    agent(&mut w, AgentEvent::Text("line\n".to_owned()));
    assert!(w.moves(MotionName::Breathe));
    assert!(w.moves(MotionName::Sweep));
    // The turn ending settles the sweep and the breath; the row's
    // arrival is the last thing named, and then nothing is.
    agent(&mut w, AgentEvent::TurnEnded(TurnEnd::EndTurn));
    assert!(!w.moves(MotionName::Sweep));
    assert!(!w.moves(MotionName::Breathe));
    settle(&mut w);
    assert!(
        w.motions_in_flight().is_empty(),
        "{:?}",
        w.motions_in_flight()
    );
    assert!(!w.is_animating());
}

/// A decision screen holds still: the ⌘⇧F search float over a
/// working window names nothing at all, and the breath stands down
/// with it — through the one predicate the window's breath reads.
#[test]
fn a_decision_screen_holds_still() {
    let mut w = on_a_window(model());
    agent(&mut w, AgentEvent::Text("working\n".to_owned()));
    assert_eq!(w.motion_hero(), Some(MotionName::Sweep));
    let chord =
        Msg::Key(KeyEvent::new(KeyCode::Char('F')).with(Modifiers::SUPER | Modifiers::SHIFT));
    update(&mut w, chord);
    assert!(w.search().is_some(), "the search float is up");
    assert!(w.motions_in_flight().is_empty());
    assert_eq!(w.motion_hero(), None);
    assert!(!w.moves(MotionName::Breathe));
    update(&mut w, key(KeyCode::Esc));
    assert!(w.search().is_none());
    assert_eq!(w.motion_hero(), Some(MotionName::Sweep), "and it is back");
}

/// The Galaxy's live clusters breathe — the screen's one hero — and
/// ask for the tick the breath needs; at night the shipped stars
/// twinkle beside it, by day ink is still; an idle galaxy asks for
/// nothing.
#[test]
fn the_galaxy_breathes_while_a_project_is_live_and_holds_still_otherwise() {
    use crate::cockpit::{GalaxyData, GalaxyProject, Place};
    let w = galaxy_window();
    assert!(
        w.animates(crate::cockpit::Animation::Motion),
        "the live cluster's breath asks for the tick"
    );
    assert_eq!(w.motion_hero(), Some(MotionName::Breathe));
    assert!(
        w.moves(MotionName::Twinkle),
        "magistr's shipped stars twinkle at night"
    );
    let day = galaxy_window().with_theme(Theme::forced(ThemeMode::Light));
    assert_eq!(
        day.motions_in_flight(),
        [MotionName::Breathe],
        "ink is still"
    );

    let mut idle = model()
        .with_surface(Surface::Native)
        .with_place(Place::Galaxy);
    update(
        &mut idle,
        Msg::Job {
            id: JobId::new(9),
            output: JobOutput::Galaxy(Box::new(GalaxyData {
                projects: vec![GalaxyProject {
                    name: "sample-project".to_owned(),
                    done: Some(40),
                    idle_secs: Some(3 * 86_400),
                    ..GalaxyProject::default()
                }],
                ..GalaxyData::default()
            })),
        },
    );
    assert!(!idle.is_animating(), "no live project, no tick");
    assert!(
        idle.motions_in_flight().is_empty(),
        "and the twinkle holds its phase"
    );
    assert_eq!(idle.motion_hero(), None);
}

/// The handshake binds the rail's agent block: the model the wire named under the seeded mode word, and
/// the ledger line moves with the one metering message and the turn
/// boundary — across sessions, the way the meter does.
#[test]
fn the_handshake_binds_the_agent_block_and_the_ledger_moves() {
    use magistr_application::sessions::TurnEnd;
    let mut m = model();
    m.rail_mut().set_mode("Workflow mode");
    assert_eq!(m.rail().agent(), None, "no block before the handshake");
    update(
        &mut m,
        Msg::Agent(AgentEvent::Initialized {
            session_id: "s1".to_owned(),
            model: Some("sonnet".to_owned()),
        }),
    );
    let agent = m.rail().agent().expect("bound by the handshake");
    assert_eq!(agent.identity(), "sonnet · Workflow mode");
    assert_eq!(agent.ledger(), "0.00 USD · 0 min · 0 requests");
    update(
        &mut m,
        Msg::Agent(AgentEvent::Usage {
            used: 1_000,
            size: 200_000,
            cost_cents: Some(61),
            elapsed_secs: Some(12 * 60 + 5),
        }),
    );
    update(&mut m, Msg::Agent(AgentEvent::TurnEnded(TurnEnd::EndTurn)));
    update(&mut m, Msg::Agent(AgentEvent::TurnEnded(TurnEnd::EndTurn)));
    assert_eq!(
        m.rail().agent().expect("bound").ledger(),
        "0.61 USD · 12 min · 2 requests"
    );
    // The loop's next iteration: a new session on another model, whose
    // cost and clock start over — the run's ledger keeps counting.
    update(
        &mut m,
        Msg::Agent(AgentEvent::Initialized {
            session_id: "s2".to_owned(),
            model: Some("haiku".to_owned()),
        }),
    );
    update(
        &mut m,
        Msg::Agent(AgentEvent::Usage {
            used: 500,
            size: 200_000,
            cost_cents: Some(39),
            elapsed_secs: Some(60),
        }),
    );
    update(&mut m, Msg::Agent(AgentEvent::TurnEnded(TurnEnd::EndTurn)));
    let agent = m.rail().agent().expect("rebound");
    assert_eq!(agent.identity(), "haiku · Workflow mode");
    assert_eq!(agent.ledger(), "1.00 USD · 13 min · 3 requests");
}

#[test]
fn a_terminals_bytes_are_kept_by_id_in_order_and_show_nothing() {
    let mut m = model();
    agent(
        &mut m,
        AgentEvent::Terminal {
            terminal_id: "term-1".to_owned(),
            bytes: b"\x1b[31mred".to_vec(),
        },
    );
    agent(
        &mut m,
        AgentEvent::Terminal {
            terminal_id: "term-1".to_owned(),
            bytes: b"\x1b[0m\r\n".to_vec(),
        },
    );
    assert_eq!(
        m.terminal_output("term-1"),
        Some(&b"\x1b[31mred\x1b[0m\r\n"[..])
    );
    assert_eq!(m.terminal_output("term-2"), None);
    assert!(m.turn().is_empty(), "bytes are data, never a row");
}

#[test]
fn a_gates_bytes_arrive_as_engine_frames_and_are_kept_by_the_gate() {
    // Both gate callers speak the same frame: the loop driver posts it on
    // the inbox, mcp-serve's socket relays it — and it folds into the
    // terminal store, never into the rail, the trace or the stage.
    let mut m = model();
    let frames_before = m.engine_frames();
    for chunk in [
        &b"\x1b[32m   Compiling\x1b[0m foo\r\n"[..],
        b"   Finished\r\n",
    ] {
        let output = magistr_workflow_contracts::GateOutput {
            gate: "check".to_owned(),
            bytes: chunk.to_vec(),
        };
        let cmds = update(
            &mut m,
            Msg::Engine(EngineEvent::local(
                magistr_workflow_contracts::NativeEvent::Magistr(
                    magistr_workflow_contracts::MagistrFrame::GateOutput(output),
                ),
            )),
        );
        assert!(cmds.is_empty(), "{cmds:?}");
    }
    assert_eq!(
        m.terminal_output("check"),
        Some(&b"\x1b[32m   Compiling\x1b[0m foo\r\n   Finished\r\n"[..])
    );
    assert_eq!(m.terminal_output("test"), None);
    assert!(m.turn().is_empty(), "a gate's bytes are never a row");
    assert_eq!(
        m.engine_frames(),
        frames_before,
        "a gate's chunk is not an engine mutation"
    );
    assert_eq!(m.phase(), None, "and moves no phase");
}

/// A running run card whose terminal has streamed, open on the stage.
fn live_pane() -> Model {
    use magistr_application::sessions::{ToolRecord, ToolStatus, ToolVerb};
    let mut m = model();
    agent(
        &mut m,
        AgentEvent::ToolCall {
            id: "c1".to_owned(),
            title: "cargo test".to_owned(),
            verb: ToolVerb::Run,
            status: ToolStatus::InProgress,
            diff: None,
            images: Vec::new(),
            record: Box::new(ToolRecord {
                terminal: Some("term-1".to_owned()),
                ..ToolRecord::default()
            }),
        },
    );
    let lines: String = (1..=30).map(|i| format!("line {i}\r\n")).collect();
    agent(
        &mut m,
        AgentEvent::Terminal {
            terminal_id: "term-1".to_owned(),
            bytes: lines.into_bytes(),
        },
    );
    update(&mut m, key(KeyCode::Enter));
    m
}

#[test]
fn a_terminals_bytes_make_a_screen_stamped_with_the_tick_they_arrived_on() {
    let mut m = model();
    for _ in 0..5 {
        update(&mut m, Msg::Tick);
    }
    agent(
        &mut m,
        AgentEvent::Terminal {
            terminal_id: "term-1".to_owned(),
            bytes: b"\x1b[31mred\x1b[0m\r\n".to_vec(),
        },
    );
    let term = m.term("term-1").expect("a screen");
    assert_eq!(
        term.pane_row(0).expect("a row")[0].fg,
        crate::render::Color::Indexed(1)
    );
    assert_eq!(term.stamp(0), Some(5), "the tick the row arrived on");
    assert!(m.term("term-2").is_none());
}

#[test]
fn an_open_run_card_shows_its_pane_and_a_completion_freezes_it() {
    use magistr_application::sessions::ToolStatus;
    let mut m = live_pane();
    let rows = m.turn().rows();
    let panes = rows.iter().filter(|r| r.kind == RowKind::Pane).count();
    assert_eq!(
        panes,
        crate::turn::Turn::PANE_ROWS,
        "the card opened into its pane"
    );
    assert!(
        m.term("term-1").is_some_and(|t| t.cursor().is_some()),
        "live: a cursor"
    );
    agent(
        &mut m,
        AgentEvent::ToolCallUpdate {
            id: "c1".to_owned(),
            title: None,
            status: Some(ToolStatus::Completed),
            diff: None,
            duration_ms: Some(842),
            images: Vec::new(),
            record: Default::default(),
        },
    );
    let term = m.term("term-1").expect("still there");
    assert!(term.is_finished(), "the pane froze with the run");
    assert_eq!(term.cursor(), None, "the cursor stopped");
}

#[test]
fn a_recorded_check_freezes_its_gates_pane_and_seats_it_on_the_card() {
    let mut m = model();
    let output = magistr_workflow_contracts::GateOutput {
        gate: "check".to_owned(),
        bytes: b"\x1b[32m   Compiling\x1b[0m foo\r\n".to_vec(),
    };
    update(
        &mut m,
        Msg::Engine(EngineEvent::local(
            magistr_workflow_contracts::NativeEvent::Magistr(
                magistr_workflow_contracts::MagistrFrame::GateOutput(output),
            ),
        )),
    );
    let check = |name: &str, extra: serde_json::Value| {
        let mut check = serde_json::json!({"name": name, "passed": true});
        if let (Some(check), Some(extra)) = (check.as_object_mut(), extra.as_object()) {
            for (key, value) in extra {
                check.insert(key.clone(), value.clone());
            }
        }
        Msg::Engine(crate::harness::fixtures::event(
            "ship",
            "check_recorded",
            serde_json::json!({"check": check}),
        ))
    };
    update(
        &mut m,
        check(
            "check",
            serde_json::json!({"exit_code": 0, "duration_ms": 900, "details": "ok"}),
        ),
    );
    assert!(
        m.term("check")
            .is_some_and(crate::cockpit::Term::is_finished)
    );
    assert_eq!(m.turn().len(), 1, "the card, folded");
    update(&mut m, key(KeyCode::Enter));
    assert_eq!(
        m.turn().pane_at_row(2),
        Some(("check", 0)),
        "the check card opens into the gate's pane"
    );
    // A gate that never streamed makes a card with no pane.
    let mut quiet = model();
    update(&mut quiet, check("lint", serde_json::json!({})));
    update(&mut quiet, key(KeyCode::Enter));
    assert!(quiet.turn().rows().iter().all(|r| r.kind != RowKind::Pane));
}

#[test]
fn a_notch_inside_the_pane_scrolls_the_pane_and_f_returns_it_to_its_tail() {
    use crate::input::{MouseEvent, MouseKind};
    let mut m = live_pane();
    let first = m
        .turn()
        .rows()
        .iter()
        .position(|r| r.kind == RowKind::Pane)
        .expect("a pane row");
    settle(&mut m);
    let index = first - m.top_row();
    let stage_before = m.top_row();
    let notch = |kind, index: usize| Msg::Mouse {
        event: MouseEvent::new(kind, 10, 5),
        target: Some(Widget::stage_row(index)),
    };
    update(&mut m, notch(MouseKind::ScrollUp, index));
    settle(&mut m);
    let term = m.term("term-1").expect("a screen");
    assert!(!term.is_following(), "the pane stopped following");
    assert_eq!(term.offset(), Viewport::WHEEL);
    assert_eq!(m.top_row(), stage_before, "the stage did not move");
    // Rows arriving under it are counted, not shown.
    agent(
        &mut m,
        AgentEvent::Terminal {
            terminal_id: "term-1".to_owned(),
            bytes: b"more\r\nand more\r\n".to_vec(),
        },
    );
    assert_eq!(m.term("term-1").map(crate::cockpit::Term::arrived), Some(2));
    // The two rows that arrived anchored the view two further up, so
    // one notch down lands short of the tail and a second reaches it.
    update(&mut m, notch(MouseKind::ScrollDown, index));
    assert_eq!(m.term("term-1").map(crate::cockpit::Term::offset), Some(2));
    update(&mut m, notch(MouseKind::ScrollDown, index));
    assert!(
        m.term("term-1")
            .is_some_and(crate::cockpit::Term::is_following)
    );
    update(&mut m, notch(MouseKind::ScrollUp, index));
    assert!(
        !m.term("term-1")
            .is_some_and(crate::cockpit::Term::is_following)
    );
    assert_eq!(update(&mut m, key(KeyCode::Char('f'))), vec![]);
    assert!(
        m.term("term-1")
            .is_some_and(crate::cockpit::Term::is_following)
    );
    assert!(m.prompt().is_empty(), "f was the pane's, not the prompt's");
    // With nothing scrolled, f is a letter for the prompt.
    update(&mut m, key(KeyCode::Char('f')));
    assert_eq!(m.prompt().text(), "f");
}

// ── The rail's verbs on the window ─────────

/// A checkpoint row's Restore and Fork — revealed on the window's rail
///  — reach the port through the same commands the
/// terminal's `/restore` and `/fork` send, and never for a row the
/// spine does not hold.
#[test]
fn a_rows_restore_and_fork_run_the_spine_port_and_refuse_an_unheld_row() {
    let mut m = model();
    update(&mut m, Msg::Agent(risky(1, "a")));
    update(&mut m, Msg::Agent(risky(2, "b")));
    assert_eq!(
        update(&mut m, click(Widget::spine_restore(1))),
        vec![Cmd::Checkpoint(CheckpointCmd::Restore {
            id: "cp-2".to_owned()
        })]
    );
    assert_eq!(
        update(&mut m, click(Widget::spine_fork(0))),
        vec![Cmd::Checkpoint(CheckpointCmd::Fork {
            id: "cp-1".to_owned(),
            branch: None,
        })]
    );
    assert_eq!(
        update(&mut m, click(Widget::spine_restore(7))),
        vec![],
        "the port is never asked about a state that was never on the rail"
    );
    assert_eq!(update(&mut m, click(Widget::spine_fork(7))), vec![]);
}

/// On a window a row's Preview opens the Changes reading: the camera
/// turns to Changes with the row previewed, and folding the preview
/// back holds the camera. A terminal's click previews and stays — its
/// rail is unchanged.
#[test]
fn previewing_a_row_on_a_window_turns_the_camera_to_changes() {
    let mut w = model().with_surface(Surface::Native);
    let id = w
        .rail_mut()
        .spine_mut()
        .open(CheckpointReason::Manual, "start");
    w.rail_mut().spine_mut().saved(&id, None, true);
    let cmds = update(&mut w, click(Widget::spine_row(0)));
    assert!(
        cmds.iter().all(|cmd| matches!(cmd, Cmd::Animate(_))),
        "the camera's own transition, nothing for the port: {cmds:?}"
    );
    assert_eq!(w.place(), Some(Place::Changes));
    assert_eq!(
        w.rail()
            .spine()
            .previewed()
            .map(crate::cockpit::Checkpoint::id),
        Some("cp-1")
    );
    update(&mut w, click(Widget::spine_row(0)));
    assert!(
        w.rail().spine().previewed().is_none(),
        "the same click folds"
    );
    assert_eq!(w.place(), Some(Place::Changes), "and holds the camera");

    let mut t = model();
    let id = t
        .rail_mut()
        .spine_mut()
        .open(CheckpointReason::Manual, "start");
    t.rail_mut().spine_mut().saved(&id, None, true);
    update(&mut t, click(Widget::spine_row(0)));
    assert_eq!(t.tab(), StageTab::Turn, "a terminal's stage stays");
    assert!(t.rail().spine().previewed().is_some());
}

/// The meter's Raise opens settings at Budget & limits
/// — the bad day's `Increase limit`, reachable from the rail.
#[test]
fn the_meters_raise_opens_settings_at_budget() {
    let mut w = model().with_surface(Surface::Native);
    assert_eq!(update(&mut w, click(Widget::BudgetRaise.id())), vec![]);
    assert_eq!(
        w.settings().map(crate::cockpit::Settings::group),
        Some(crate::cockpit::SettingsGroup::Budget)
    );
}

/// A previewed checkpoint is what the review reads since — when the
/// port confirmed it; an unverified preview scopes nothing new, so the
/// reading falls back to the newest verified state as before.
#[test]
fn a_previewed_verified_checkpoint_scopes_the_review() {
    let mut m = model();
    {
        let spine = m.rail_mut().spine_mut();
        let a = spine.open(CheckpointReason::Manual, "a");
        spine.saved(&a, None, true);
        let b = spine.open(CheckpointReason::Manual, "b");
        spine.saved(&b, None, true);
        spine.open(CheckpointReason::Manual, "c");
    }
    assert_eq!(m.review_since(), Some("cp-2"), "the newest verified");
    update(&mut m, click(Widget::spine_row(0)));
    assert_eq!(m.review_since(), Some("cp-1"), "the previewed row");
    update(&mut m, click(Widget::spine_row(2)));
    assert_eq!(
        m.review_since(),
        Some("cp-2"),
        "an unverified preview is not a state to read since"
    );
}

// ── The terminal dock ─────────────────────────────

/// A running run card whose terminal has streamed thirty rows, open on
/// the stage of a `size` cockpit — the dock's subject.
fn pane_at(size: Size) -> Model {
    use magistr_application::sessions::{ToolRecord, ToolStatus, ToolVerb};
    let mut m = Model::new(size);
    agent(
        &mut m,
        AgentEvent::ToolCall {
            id: "c1".to_owned(),
            title: "cargo test".to_owned(),
            verb: ToolVerb::Run,
            status: ToolStatus::InProgress,
            diff: None,
            images: Vec::new(),
            record: Box::new(ToolRecord {
                terminal: Some("term-1".to_owned()),
                ..ToolRecord::default()
            }),
        },
    );
    let lines: String = (1..=30).map(|i| format!("line {i}\r\n")).collect();
    agent(
        &mut m,
        AgentEvent::Terminal {
            terminal_id: "term-1".to_owned(),
            bytes: lines.into_bytes(),
        },
    );
    update(&mut m, key(KeyCode::Enter));
    settle(&mut m);
    m
}

fn cmd_enter() -> Msg {
    Msg::Key(KeyEvent::new(KeyCode::Enter).with(Modifiers::SUPER))
}

fn cmd_shift_enter() -> Msg {
    Msg::Key(KeyEvent::new(KeyCode::Enter).with(Modifiers::SUPER | Modifiers::SHIFT))
}

fn ctrl_t() -> Msg {
    Msg::Key(KeyEvent::new(KeyCode::Char('t')).with(Modifiers::CTRL))
}

/// ⌘↵ on a window raises the running card's screen into the dock — a
/// third of the stage the first time — rising over the glide's steps
/// and taking its rows off the stage once up, the feed, the prompt and
/// the place untouched; ⌘↵ again lowers it, and the card's window is a
/// card's again.
#[test]
fn cmd_enter_raises_the_running_cards_pane_a_third_of_the_stage_and_again_lowers_it() {
    let mut w = pane_at(Size::new(120, 60)).with_surface(crate::cockpit::Surface::Native);
    let stage = w.stage_rows();
    let third = w.dock_first_rows();
    assert_eq!(usize::from(third), stage / 3, "a third of the stage");
    assert!(
        third > TermDock::MIN_ROWS,
        "tall enough for the third to show"
    );
    assert_eq!(update(&mut w, cmd_enter()), vec![]);
    assert!(w.term_dock().is_up());
    assert_eq!(w.term_dock().id(), Some("term-1"));
    assert_eq!(w.term_dock().rows(), third);
    assert!(w.animates(Animation::Motion), "the rise holds the tick");
    assert!(w.motions_in_flight().contains(&MotionName::Rise));
    assert_eq!(
        w.docked_term(),
        None,
        "the first frame is the edge: nothing of the screen shows yet"
    );
    assert_eq!(w.stage_rows(), stage, "and the stage is still whole");
    update(&mut w, Msg::Tick);
    let rising = w.docked_term().map(|(_, rows)| rows).expect("rising");
    assert!(rising > 0 && rising < usize::from(third), "{rising}");
    settle(&mut w);
    assert_eq!(
        w.docked_term().map(|(_, rows)| rows),
        Some(usize::from(third))
    );
    assert_eq!(
        w.stage_rows(),
        stage - usize::from(third) - 1,
        "the rows and the grip row"
    );
    assert_eq!(
        w.term("term-1").map(Term::window),
        Some(usize::from(third)),
        "the docked screen's window is the dock's"
    );
    assert!(w.prompt().is_empty(), "⌘↵ never reached the prompt");
    assert_eq!(w.place(), Some(crate::cockpit::Place::Session));
    assert!(!w.rail().is_collapsed());
    assert_eq!(update(&mut w, cmd_enter()), vec![]);
    assert!(!w.term_dock().is_up());
    assert!(w.docked_term().is_some(), "still drawn while it lowers");
    settle(&mut w);
    assert!(w.docked_term().is_none());
    assert_eq!(w.stage_rows(), stage);
    assert_eq!(w.term("term-1").map(Term::window), Some(Term::PANE_ROWS));
}

/// ⌘⇧↵ maximizes — raising first when the dock is down — to the stage
/// less the grip, capped at the screen's rows; the rail stays and the
/// dragged height is kept, so ⌘⇧↵ again is the way back to it.
#[test]
fn cmd_shift_enter_maximizes_with_the_rail_staying_and_again_restores() {
    let mut w = pane_at(Size::new(120, 60)).with_surface(crate::cockpit::Surface::Native);
    let max = w.dock_max_rows();
    assert_eq!(
        max,
        TermDock::MAX_ROWS,
        "a tall stage: the screen's own rows"
    );
    assert_eq!(update(&mut w, cmd_shift_enter()), vec![]);
    assert!(w.term_dock().is_up() && w.term_dock().is_maximized());
    assert_eq!(w.term_dock().target_rows(), max);
    assert!(!w.rail().is_collapsed(), "the rail stays");
    settle(&mut w);
    assert_eq!(
        w.docked_term().map(|(_, rows)| rows),
        Some(usize::from(max))
    );
    assert_eq!(w.term("term-1").map(Term::window), Some(usize::from(max)));
    assert_eq!(update(&mut w, cmd_shift_enter()), vec![]);
    assert!(w.term_dock().is_up() && !w.term_dock().is_maximized());
    assert_eq!(w.term_dock().target_rows(), w.dock_first_rows());
    // A short stage maximizes to what it has.
    let mut short = pane_at(Size::new(80, 24)).with_surface(crate::cockpit::Surface::Native);
    let max = short.dock_max_rows();
    assert!(max < TermDock::MAX_ROWS, "{max}");
    update(&mut short, cmd_shift_enter());
    settle(&mut short);
    assert_eq!(
        short.docked_term().map(|(_, rows)| rows),
        Some(usize::from(max))
    );
}

/// On a terminal `ctrl-t` raises and lowers, and Esc lowers a raised
/// dock ahead of every other meaning; the chords are the window's and
/// a terminal's Enter still sends.
#[test]
fn ctrl_t_docks_on_a_terminal_and_esc_lowers_it() {
    let mut m = pane_at(Size::new(120, 40));
    assert_eq!(update(&mut m, ctrl_t()), vec![]);
    assert!(m.term_dock().is_up());
    assert_eq!(update(&mut m, key(KeyCode::Esc)), vec![]);
    assert!(!m.term_dock().is_up(), "Esc lowers it");
    assert_eq!(update(&mut m, ctrl_t()), vec![]);
    assert_eq!(update(&mut m, ctrl_t()), vec![]);
    assert!(!m.term_dock().is_up(), "ctrl-t again lowers it");
    // The chords are the window's: a terminal's ⌘↵ is a plain Enter,
    // whatever that does to the prompt (a streaming turn queues it).
    for c in "hi".chars() {
        update(&mut m, key(KeyCode::Char(c)));
    }
    let mut plain = m.clone();
    let expected = update(&mut plain, key(KeyCode::Enter));
    assert_eq!(update(&mut m, cmd_enter()), expected);
    assert_eq!(m.prompt().text(), plain.prompt().text());
    assert!(!m.term_dock().is_up());
    assert!(!plain.term_dock().is_up());
}

/// Nothing docks without a card that ran on a terminal that wrote
/// something: the chord is spent and the prompt is untouched.
#[test]
fn nothing_docks_without_a_running_terminal() {
    let mut w = model().with_surface(crate::cockpit::Surface::Native);
    assert_eq!(update(&mut w, cmd_enter()), vec![]);
    assert!(!w.term_dock().is_up());
    assert_eq!(update(&mut w, cmd_shift_enter()), vec![]);
    assert!(!w.term_dock().is_up());
    let mut t = model();
    assert_eq!(update(&mut t, ctrl_t()), vec![]);
    assert!(!t.term_dock().is_up());
    assert!(t.prompt().is_empty());
}

/// The grip drag: the press remembers where it began, every drag sizes
/// the dock by the rows the pointer moved (and the docked screen's
/// window with it), and the release is the height this project
/// remembers — the next rise takes it instead of the third.
#[test]
fn a_grip_drag_sizes_the_dock_and_the_release_is_remembered_per_project() {
    let mut m = pane_at(Size::new(120, 60));
    update(&mut m, ctrl_t());
    settle(&mut m);
    let third = m.term_dock().rows();
    let at = |kind, row| Msg::Mouse {
        event: MouseEvent::new(kind, 10, row),
        target: Some(Widget::TermDockGrip.id()),
    };
    update(&mut m, at(MouseKind::Down(Button::Left), 30));
    assert!(m.term_dock().is_dragging());
    update(&mut m, notch(MouseKind::Drag(Button::Left), 10, 27));
    assert_eq!(
        m.term_dock().rows(),
        third + 3,
        "three rows up is three rows taller"
    );
    assert_eq!(
        m.term("term-1").map(Term::window),
        Some(usize::from(third + 3))
    );
    assert_eq!(
        m.docked_term().map(|(_, rows)| rows),
        Some(usize::from(third + 3))
    );
    update(&mut m, notch(MouseKind::Drag(Button::Left), 10, 32));
    assert_eq!(m.term_dock().rows(), third - 2);
    update(&mut m, notch(MouseKind::Up(Button::Left), 10, 32));
    assert!(!m.term_dock().is_dragging());
    assert_eq!(m.settings_data().dock_rows, Some(third - 2), "remembered");
    update(&mut m, key(KeyCode::Esc));
    settle(&mut m);
    update(&mut m, ctrl_t());
    assert_eq!(
        m.term_dock().rows(),
        third - 2,
        "the next rise is the remembered one"
    );
    // A remembered height seeds a fresh cockpit's first rise too.
    let mut seeded = pane_at(Size::new(120, 60)).with_settings(crate::cockpit::SettingsData {
        dock_rows: Some(11),
        ..crate::cockpit::SettingsData::default()
    });
    update(&mut seeded, ctrl_t());
    assert_eq!(seeded.term_dock().rows(), 11);
}

/// The docked pane follows and stops by the card's rule: a notch over
/// the dock scrolls the docked screen (never the stage), the rows that
/// arrive under it are counted for the pill, and `f` returns it.
#[test]
fn a_notch_over_the_dock_scrolls_the_docked_pane_and_f_follows() {
    let mut m = pane_at(Size::new(120, 60));
    update(&mut m, ctrl_t());
    settle(&mut m);
    let top = m.top_row();
    let over_dock = |kind| Msg::Mouse {
        event: MouseEvent::new(kind, 10, 40),
        target: Some(Widget::TermDock.id()),
    };
    update(&mut m, over_dock(MouseKind::ScrollUp));
    assert!(!m.term("term-1").is_some_and(Term::is_following), "held");
    assert_eq!(m.top_row(), top, "the stage did not move");
    agent(
        &mut m,
        AgentEvent::Terminal {
            terminal_id: "term-1".to_owned(),
            bytes: b"more\r\nrows\r\n".to_vec(),
        },
    );
    assert_eq!(
        m.term("term-1").map(Term::arrived),
        Some(2),
        "the pill's count"
    );
    // The two rows that arrived anchored the window two further up, so
    // one notch down lands short of the tail and a second reaches it —
    // the card's own rule.
    update(&mut m, over_dock(MouseKind::ScrollDown));
    assert!(!m.term("term-1").is_some_and(Term::is_following));
    update(&mut m, over_dock(MouseKind::ScrollDown));
    assert!(
        m.term("term-1").is_some_and(Term::is_following),
        "back to the tail"
    );
    update(&mut m, over_dock(MouseKind::ScrollUp));
    assert_eq!(update(&mut m, key(KeyCode::Char('f'))), vec![]);
    assert!(m.term("term-1").is_some_and(Term::is_following));
    assert!(m.prompt().is_empty(), "f was the pane's");
}

/// Under reduced motion the dock lands at once and names no motion.
#[test]
fn under_reduced_motion_the_dock_lands_at_once() {
    let mut m = pane_at(Size::new(120, 60))
        .with_theme(Theme::detect(None, None).with_motion(MotionMode::Reduced));
    update(&mut m, ctrl_t());
    assert_eq!(
        m.docked_term().map(|(_, rows)| rows),
        Some(usize::from(m.term_dock().rows()))
    );
    assert!(!m.animates(Animation::Motion));
    assert!(m.motions_in_flight().is_empty());
    update(&mut m, key(KeyCode::Esc));
    assert!(m.docked_term().is_none());
}

/// The usage update's size half is kept beside the level:
/// the headroom is what the window has
/// left, `None` until the agent reported usage at all, and never a
/// wrap when the level runs past the window.
#[test]
fn the_context_headroom_is_the_window_less_the_level() {
    let mut m = model();
    assert_eq!(m.token_headroom(), None);
    let usage = |used| {
        Msg::Agent(AgentEvent::Usage {
            used,
            size: 200_000,
            cost_cents: None,
            elapsed_secs: None,
        })
    };
    update(&mut m, usage(38_204));
    assert_eq!(m.token_headroom(), Some(161_796));
    assert_eq!(m.token_level(), Some(38_204));
    update(&mut m, usage(210_000));
    assert_eq!(
        m.token_headroom(),
        Some(0),
        "a level past the window reads no headroom, not a wrap"
    );
}

/// The identity band's chunk chip: a click
/// rises to the Sky with the chunk's card framed — the roadmap's
/// selection — and never through the open-chunk fold, which would
/// route an executing chunk the rail names straight back to the
/// Session. With no chunk on the rail the chip is not drawn, and a
/// stray click holds the camera.
#[test]
fn the_chunk_chip_rises_to_the_sky_with_the_card_framed() {
    use crate::cockpit::Place;
    let mut m = model().with_surface(crate::cockpit::Surface::Native);
    m.place = Place::Session;
    let _ = update(&mut m, click(Widget::SessionChunk.id()));
    assert_eq!(
        m.place(),
        Some(Place::Session),
        "no chunk on the rail: the camera holds"
    );
    update(
        &mut m,
        Msg::Engine(EngineEvent::Seed(
            magistr_workflow_contracts::NativeSnapshot {
                chunks: crate::harness::fixtures::chunks_from(vec![serde_json::json!({
                    "id": "gui2-2a",
                    "title": "The design language",
                    "status": "in_progress",
                    "priority": 100
                })]),
                ..magistr_workflow_contracts::NativeSnapshot::default()
            },
        )),
    );
    let _ = update(&mut m, click(Widget::SessionChunk.id()));
    assert_eq!(m.place(), Some(Place::Sky), "the chip rises to the Sky");
    assert_eq!(
        m.roadmap().selected().map(|chunk| chunk.id.as_str()),
        Some("gui2-2a"),
        "with the chunk's card framed"
    );
}

// ── the held kill and the transcript copy ──────────

/// A pane with the terminal reporting key releases (the kitty keyboard
/// protocol's event types), so `k` may be held.
fn pane_holdable(size: Size) -> Model {
    let mut m = pane_at(size);
    update(&mut m, Msg::KeyReleases(true));
    m
}

fn kill_release() -> Msg {
    Msg::Key(KeyEvent::new(KeyCode::Char('k')).kind(KeyKind::Release))
}

/// Ticks until the hold is done, with the fill read along the way.
fn hold_out(m: &mut Model) -> Vec<Cmd> {
    let mut cmds = Vec::new();
    for _ in 0..12 {
        cmds.extend(update(m, Msg::Tick));
        if m.term_hold().is_none() {
            break;
        }
    }
    cmds
}

/// `k` on a terminal that never said it reports releases is the
/// prompt's letter: a hold nobody could let go of would complete on the
/// timer. Once the terminal answers with event types, the same key
/// arms the hold and types nothing.
#[test]
fn k_arms_a_hold_only_where_the_key_can_be_let_go_of() {
    let mut mute = pane_at(Size::new(80, 24));
    assert!(mute.kill_target().is_some(), "the run is killable");
    assert_eq!(update(&mut mute, key(KeyCode::Char('k'))), vec![]);
    assert!(mute.term_hold().is_none());
    assert_eq!(mute.prompt().text(), "k", "the letter stays the prompt's");

    let mut m = pane_holdable(Size::new(80, 24));
    let cmds = update(&mut m, key(KeyCode::Char('k')));
    assert!(m.term_hold().is_some(), "armed");
    assert!(m.prompt().is_empty(), "nothing typed");
    // The streaming run already holds the tick; the hold rides it —
    // the reduced-motion test below is where the hold takes it alone.
    assert_eq!(cmds, vec![]);
    assert!(m.animates(Animation::Motion), "the hold has its tick");
    assert_eq!(m.hold_fraction("term-1"), Some(0.0), "the dead zone first");
    assert_eq!(
        m.hold_fraction("term-9"),
        None,
        "another program's head is untouched"
    );

    let mut w = pane_at(Size::new(120, 40)).with_surface(Surface::Native);
    update(&mut w, key(KeyCode::Char('k')));
    assert!(w.term_hold().is_some(), "a window always reports releases");
}

/// The head fills by the ticks after the dead zone, and the key coming
/// up before the half second leaves the program running: no kill goes
/// out, the screen is untouched, and the tick is given back.
#[test]
fn a_key_hold_fills_by_the_ticks_and_letting_go_early_does_nothing() {
    let mut m = pane_holdable(Size::new(80, 24));
    update(&mut m, key(KeyCode::Char('k')));
    for _ in 0..4 {
        assert_eq!(update(&mut m, Msg::Tick), vec![]);
    }
    let fraction = m.hold_fraction("term-1").expect("held");
    assert!(fraction > 0.0 && fraction < 1.0, "{fraction}");
    let head = m
        .turn()
        .rows()
        .iter()
        .position(|r| r.kind == RowKind::Tool(ToolStatus::InProgress))
        .expect("the run's head");
    assert_eq!(
        m.hold_at_row(head),
        Some(fraction),
        "the head's row wears the fill"
    );
    assert_eq!(m.hold_at_row(head + 1), None);
    let cmds = update(&mut m, kill_release());
    assert!(m.term_hold().is_none(), "let go");
    assert_eq!(cmds, vec![], "nothing happens: {cmds:?}");
    assert!(!m.term("term-1").is_some_and(Term::is_killed));
    assert_eq!(m.hold_at_row(head), None, "the head is its own again");
}

/// Held the whole half second, the hold kills: the screen is marked the
/// reader's doing and frozen, the card keeps a strip row for the word,
/// the kill goes to the producer once, and the receipt the call then
/// earns says so. A second `k` after that arms nothing — the program
/// ended.
#[test]
fn a_completed_hold_kills_the_screen_sends_the_kill_once_and_names_the_receipt() {
    let mut m = pane_holdable(Size::new(80, 24));
    update(&mut m, key(KeyCode::Char('k')));
    let cmds = hold_out(&mut m);
    assert_eq!(
        cmds.iter().filter(|c| matches!(c, Cmd::Kill(_))).count(),
        1,
        "{cmds:?}"
    );
    assert_eq!(
        cmds,
        vec![Cmd::Kill(KillCmd::Terminal("term-1".to_owned()))]
    );
    let term = m.term("term-1").expect("the screen");
    assert!(term.is_killed());
    assert!(term.is_finished(), "frozen");
    assert!(
        m.turn().has_strip("term-1"),
        "the strip row carries the word"
    );
    assert_eq!(m.kill_target(), None, "nothing left to kill");
    assert_eq!(update(&mut m, key(KeyCode::Char('k'))), vec![]);
    assert!(m.term_hold().is_none());
    assert_eq!(
        m.prompt().text(),
        "k",
        "with nothing to kill the letter is the prompt's"
    );
    update(&mut m, Msg::Key(KeyEvent::new(KeyCode::Backspace)));
    // The release the kill left behind changes nothing.
    assert_eq!(update(&mut m, kill_release()), vec![]);
    // The agent reports the call ended — the receipt names the human.
    agent(
        &mut m,
        AgentEvent::ToolCallUpdate {
            id: "c1".to_owned(),
            title: None,
            status: Some(ToolStatus::Completed),
            diff: None,
            duration_ms: Some(1200),
            images: Vec::new(),
            record: Box::default(),
        },
    );
    let receipt = m.turn().receipt("c1").expect("minted");
    assert!(receipt.killed);
    assert!(
        m.turn().has_strip("term-1"),
        "the word stays after the call ends"
    );
}

/// A press on the running card's head is the cap: it folds the card as
/// it always did and holds the kill from the press; the button coming
/// up before the half second cancels it. A press anywhere else holds
/// nothing, and the key's release never ends a cap hold.
#[test]
fn a_press_on_the_head_holds_the_kill_and_the_button_coming_up_lets_go() {
    let mut m = pane_holdable(Size::new(120, 40));
    let head = m
        .turn()
        .rows()
        .iter()
        .position(|r| r.kind == RowKind::Tool(ToolStatus::InProgress))
        .expect("the run's head")
        - m.top_row();
    let on = |kind, index| Msg::Mouse {
        event: MouseEvent::new(kind, 10, 5),
        target: Some(Widget::stage_row(index)),
    };
    let open = m.turn().len();
    update(&mut m, on(MouseKind::Down(Button::Left), head));
    assert!(
        m.term_hold().is_some_and(TermHold::is_by_cap),
        "the cap holds"
    );
    assert!(
        m.turn().len() < open,
        "and the press folded the card as before"
    );
    update(&mut m, kill_release());
    assert!(
        m.term_hold().is_some(),
        "the key's release is not the cap's"
    );
    update(&mut m, on(MouseKind::Up(Button::Left), head));
    assert!(m.term_hold().is_none(), "the button came up");
    assert!(!m.term("term-1").is_some_and(Term::is_killed));
    // A press on a row that is not the head holds nothing.
    update(&mut m, on(MouseKind::Down(Button::Left), head + 1));
    assert!(m.term_hold().is_none());
    // Held through the half second, the cap kills.
    update(&mut m, on(MouseKind::Down(Button::Left), head));
    let cmds = hold_out(&mut m);
    assert!(cmds.contains(&Cmd::Kill(KillCmd::Terminal("term-1".to_owned()))));
    assert!(m.term("term-1").is_some_and(Term::is_killed));
}

/// A hold on a program that ends on its own is dropped silently: no
/// kill goes out for a run that is already over.
#[test]
fn a_hold_drops_when_the_program_ends_on_its_own() {
    let mut m = pane_holdable(Size::new(80, 24));
    update(&mut m, key(KeyCode::Char('k')));
    update(&mut m, Msg::Tick);
    agent(
        &mut m,
        AgentEvent::ToolCallUpdate {
            id: "c1".to_owned(),
            title: None,
            status: Some(ToolStatus::Completed),
            diff: None,
            duration_ms: Some(300),
            images: Vec::new(),
            record: Box::default(),
        },
    );
    let cmds = hold_out(&mut m);
    assert!(!cmds.iter().any(|c| matches!(c, Cmd::Kill(_))), "{cmds:?}");
    assert!(m.term_hold().is_none());
    assert!(!m.term("term-1").is_some_and(Term::is_killed));
    assert!(!m.turn().receipt("c1").is_some_and(|r| r.killed));
}

/// Reduced motion keeps the tick for the hold alone — a timer the
/// reader's finger drives is not decoration — and gives it back the
/// moment the hold ends.
#[test]
fn reduced_motion_keeps_the_tick_for_the_hold_alone() {
    let mut m = pane_holdable(Size::new(80, 24))
        .with_theme(Theme::default().with_motion(MotionMode::Reduced));
    assert!(
        m.animations().is_empty(),
        "a quiet pane under reduced motion"
    );
    let cmds = update(&mut m, key(KeyCode::Char('k')));
    assert!(
        cmds.iter().any(|c| matches!(c, Cmd::Animate(_))),
        "{cmds:?}"
    );
    assert!(m.animates(Animation::Motion));
    let cmds = update(&mut m, kill_release());
    assert!(cmds.contains(&Cmd::Still), "{cmds:?}");
    assert!(m.animations().is_empty());
}

/// `y` on an empty prompt puts the run's stripped transcript on the
/// clipboard — the rows the program wrote, the escapes gone — and
/// types nothing; with text in the prompt it is the prompt's letter.
#[test]
fn y_copies_the_running_cards_stripped_transcript() {
    let mut m = pane_at(Size::new(80, 24));
    let cmds = update(&mut m, key(KeyCode::Char('y')));
    let [Cmd::Clipboard(text)] = cmds.as_slice() else {
        panic!("{cmds:?}");
    };
    assert!(text.starts_with("line 1\nline 2\n"), "{text:?}");
    assert!(text.ends_with("line 30"), "{text:?}");
    assert!(m.prompt().is_empty());
    update(&mut m, key(KeyCode::Char('a')));
    assert_eq!(update(&mut m, key(KeyCode::Char('y'))), vec![]);
    assert_eq!(m.prompt().text(), "ay");
    let mut quiet = model();
    assert_eq!(update(&mut quiet, key(KeyCode::Char('y'))), vec![]);
    assert_eq!(quiet.prompt().text(), "y", "no run, no transcript");
}

/// No key reaches the pty: typing over
/// an open pane edits the prompt and sends nothing at all, and the
/// screen's bytes are exactly what the program wrote. (`Cmd` carries
/// no terminal-input variant, so this is also true by construction.)
#[test]
fn no_key_reaches_the_pty() {
    let mut m = pane_holdable(Size::new(80, 24));
    let before = m.terminal_output("term-1").map(<[u8]>::to_vec);
    let mut sent = Vec::new();
    for code in [
        KeyCode::Char('l'),
        KeyCode::Char('s'),
        KeyCode::Char(' '),
        KeyCode::Backspace,
        KeyCode::Char('q'),
        KeyCode::Up,
        KeyCode::Down,
        KeyCode::Tab,
    ] {
        sent.extend(update(&mut m, key(code)));
    }
    sent.extend(update(&mut m, Msg::Paste("\x03".to_owned())));
    sent.extend(update(&mut m, ctrl('c')));
    assert!(
        sent.iter()
            .all(|c| matches!(c, Cmd::Cancel | Cmd::Animate(_) | Cmd::Still)),
        "nothing typed reached a producer as input: {sent:?}"
    );
    assert_eq!(m.terminal_output("term-1").map(<[u8]>::to_vec), before);
}

// ── The stage's scroll anchor (stage-scroll-anchor-and-rate) ─────────

/// A long turn with cards around the window — a run card near the head
/// (A), a check card with an eight-row body a few hundred rows in (B),
/// a run card a hundred rows short of the tail (C) — prose between,
/// on a window (whose Esc folds the newest open card). Built through
/// `update`, so the window is held the way the runtime holds it.
fn anchored_turn() -> Model {
    use magistr_application::sessions::{ToolStatus, ToolVerb};
    let mut m = model().with_surface(Surface::Native);
    let tool = |id: &str, title: &str| AgentEvent::ToolCall {
        id: id.to_owned(),
        title: title.to_owned(),
        verb: ToolVerb::Run,
        status: ToolStatus::Completed,
        diff: None,
        images: Vec::new(),
        record: Default::default(),
    };
    for i in 0..4 {
        agent(&mut m, AgentEvent::Text(format!("line {i}\n")));
    }
    agent(
        &mut m,
        tool(
            "a",
            "cargo check --workspace --all-targets with a title long enough to wrap onto two \
             detail rows at eighty columns",
        ),
    );
    for i in 4..300 {
        agent(&mut m, AgentEvent::Text(format!("line {i}\n")));
    }
    update(
        &mut m,
        Msg::Engine(EngineEvent::local(crate::harness::fixtures::frame(
            "ship",
            "check_recorded",
            serde_json::json!({"check": {
                "name": "test", "passed": true, "exit_code": 0,
                "details": "r1\nr2\nr3\nr4\nr5\nr6\nr7\nr8"
            }}),
        ))),
    );
    for i in 300..600 {
        agent(&mut m, AgentEvent::Text(format!("line {i}\n")));
    }
    agent(&mut m, tool("c", "cargo nextest run"));
    for i in 600..700 {
        agent(&mut m, AgentEvent::Text(format!("line {i}\n")));
    }
    agent(
        &mut m,
        AgentEvent::TurnEnded(magistr_application::sessions::TurnEnd::EndTurn),
    );
    m
}

/// The text on the stage's first row.
fn first_row(m: &Model) -> String {
    m.visible_rows()[0].text.clone()
}

/// A press on the stage row `index` rows under the top — the click
/// that folds or opens the card there.
fn press_row(m: &mut Model, index: usize) {
    use crate::input::{Button, MouseEvent, MouseKind};
    update(
        m,
        Msg::Mouse {
            event: MouseEvent::new(MouseKind::Down(Button::Left), 5, 5),
            target: Some(Widget::stage_row(index)),
        },
    );
    update(
        m,
        Msg::Mouse {
            event: MouseEvent::new(MouseKind::Up(Button::Left), 5, 5),
            target: Some(Widget::stage_row(index)),
        },
    );
}

/// Expanding or collapsing a card above, under or across the top of a
/// pinned window leaves the same content at the top: the window is held
/// to the line it was showing, not to a row index that every re-wrap
/// moves. Across — the window was inside the card's body when the card
/// folded — the head is what is left of that content, and it lands
/// there.
#[test]
fn a_card_toggled_above_under_or_across_the_top_leaves_the_same_row_at_the_top() {
    let mut m = anchored_turn();
    let height = m.stage_rows();
    // Above: C opens and folds above a window pinned two pages short of
    // the tail, through Enter on an empty prompt (the newest card).
    update(&mut m, key(KeyCode::PageUp));
    update(&mut m, key(KeyCode::PageUp));
    settle(&mut m);
    assert!(!m.viewport().is_following());
    let held = first_row(&m);
    assert!(held.starts_with("line 6"), "{held}");
    let rows = m.turn().len();
    update(&mut m, key(KeyCode::Enter));
    assert!(m.turn().len() > rows, "C opened above the window");
    assert_eq!(first_row(&m), held, "a card opening above the window");
    assert_eq!(m.visible_rows().len(), height);
    update(&mut m, key(KeyCode::Enter));
    assert_eq!(m.turn().len(), rows, "C folded again");
    assert_eq!(first_row(&m), held, "a card folding above the window");

    // Under: from the head, the same card opens and folds far below.
    update(&mut m, key(KeyCode::Home));
    settle(&mut m);
    assert_eq!(m.top_row(), 0);
    assert!(!m.viewport().is_following());
    update(&mut m, key(KeyCode::Enter));
    assert_eq!(first_row(&m), "line 0", "a card opening under the window");
    update(&mut m, key(KeyCode::Enter));
    assert_eq!(first_row(&m), "line 0", "a card folding under the window");

    // Across: B opens with the window on it, the wheel carries the top
    // into its body, and Esc folds it under the window — the head is
    // what is left of that content.
    let b = m
        .turn()
        .lines()
        .iter()
        .position(|line| matches!(line.kind(), crate::turn::LineKind::Check { .. }))
        .expect("the check card");
    let b_head = m.turn().lines()[b].row_start();
    let page = height - 1;
    for _ in 0..b_head / page {
        update(&mut m, key(KeyCode::PageDown));
    }
    settle(&mut m);
    let visible = b_head - m.top_row();
    assert!(visible < height, "B's head is on screen at {visible}");
    press_row(&mut m, visible);
    assert_eq!(m.turn().len(), rows + 8, "B opened: eight detail rows");
    assert_eq!(m.top_row(), b_head - visible, "opening under the top holds");
    let wheel = Msg::Mouse {
        event: crate::input::MouseEvent::new(MouseKind::ScrollDown, 5, 5),
        target: Some(Widget::Stage.id()),
    };
    for _ in 0..visible / Viewport::WHEEL + 1 {
        update(&mut m, wheel.clone());
    }
    settle(&mut m);
    let inside = m.top_row() - b_head;
    assert!(
        (1..=8).contains(&inside),
        "the top is in B's body: {inside}"
    );
    assert!(
        first_row(&m).trim_start().starts_with('r'),
        "{}",
        first_row(&m)
    );
    update(&mut m, key(KeyCode::Esc));
    assert_eq!(m.turn().len(), rows, "B folded");
    assert_eq!(m.top_row(), b_head, "the window lands on B's head");
    assert!(first_row(&m).contains("test"), "{}", first_row(&m));
    assert!(!m.viewport().is_following());
}

/// A re-wrap for a new width holds the pinned window on the line it was
/// showing, and rows arriving under it leave it where it is.
#[test]
fn a_resize_re_wrap_and_arriving_rows_hold_the_pinned_window_on_its_line() {
    let mut m = model();
    for i in 0..300 {
        agent(
            &mut m,
            AgentEvent::Text(format!("paragraph {i} holds its place\n")),
        );
    }
    update(&mut m, key(KeyCode::Home));
    for _ in 0..8 {
        update(&mut m, key(KeyCode::PageDown));
    }
    settle(&mut m);
    assert!(!m.viewport().is_following());
    let line = m
        .turn()
        .anchor_at(m.top_row())
        .expect("a line under the top")
        .line();
    assert_eq!(first_row(&m), format!("paragraph {line} holds its place"));
    let width = m.turn().width();
    update(&mut m, Msg::Resize(Size::new(40, 24)));
    assert!(m.turn().width() < width, "the turn re-wrapped narrower");
    assert_eq!(
        m.turn()
            .anchor_at(m.top_row())
            .expect("a line under the top")
            .line(),
        line,
        "the same line under the top"
    );
    assert!(
        first_row(&m).starts_with(&format!("paragraph {line}")),
        "{}",
        first_row(&m)
    );
    assert!(!m.viewport().is_following());
    let held = first_row(&m);
    for i in 300..320 {
        agent(&mut m, AgentEvent::Text(format!("paragraph {i}\n")));
    }
    assert_eq!(first_row(&m), held, "rows arriving under the window");
    // Back to the tail, and the tail-follow rule is what it was.
    update(&mut m, key(KeyCode::End));
    settle(&mut m);
    assert!(m.viewport().is_following());
    update(&mut m, Msg::Resize(Size::new(80, 24)));
    assert!(
        m.viewport().is_following(),
        "following is not a row to hold"
    );
    assert_eq!(
        m.visible_rows().last().map(|row| row.text.as_str()),
        Some("paragraph 319")
    );
}

/// An open card's detail growing while it streams — a running call's
/// title updated above a pinned window — moves its rows and nothing the
/// reader is looking at.
#[test]
fn an_open_cards_detail_growing_above_the_window_holds_it() {
    let mut m = anchored_turn();
    update(&mut m, key(KeyCode::Enter));
    let rows = m.turn().len();
    update(&mut m, key(KeyCode::PageUp));
    update(&mut m, key(KeyCode::PageUp));
    settle(&mut m);
    let held = first_row(&m);
    assert!(held.starts_with("line 6"), "{held}");
    let top = m.top_row();
    agent(
        &mut m,
        AgentEvent::ToolCallUpdate {
            id: "c".to_owned(),
            title: Some(
                "cargo nextest run --workspace --exclude magistr-gui --no-fail-fast with a title \
                 that now wraps onto three detail rows under the head"
                    .to_owned(),
            ),
            status: None,
            diff: None,
            duration_ms: Some(1200),
            images: Vec::new(),
            record: Default::default(),
        },
    );
    assert!(m.turn().len() > rows, "the detail grew");
    assert_eq!(first_row(&m), held, "the window kept its line");
    assert!(
        m.top_row() > top,
        "on a row further down, where the line moved"
    );
    assert!(!m.viewport().is_following());
}

/// A window paints between rows while a notch glides: the fraction is
/// on between frames, one more row is on the stage, and it is exactly
/// zero again once the glide is over. A cell surface never has one.
#[test]
fn a_window_paints_between_rows_while_a_notch_glides_and_a_terminal_never_does() {
    use crate::input::MouseEvent;
    let wheel = Msg::Mouse {
        event: MouseEvent::new(MouseKind::ScrollUp, 5, 5),
        target: Some(Widget::Stage.id()),
    };
    for (native, m) in [
        (true, model().with_surface(Surface::Native)),
        (false, model()),
    ] {
        let mut m = m;
        for i in 0..200 {
            agent(&mut m, AgentEvent::Text(format!("line {i}\n")));
        }
        agent(
            &mut m,
            AgentEvent::TurnEnded(magistr_application::sessions::TurnEnd::EndTurn),
        );
        update(&mut m, key(KeyCode::PageUp));
        settle(&mut m);
        let height = m.stage_rows();
        assert_eq!(m.stage_scroll_fraction(), 0.0);
        assert_eq!(m.visible_rows().len(), height);
        let top = m.top_row();
        update(&mut m, wheel.clone());
        update(&mut m, Msg::Phase(0.5));
        let fraction = m.stage_scroll_fraction();
        if native {
            assert!(fraction > 0.0 && fraction < 1.0, "between rows: {fraction}");
            assert!(m.top_row() < top && m.top_row() >= top - Viewport::WHEEL);
            assert_eq!(m.visible_rows().len(), height + 1, "one more row on");
        } else {
            assert_eq!(fraction, 0.0, "a cell surface paints whole rows");
            assert_eq!(m.top_row(), top - Viewport::WHEEL, "already there");
            assert_eq!(m.visible_rows().len(), height);
        }
        update(&mut m, Msg::Phase(0.0));
        settle(&mut m);
        assert_eq!(m.stage_scroll_fraction(), 0.0);
        assert_eq!(m.top_row(), top - Viewport::WHEEL);
        assert_eq!(m.visible_rows().len(), height);
    }
}

/// A rebuild is deterministic: the same snapshot and the same in-order
/// events, folded into two fresh models, leave the same projection — and
/// a frame delivered twice is folded once, so a duplicate changes nothing.
#[test]
fn folding_the_same_seed_and_events_rebuilds_the_same_projection_and_a_duplicate_changes_nothing() {
    use crate::engine::EngineEvent;
    use crate::harness::fixtures;
    use magistr_workflow_contracts::Delivery;

    let seed = || {
        fixtures::snapshot(
            vec![
                fixtures::chunk_from(serde_json::json!({"id": "a", "status": "in_progress"})),
                fixtures::chunk_from(serde_json::json!({"id": "b", "status": "pending"})),
            ],
            vec![fixtures::signal_from(
                serde_json::json!({"id": "s1", "status": "new"}),
            )],
        )
    };
    let events = || {
        vec![
            (
                fixtures::frame(
                    "ship",
                    "objective_set",
                    serde_json::json!({"objective": {"description": "goal"}}),
                ),
                Delivery::First,
            ),
            (
                fixtures::frame(
                    "roadmap",
                    "chunk_completed",
                    serde_json::json!({"chunk": {"id": "a"}}),
                ),
                Delivery::InOrder,
            ),
            (
                fixtures::frame(
                    "think",
                    "step_appended",
                    serde_json::json!({"step": {"step_number": 3, "purpose": "open"}}),
                ),
                Delivery::InOrder,
            ),
        ]
    };
    let build = || {
        let mut m = model();
        update(&mut m, Msg::Engine(EngineEvent::Seed(seed())));
        for (event, delivery) in events() {
            update(&mut m, Msg::Engine(EngineEvent::Event { event, delivery }));
        }
        m
    };
    let (first, second) = (build(), build());
    assert_eq!(first.rail(), second.rail());
    assert_eq!(first.roadmap(), second.roadmap());
    assert_eq!(first.trace(), second.trace());
    assert_eq!(first.projection(), second.projection());
    assert_eq!(first.turn().rows().len(), second.turn().rows().len());

    let mut again = build();
    let (event, _) = events().remove(1);
    update(
        &mut again,
        Msg::Engine(EngineEvent::Event {
            event,
            delivery: Delivery::Duplicate,
        }),
    );
    assert_eq!(
        again.rail(),
        first.rail(),
        "a duplicate completion is not counted twice"
    );
    assert_eq!(again.roadmap(), first.roadmap());
    assert_eq!(again.turn().rows().len(), first.turn().rows().len());
    assert_eq!(
        again.projection().freshness(),
        first.projection().freshness(),
        "the projection records the duplicate it saw and stays current"
    );
}

#[test]
fn the_arrows_walk_the_trace_selection_and_enter_turns_the_card() {
    let mut m = model().with_tab(StageTab::Session);
    for number in 1..=3u32 {
        update(&mut m, think_step_frame(number, serde_json::json!({})));
    }
    assert_eq!(m.trace().selected_index(), None);
    assert_eq!(update(&mut m, key(KeyCode::Down)), vec![]);
    assert_eq!(
        m.trace().selected_index(),
        Some(0),
        "Down selects the first timeline item"
    );
    update(&mut m, key(KeyCode::Down));
    assert_eq!(m.trace().selected_index(), Some(1));
    assert!(!m.inspector_open());
    update(&mut m, key(KeyCode::Enter));
    assert!(m.inspector_open(), "Enter opens the full card");
    update(&mut m, key(KeyCode::Enter));
    assert!(!m.inspector_open(), "and folds it back to the brief cut");
    assert!(m.prompt().is_empty(), "no Enter reached the prompt");

    // A letter in the prompt gives the keys back: Up is history again.
    update(&mut m, key(KeyCode::Char('h')));
    update(&mut m, key(KeyCode::Up));
    assert_eq!(m.trace().selected_index(), Some(1), "the selection held");

    // Another tab leaves the arrows alone too.
    let mut turn = model();
    update(&mut turn, think_step_frame(1, serde_json::json!({})));
    update(&mut turn, key(KeyCode::Down));
    assert_eq!(turn.trace().selected_index(), None);
}

#[test]
fn a_keyboard_selection_scrolls_its_row_into_the_window() {
    use crate::harness::Scenes;
    let mut m = Scenes::long_trace(Size::new(80, 24));
    update(&mut m, key(KeyCode::Home));
    Scenes::settle(&mut m);
    assert_eq!(m.session_top(), 0);
    update(&mut m, key(KeyCode::Down));
    Scenes::settle(&mut m);
    let (_, height) = m.session_window();
    let row = m.trace().selected_index().expect("the newest step");
    assert!(
        row >= m.session_top() && row < m.session_top() + height,
        "row {row} sits in the window starting at {}",
        m.session_top()
    );
    for _ in 0..25 {
        update(&mut m, key(KeyCode::Up));
    }
    Scenes::settle(&mut m);
    assert_eq!(m.trace().selected_index(), Some(0), "Up stops at the head");
    assert_eq!(m.session_top(), 0, "and the window followed it back up");
}

#[test]
fn the_more_button_turns_the_card_and_a_row_click_preserves_it() {
    let mut m = model().with_tab(StageTab::Session);
    update(&mut m, think_step_frame(4, serde_json::json!({})));
    trace_click(&mut m, Widget::trace_row(0));
    assert!(m.trace().is_expanded(4));
    assert_eq!(trace_click(&mut m, Widget::InspectorDetail.id()), vec![]);
    assert!(m.inspector_open());
    trace_click(&mut m, Widget::trace_row(0));
    assert!(m.trace().is_expanded(4), "the row retains its selection");
    assert!(
        m.inspector_open(),
        "the disclosure is a preference, not part of the selection"
    );
}

#[test]
fn inspector_selection_is_shared_across_views_and_survives_resize() {
    use crate::cockpit::Selected;
    let mut m = model().with_tab(StageTab::Session);
    update(&mut m, think_step_frame(4, serde_json::json!({})));
    trace_click(&mut m, Widget::trace_row(0));
    assert_eq!(m.selected(), Some(&Selected::Step(4)));
    update(&mut m, Msg::Resize(Size::new(132, 42)));
    assert_eq!(m.selected(), Some(&Selected::Step(4)));
    trace_click(&mut m, Widget::TabRoadmap.id());
    assert_eq!(
        m.selected(),
        Some(&Selected::Step(4)),
        "view changes keep the inspector"
    );
    update(
        &mut m,
        Msg::Engine(EngineEvent::Seed(crate::harness::fixtures::snapshot(
            vec![crate::harness::fixtures::chunk_from(
                serde_json::json!({"id":"a","status":"pending"}),
            )],
            vec![],
        ))),
    );
    trace_click(&mut m, Widget::roadmap_row(0));
    assert_eq!(m.selected(), Some(&Selected::Chunk("a".to_owned())));
    update(&mut m, key(KeyCode::Enter));
    assert!(m.inspector_open());
    update(&mut m, key(KeyCode::Esc));
    assert!(!m.inspector_open());
    assert_eq!(
        m.selected(),
        Some(&Selected::Chunk("a".to_owned())),
        "Esc closes disclosure, not selection"
    );
}

#[test]
fn inspector_navigation_scrolls_its_fields_and_keeps_the_list_still() {
    use crate::cockpit::{Pane, Selected};
    let mut m = crate::harness::Scenes::changed(Size::new(80, 24));
    update(&mut m, key(KeyCode::Down));
    assert_eq!(
        m.selected(),
        Some(&Selected::Change(m.changes().entries()[0].identity()))
    );
    update(&mut m, key(KeyCode::Down));
    assert_eq!(
        m.selected(),
        Some(&Selected::Change(m.changes().entries()[1].identity()))
    );
    update(&mut m, key(KeyCode::Up));
    assert_eq!(
        m.selected(),
        Some(&Selected::Change(m.changes().entries()[0].identity()))
    );
    update(&mut m, key(KeyCode::Enter));
    assert!(m.inspector_open());
    update(&mut m, key(KeyCode::Tab));
    assert_eq!(m.pane(), Pane::Inspector);
    let list_top = m.session_top();
    update(&mut m, key(KeyCode::Down));
    assert_eq!(m.inspector_scroll, 1);
    update(
        &mut m,
        Msg::Mouse {
            event: MouseEvent::new(MouseKind::ScrollDown, 35, 16),
            target: Some(Widget::change_approve(0)),
        },
    );
    assert!(
        m.inspector_scroll > 1,
        "wheel over an inspector action scrolls its fields"
    );
    assert_eq!(m.session_top(), list_top);
    update(&mut m, key(KeyCode::Home));
    assert_eq!(m.inspector_scroll, 0);
    update(&mut m, key(KeyCode::End));
    assert!(m.inspector_scroll > 0);
    update(&mut m, key(KeyCode::Esc));
    assert!(!m.inspector_open());
    assert_eq!(m.inspector_scroll, 0);
    update(&mut m, left_click(Widget::TabSession.id()));
    assert_eq!(m.pane(), Pane::Stage);
    update(&mut m, Msg::Paste("draft".into()));
    update(&mut m, key(KeyCode::Down));
    assert_eq!(
        m.selected(),
        Some(&Selected::Change(m.changes().entries()[0].identity()))
    );

    let mut roadmap = crate::harness::Scenes::roadmapped(Size::new(80, 24));
    update(&mut roadmap, key(KeyCode::Down));
    assert_eq!(
        roadmap.selected(),
        Some(&Selected::Chunk("tui2-2-rail".into()))
    );
    update(&mut roadmap, key(KeyCode::Down));
    assert_eq!(
        roadmap.selected(),
        Some(&Selected::Chunk("tui2-3-roadmap".into()))
    );
    update(&mut roadmap, key(KeyCode::Down));
    assert!(matches!(roadmap.selected(), Some(Selected::Signal(_))));
    let selected = roadmap.selected().cloned();
    update(&mut roadmap, key(KeyCode::Down));
    assert_eq!(
        roadmap.selected(),
        selected.as_ref(),
        "selection stops at the list end"
    );
}

#[test]
fn inspector_tab_cycles_panes_and_control_tab_changes_views() {
    use crate::cockpit::Pane;
    let mut m = model();
    assert_eq!(m.pane(), Pane::Stage);
    update(&mut m, key(KeyCode::Tab));
    assert_eq!(m.pane(), Pane::Inspector);
    assert_eq!(m.tab(), StageTab::Turn);
    update(&mut m, key(KeyCode::Tab));
    assert_eq!(m.pane(), Pane::Rail);
    update(&mut m, key(KeyCode::Tab));
    assert_eq!(m.pane(), Pane::Stage);
    update(
        &mut m,
        Msg::Key(KeyEvent::new(KeyCode::Tab).with(Modifiers::CTRL)),
    );
    assert_eq!(m.tab(), StageTab::Session);
    update(
        &mut m,
        Msg::Key(KeyEvent::new(KeyCode::Tab).with(Modifiers::SHIFT)),
    );
    assert_eq!(m.pane(), Pane::Rail, "Shift-Tab reverses the pane cycle");
}
