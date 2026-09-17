use super::*;

fn activity_marks(node: &Node) -> Vec<u16> {
    let mut positions: Vec<_> = node
        .marks()
        .iter()
        .filter_map(|mark| match mark {
            crate::layout::Mark::Indeterminate { position, .. } => Some(*position),
            _ => None,
        })
        .collect();
    for child in node.child_nodes() {
        positions.extend(activity_marks(child));
    }
    positions
}

#[test]
fn active_work_has_one_continuous_indicator_and_reduced_motion_holds_it_still() {
    use crate::cockpit::{Animation, LoopEvent, LoopStage};
    use crate::theme::{MotionMode, Theme, ThemeMode};
    for theme in [ThemeMode::Dark, ThemeMode::Light] {
        let mut model = model(Size::new(144, 50)).with_theme(Theme::forced(theme));
        update(
            &mut model,
            Msg::Loop(LoopEvent::Started {
                iteration: 1,
                max: 4,
            }),
        );
        for stage in [
            LoopStage::Connecting,
            LoopStage::Agent,
            LoopStage::Gates,
            LoopStage::Judge,
        ] {
            update(&mut model, Msg::Loop(LoopEvent::Stage(stage)));
            assert!(model.animates(Animation::Motion), "{stage:?}");
            let s = S::for_model(&model);
            let before = activity_marks(&FlightShell::view(
                &model,
                &FrameClock::new(0, std::time::Duration::ZERO),
            ));
            assert_eq!(before.len(), 1, "one leading indicator");
            let text = status(&model);
            update(&mut model, Msg::Tick);
            update(&mut model, Msg::Phase(0.5));
            let after = activity_marks(&status_label(&model, &s, true));
            assert_ne!(
                before, after,
                "fractional motion continues through {stage:?}"
            );
            assert_eq!(status(&model), text, "animation never rewrites status");
        }
        model.set_theme(Theme::forced(theme).with_motion(MotionMode::Reduced));
        let s = S::for_model(&model);
        assert_eq!(activity_marks(&status_label(&model, &s, true)), vec![500]);
        for _ in 0..10 {
            update(&mut model, Msg::Tick);
        }
        assert_eq!(activity_marks(&status_label(&model, &s, true)), vec![500]);
        assert!(!model.has_activity_motion());
        assert!(!model.is_animating());
    }
}

#[test]
fn pausing_answering_and_finishing_remove_the_activity_indicator() {
    use crate::cockpit::LoopEvent;
    let mut model = model(Size::new(144, 50));
    update(
        &mut model,
        Msg::Loop(LoopEvent::Started {
            iteration: 1,
            max: 4,
        }),
    );
    update(
        &mut model,
        Msg::Loop(LoopEvent::Paused("Review the result".into())),
    );
    let s = S::for_model(&model);
    assert!(!model.has_activity_motion());
    assert!(activity_marks(&status_label(&model, &s, true)).is_empty());
    update(&mut model, Msg::Loop(LoopEvent::Ended("Finished".into())));
    update(
        &mut model,
        Msg::Agent(AgentEvent::UserText("Check recovery".into())),
    );
    assert!(model.has_activity_motion());
    update(
        &mut model,
        Msg::Agent(AgentEvent::PermissionRequest {
            id: 4,
            title: "Run checks".into(),
            choices: vec![PermissionChoice::new("allow", PermissionKind::AllowOnce)],
            edit: false,
            risk: magistr_domain::policy::RiskTier::Low,
            plan: None,
        }),
    );
    assert!(!model.has_activity_motion());
    assert!(activity_marks(&status_label(&model, &s, true)).is_empty());
    click(&mut model, Widget::Allow.id());
    update(
        &mut model,
        Msg::Agent(AgentEvent::TurnEnded(TurnEnd::EndTurn)),
    );
    assert!(!model.has_activity_motion());
    assert!(activity_marks(&status_label(&model, &s, true)).is_empty());
}

use crate::cockpit::{Cmd, Msg, Selected, Size, Surface, update};
use crate::harness::fixtures;
use crate::hit::HitRegistry;
use crate::input::{Button, MouseEvent, MouseKind};
use crate::layout::{Basis, LayoutTree, Resolved};
use crate::pixel::Typography;
use magistr_application::sessions::{AgentEvent, PermissionChoice, PermissionKind, TurnEnd};
use magistr_workflow_contracts::{FeedStatus, NativeSnapshot};
use serde_json::json;

fn model(size: Size) -> Model {
    let mut model = Model::new(size).with_surface(Surface::Terminal)
        .with_tab(StageTab::Turn).with_snapshot(&NativeSnapshot {
            chunks: vec![fixtures::chunk_from(json!({"id":"internal-work","name":"Save the retry schedule","status":"pending"}))],
            steps: fixtures::steps_from(vec![json!({"step_number":17,"purpose":"Keep separate limits","outcome":"Keep separate limits.","execution_ref":"task:internal-task"})]),
            tasks: vec![fixtures::task_from(json!({"id":"internal-task","title":"Save the retry schedule"}))],
            ..NativeSnapshot::default()
        });
    model.pixels = true;
    model.session.prompt.insert("Keep this draft");
    model
}
fn layout(model: &Model) -> Resolved {
    let cell = model.cell_size();
    let mut face = Typography::default();
    face.set_cell(cell);
    LayoutTree::new()
        .compute_on(
            &FlightShell::view(model, &FrameClock::new(0, std::time::Duration::ZERO)),
            f32::from(model.size().cols) * f32::from(cell.width),
            f32::from(model.size().rows) * f32::from(cell.height),
            Basis::new(f32::from(cell.width), f32::from(cell.height)),
            &mut face,
        )
        .unwrap()
}

#[test]
fn roadmap_space_grows_with_the_window() {
    let normal = model(Size::new(132, 42));
    let large = model(Size::new(180, 64));
    assert!(normal.flight_layout().roadmap_height > 148.0);
    assert!(large.flight_layout().roadmap_height > normal.flight_layout().roadmap_height);
    assert!(super::super::flight_roadmap_page::FlightRoadmapPage::graph_height(&normal) >= 240.0);
    assert!(super::super::flight_roadmap_page::FlightRoadmapPage::graph_height(&large) > 280.0);
}
fn click(model: &mut Model, id: crate::widget_id::WidgetId) -> Vec<Cmd> {
    let layout = layout(model);
    let placed = layout
        .placed()
        .iter()
        .find(|p| p.id == Some(id))
        .expect("visible control")
        .px;
    assert!(placed.width > 0.0 && placed.height > 0.0);
    let (x, y) = (
        placed.x + placed.width / 2.0,
        placed.y + placed.height / 2.0,
    );
    let mut hits = HitRegistry::new();
    hits.register_layout_px(&layout, 0);
    let target = hits.hit_at(f64::from(x), f64::from(y));
    assert_eq!(target, Some(id));
    update(
        model,
        Msg::Mouse {
            event: MouseEvent::new(MouseKind::Down(Button::Left), 0, 0).at_px(x as u32, y as u32),
            target,
        },
    )
}

#[test]
fn roadmap_requests_use_visible_targets_and_keep_the_plan_and_draft_stable() {
    for size in [Size::new(144, 50), Size::new(64, 24)] {
        let mut model = model(size);
        click(&mut model, Widget::TabRoadmap.id());
        let top = model.top_row();
        let selected = model.selected().cloned();
        let commands = click(&mut model, Widget::PromptAction.id());
        assert!(
            commands
                .iter()
                .any(|c| matches!(c, Cmd::RoadmapPrompt(text) if text == "Keep this draft"))
        );
        assert!(model.roadmap_requests_open());
        assert!(layout(&model).rect(Widget::RequestBody.id()).is_some());
        assert_eq!(model.top_row(), top);
        assert_eq!(model.selected(), selected.as_ref());
        update(&mut model, Msg::RoadmapRequestStarted);
        update(
            &mut model,
            Msg::Agent(AgentEvent::Text("A recovery check.\n".repeat(100))),
        );
        assert!(model.reveal_conversation(0));
        let top = model.top_row();
        update(&mut model, Msg::Paste("Keep a follow-up draft".into()));
        let before = model.roadmap_request_scroll();
        let node = layout(&model);
        let rect = node
            .placed()
            .iter()
            .find(|p| p.id == Some(Widget::RequestBody.id()))
            .unwrap()
            .px;
        let x = rect.x + 8.0;
        let y = rect.y + 8.0;
        let mut hits = HitRegistry::new();
        hits.register_layout_px(&node, 0);
        let target = hits.hit_at(f64::from(x), f64::from(y));
        assert_eq!(target, Some(Widget::RequestBody.id()));
        update(
            &mut model,
            Msg::Mouse {
                event: MouseEvent::new(MouseKind::ScrollUp, 0, 0).at_px(x as u32, y as u32),
                target,
            },
        );
        assert!(model.roadmap_request_scroll() < before);
        let held = model.roadmap_request_scroll();
        update(
            &mut model,
            Msg::Agent(AgentEvent::Text("Another check.\n".into())),
        );
        assert_eq!(model.roadmap_request_scroll(), held);
        click(&mut model, Widget::RequestLatest.id());
        assert_eq!(model.roadmap_request_scroll(), usize::MAX);
        assert_eq!(model.top_row(), top);
        assert_eq!(model.prompt().text(), "Keep a follow-up draft");
        let commands = click(&mut model, Widget::PromptAction.id());
        assert!(commands.contains(&Cmd::CancelRoadmapPrompt));
        click(&mut model, Widget::RoadmapDetails.id());
        click(&mut model, Widget::RoadmapRequests.id());
        assert_eq!(model.prompt().text(), "Keep a follow-up draft");
    }
}

#[test]
fn reasoning_starts_minimized_and_pointer_toggle_preserves_the_reading_position() {
    for (cols, rows, cw, ch) in [
        (144, 50, 10, 20),
        (128, 40, 10, 20),
        (64, 36, 10, 20),
        (110, 34, 12, 28),
    ] {
        let mut model = model(Size::new(cols, rows));
        update(
            &mut model,
            Msg::CellSize(Some(crate::pixel::CellSize {
                width: cw,
                height: ch,
            })),
        );
        update(
            &mut model,
            Msg::Agent(AgentEvent::Text(
                (0..100)
                    .map(|i| format!("Saved retry check {i} passed.\n"))
                    .collect(),
            )),
        );
        model.inspect(Selected::Step(17));
        model.pane = Pane::Stage;
        assert!(model.reveal_conversation(20));
        let anchor = model.stage_anchor().expect("held reading position");
        let selected = model.selected().cloned();
        let draft = model.prompt().text().to_owned();
        let closed = model.flight_layout();
        let resolved = layout(&model);
        assert!(!model.reasoning_expanded());
        assert_eq!(closed.recorder_height, f32::from(ch) + 16.0);
        assert!(resolved.rect(Widget::ReasoningToggle.id()).is_some());
        assert!(resolved.rect(Widget::TraceGraph.id()).is_none());
        assert!(resolved.rect(Widget::LatestDecision.id()).is_none());
        assert!(click(&mut model, Widget::ReasoningToggle.id()).is_empty());
        assert!(model.reasoning_expanded());
        let opened = model.flight_layout();
        assert!(opened.conversation_height < closed.conversation_height);
        assert!(layout(&model).rect(Widget::TraceGraph.id()).is_some());
        assert_eq!(model.stage_anchor(), Some(anchor));
        assert_eq!(model.selected(), selected.as_ref());
        assert_eq!(model.prompt().text(), draft);
        assert!(click(&mut model, Widget::ReasoningToggle.id()).is_empty());
        assert!(!model.reasoning_expanded());
        assert_eq!(model.stage_anchor(), Some(anchor));
        assert_eq!(model.selected(), selected.as_ref());
        assert_eq!(model.prompt().text(), draft);
        assert_eq!(
            model.flight_layout().conversation_height,
            closed.conversation_height
        );
        assert!(layout(&model).rect(Widget::TraceGraph.id()).is_none());
        update(
            &mut model,
            Msg::Engine(fixtures::event(
                "think",
                "step_appended",
                json!({"step":{"step_number":91,"purpose":"Keep retry limits separate"}}),
            )),
        );
        assert!(
            !model.reasoning_expanded(),
            "new reasoning does not take conversation space"
        );
        assert_eq!(model.selected(), selected.as_ref());
        assert_eq!(model.prompt().text(), draft);
        let node = FlightRecorder::view(&model, closed.left_width);
        fn text(node: &Node) -> String {
            node.text_metrics()
                .map_or_else(String::new, |t| t.text().to_owned())
                + &node.child_nodes().iter().map(text).collect::<String>()
        }
        assert!(text(&node).contains("2 steps"));
        assert!(text(&node).contains("Show"));
    }
}

#[test]
fn focused_reasoning_can_be_toggled_by_keyboard_without_submitting_a_draft() {
    use crate::input::{KeyCode, KeyEvent, Modifiers};
    let mut model = model(Size::new(64, 36));
    let draft = model.prompt().text().to_owned();
    update(
        &mut model,
        Msg::Key(KeyEvent::new(KeyCode::Tab).with(Modifiers::SHIFT)),
    );
    assert_eq!(model.pane(), Pane::Rail);
    assert!(layout(&model).rect(Widget::ReasoningToggle.id()).is_some());
    for expanded in [true, false] {
        let commands = update(&mut model, Msg::Key(KeyEvent::new(KeyCode::Enter)));
        assert!(commands.is_empty());
        assert_eq!(model.reasoning_expanded(), expanded);
        assert_eq!(model.prompt().text(), draft);
        assert_eq!(model.pane(), Pane::Rail);
    }
}

#[test]
fn empty_reasoning_has_a_minimized_heading_and_can_still_be_opened() {
    let mut model = Model::new(Size::new(128, 40)).with_tab(StageTab::Turn);
    update(
        &mut model,
        Msg::Mode(crate::capability::Mode::Graphics {
            pointer_shapes: true,
        }),
    );
    assert!(!model.reasoning_expanded());
    assert!(layout(&model).rect(Widget::ReasoningToggle.id()).is_some());
    click(&mut model, Widget::ReasoningToggle.id());
    assert!(model.reasoning_expanded());
    assert!(layout(&model).rect(Widget::TraceGraph.id()).is_none());
    click(&mut model, Widget::ReasoningToggle.id());
    assert!(!model.reasoning_expanded());
}
#[test]
fn an_early_loop_stop_is_visible_without_inventing_a_started_iteration() {
    use crate::cockpit::LoopEvent;
    for pixels in [false, true] {
        let mut model = model(Size::new(144, 50));
        model.pixels = pixels;
        update(
            &mut model,
            Msg::Loop(LoopEvent::Ended(
                "No work is ready. Open Roadmap to review what is blocked.".into(),
            )),
        );
        assert!(model.loop_status().is_none());
        assert!(!model.is_working());
        assert_eq!(status(&model), "Stopped");
        assert_eq!(model.prompt().text(), "Keep this draft");
        assert!(
            model
                .turn()
                .rows()
                .iter()
                .any(|row| row.text.contains("Work stopped: No work is ready."))
        );
        if pixels {
            assert!(model.turn().conversation().height() > 0);
            let resolved = layout(&model);
            assert!(
                resolved
                    .placed()
                    .iter()
                    .any(|placed| placed.id == Some(Widget::Stage.id()) && placed.px.height > 0.0)
            );
        }
    }
}

#[test]
fn loop_startup_checks_pause_and_completion_have_distinct_visible_status() {
    use crate::cockpit::{LoopEvent, LoopStage};
    let mut model = model(Size::new(144, 50));
    update(
        &mut model,
        Msg::Loop(LoopEvent::Ended("Earlier work stopped.".into())),
    );
    update(
        &mut model,
        Msg::Loop(LoopEvent::Started {
            iteration: 1,
            max: 10,
        }),
    );
    assert_eq!(model.session.notice, None);
    for (stage, label) in [
        (LoopStage::Connecting, "Starting agent"),
        (LoopStage::Agent, "Waiting for response"),
        (LoopStage::Gates, "Checking work"),
        (LoopStage::Judge, "Reviewing work"),
    ] {
        update(&mut model, Msg::Loop(LoopEvent::Stage(stage)));
        assert_eq!(status(&model), label);
    }
    update(
        &mut model,
        Msg::Loop(LoopEvent::Failed("A check failed.".into())),
    );
    update(
        &mut model,
        Msg::Loop(LoopEvent::Paused("A check failed.".into())),
    );
    assert_eq!(status(&model), "Paused");
    update(
        &mut model,
        Msg::Loop(LoopEvent::Failed("The agent could not start.".into())),
    );
    update(
        &mut model,
        Msg::Loop(LoopEvent::Ended("The agent could not start.".into())),
    );
    update(
        &mut model,
        Msg::Engine(crate::engine::EngineEvent::Feed(FeedStatus::Disconnected)),
    );
    assert_eq!(status(&model), "Work failed");
    assert_eq!(model.prompt().text(), "Keep this draft");
    assert!(!model.is_working());
}

#[test]
fn records_and_roadmap_open_details_without_moving_the_conversation_or_draft() {
    let mut model = model(Size::new(144, 50));
    update(
        &mut model,
        Msg::Agent(AgentEvent::Text("Recent work. ".repeat(80))),
    );
    let top = model.top_row();
    click(&mut model, Widget::TabRoadmap.id());
    click(&mut model, Widget::roadmap_row(0));
    assert_eq!(
        model.selected(),
        Some(&Selected::Chunk("internal-work".into()))
    );
    assert_eq!(model.pane(), Pane::Inspector);
    assert_eq!(model.top_row(), top);
    assert_eq!(model.prompt().text(), "Keep this draft");
    click(&mut model, Widget::TabSession.id());
    click(&mut model, Widget::session_row(0));
    assert_eq!(model.selected(), Some(&Selected::Step(17)));
    assert_eq!(model.pane(), Pane::Inspector);
    click(&mut model, Widget::LinkedWork.id());
    assert_eq!(
        model.selected(),
        Some(&Selected::Task("internal-task".into()))
    );
    assert_eq!(model.top_row(), top);
    assert_eq!(model.prompt().text(), "Keep this draft");
}
#[test]
fn a_pending_request_keeps_every_answer_and_the_message_box_inside_the_window() {
    for (cols, rows, cw, ch) in [(144, 50, 10, 20), (64, 36, 10, 20), (110, 34, 12, 28)] {
        let mut model = model(Size::new(cols, rows));
        update(
            &mut model,
            Msg::CellSize(Some(crate::pixel::CellSize {
                width: cw,
                height: ch,
            })),
        );
        update(
            &mut model,
            Msg::Agent(AgentEvent::PermissionRequest {
                id: 45,
                title: "Run the recovery checks after the saved receipt is reloaded. ".repeat(6),
                choices: PermissionKind::ALL
                    .into_iter()
                    .enumerate()
                    .map(|(i, kind)| PermissionChoice::new(i.to_string(), kind))
                    .collect(),
                edit: false,
                risk: magistr_domain::policy::RiskTier::Low,
                plan: None,
            }),
        );
        let resolved = layout(&model);
        let h = f32::from(rows) * f32::from(ch);
        let w = f32::from(cols) * f32::from(cw);
        for id in [
            Widget::Prompt.id(),
            Widget::Allow.id(),
            Widget::Always.id(),
            Widget::Deny.id(),
            Widget::Never.id(),
        ] {
            let p = resolved
                .placed()
                .iter()
                .find(|p| p.id == Some(id))
                .expect("visible answer or draft")
                .px;
            assert!(
                p.width > 1.0 && p.height >= f32::from(ch),
                "{id:?} clipped at {w}x{h}: {p:?}"
            );
            assert!(p.x >= 0.0 && p.y >= 0.0 && p.x + p.width <= w && p.y + p.height <= h);
        }
        let cmds = click(&mut model, Widget::Allow.id());
        assert!(cmds.iter().any(|cmd| matches!(cmd, Cmd::Permission { .. })));
        assert_eq!(model.prompt().text(), "Keep this draft");
    }
}
#[test]
fn a_short_window_starts_with_the_conversation_and_can_open_work() {
    let mut model = model(Size::new(80, 24));
    model.pixels = false;
    update(
        &mut model,
        Msg::Mode(crate::capability::Mode::Graphics {
            pointer_shapes: true,
        }),
    );
    assert!(model.flight_layout().compact);
    assert_eq!(model.pane(), Pane::Stage);
    assert_eq!(model.tab(), StageTab::Turn);
    let initial = layout(&model);
    assert!(initial.rect(Widget::Stage.id()).is_some());
    assert!(initial.rect(Widget::Inspector.id()).is_none());
    model.show(StageTab::Roadmap);
    let opened = layout(&model);
    let row = opened
        .placed()
        .iter()
        .find(|p| p.id == Some(Widget::roadmap_row(0)))
        .unwrap();
    assert!(row.px.height >= crate::cockpit::FlightRecords::row_height(&model));
    click(&mut model, Widget::roadmap_row(0));
    assert_eq!(
        model.selected(),
        Some(&Selected::Chunk("internal-work".into()))
    );
    assert_eq!(model.prompt().text(), "Keep this draft");
}

#[test]
fn starting_actions_fit_completely_in_a_short_conversation() {
    for (cols, rows, cw, ch) in [(80, 24, 10, 20), (80, 24, 12, 28), (64, 30, 10, 20)] {
        let mut model =
            crate::harness::Scenes::launcher(Size::new(cols, rows)).with_tab(StageTab::Turn);
        update(
            &mut model,
            Msg::CellSize(Some(crate::pixel::CellSize {
                width: cw,
                height: ch,
            })),
        );
        update(
            &mut model,
            Msg::Mode(crate::capability::Mode::Graphics {
                pointer_shapes: true,
            }),
        );
        let resolved = layout(&model);
        for id in [Widget::LaunchNext.id(), Widget::LaunchGates.id()] {
            let placed = resolved
                .placed()
                .iter()
                .find(|p| p.id == Some(id))
                .expect("complete starting action");
            assert!(placed.px.height >= f32::from(ch) + 4.0, "{placed:?}");
            assert!(
                placed.px.y + placed.px.height <= model.flight_layout().conversation_height + 52.0
            );
        }
    }
}

#[test]
fn compact_records_and_details_stay_between_header_and_composer() {
    for tab in [StageTab::Session, StageTab::Turn] {
        for (cols, rows, cw, ch) in [(80, 24, 10, 20), (54, 30, 12, 28)] {
            let mut model = model(Size::new(cols, rows));
            update(
                &mut model,
                Msg::CellSize(Some(crate::pixel::CellSize {
                    width: cw,
                    height: ch,
                })),
            );
            model.show(tab);
            if tab == StageTab::Turn {
                model.inspect(Selected::Step(17));
            }
            assert!(model.flight_layout().compact);
            let resolved = layout(&model);
            let body = model.flight_layout();
            let top = 24.0 + f32::from(ch).max(28.0);
            let overlays: Vec<_> = resolved
                .placed()
                .iter()
                .filter(|p| p.layer > 0 && p.px.width > 0.0 && p.px.height > 0.0)
                .collect();
            assert!(!overlays.is_empty());
            for p in overlays {
                assert!(
                    p.px.y >= top && p.px.y + p.px.height <= top + body.body_height + 0.1,
                    "{p:?}"
                );
            }
            assert!(
                top + body.body_height + 12.0 <= body.screen_height - 12.0 - body.composer_height
            );
        }
    }
}

#[test]
fn status_reports_stops_failures_and_stale_connections_from_real_events() {
    let mut model = model(Size::new(144, 50));
    update(
        &mut model,
        Msg::Agent(AgentEvent::TurnEnded(TurnEnd::Cancelled)),
    );
    assert_eq!(status(&model), "Stopped");
    update(
        &mut model,
        Msg::Agent(AgentEvent::Error("Connection closed".into())),
    );
    assert_eq!(status(&model), "Connection error");
    update(
        &mut model,
        Msg::Engine(crate::engine::EngineEvent::Feed(FeedStatus::Disconnected)),
    );
    assert_eq!(status(&model), "Disconnected");
}
