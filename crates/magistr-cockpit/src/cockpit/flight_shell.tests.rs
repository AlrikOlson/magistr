use super::*;
use crate::agent::{AgentEvent, PermissionChoice, PermissionKind, TurnEnd};
use crate::cockpit::{Cmd, Msg, Selected, Size, Surface, update};
use crate::harness::fixtures;
use crate::hit::HitRegistry;
use crate::input::{Button, MouseEvent, MouseKind, Reply};
use crate::layout::{Basis, LayoutTree, Resolved};
use crate::pixel::Typography;
use magistr_engine::{FeedStatus, NativeSnapshot};
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
    model.prompt.insert("Keep this draft");
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
            Msg::Reply(Reply::CellPixels {
                width: cw,
                height: ch,
            }),
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
                risk: magistr_engine::policy::RiskTier::Low,
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
            Msg::Reply(Reply::CellPixels {
                width: cw,
                height: ch,
            }),
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
                Msg::Reply(Reply::CellPixels {
                    width: cw,
                    height: ch,
                }),
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
