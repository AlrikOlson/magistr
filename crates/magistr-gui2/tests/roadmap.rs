//! Native roadmap controls and navigation retain shared session state.

#[test]
fn roadmap_targets_follow_shared_links_actions_filters_and_graph_selection() {
    use magistr_cockpit::cockpit::{Cmd, Selected};
    use magistr_gui2::fixture::Fixture;
    let mut model = Fixture::roadmap(Size::new(120, 40));
    let mut frame = Frame::new().unwrap();
    paint(&mut frame, &mut model);
    let commands = update(&mut model, hit(&frame, Widget::RoadmapComplete.id()));
    assert!(
        matches!(commands.as_slice(), [Cmd::Engine(magistr_cockpit::cockpit::engine_cmd::EngineCmd::CompleteChunk { id })] if id == "delivery-running")
    );
    update(&mut model, hit(&frame, Widget::roadmap_dependency(0)));
    assert_eq!(
        model.selected(),
        Some(&Selected::Chunk("delivery-base".into()))
    );
    paint(&mut frame, &mut model);
    assert!(frame.rect_of(W::id(Widget::RoadmapStart.id())).is_none());
    let star = model
        .roadmap()
        .graph_ids()
        .iter()
        .position(|id| *id == "delivery-running")
        .unwrap();
    update(&mut model, hit(&frame, Widget::sky_star(star)));
    paint(&mut frame, &mut model);
    update(&mut model, hit(&frame, Widget::roadmap_dependency(1)));
    assert_eq!(model.tab(), StageTab::Session);
    assert_eq!(model.selected(), Some(&Selected::Step(701)));
    paint(&mut frame, &mut model);
    let receipt =
        magistr_gui2::ElementId(magistr_gui2::comp_trace::TraceComposition::RECEIPTS + 701);
    let [x, y, w, h] = frame
        .rect_of(receipt)
        .expect("related step opens with its work link");
    let target = frame
        .hits()
        .hit_at(f64::from(x + w / 2.0), f64::from(y + h / 2.0));
    assert_eq!(target, Some(Widget::trace_chunk(701)));
    update(
        &mut model,
        Msg::Mouse {
            event: MouseEvent::new(MouseKind::Down(Button::Left), 0, 0),
            target,
        },
    );
    paint(&mut frame, &mut model);
    assert_eq!(model.tab(), StageTab::Roadmap);
    assert_eq!(
        model.selected(),
        Some(&Selected::Chunk("delivery-running".into()))
    );
    let filter = model.roadmap().filter_label().to_owned();
    click(&frame, &mut model, Widget::RoadmapFilter);
    assert_ne!(model.roadmap().filter_label(), filter);
}

#[test]
fn missing_related_record_never_resolves_back_to_the_chunk_that_names_it() {
    use magistr_cockpit::cockpit::inspector::Inspector;
    use magistr_cockpit::{engine::EngineEvent, harness::fixtures};
    use magistr_workflow_contracts::NativeSnapshot;
    let mut model = Model::new(Size::new(120, 40))
        .with_surface(Surface::Native)
        .with_tab(StageTab::Roadmap);
    let chunks = fixtures::chunks_from(vec![serde_json::json!({
        "id":"planning", "name":"Plan deliveries", "status":"pending",
        "cross_refs":["check:missing", "think:404"]
    })]);
    update(
        &mut model,
        Msg::Engine(EngineEvent::Seed(NativeSnapshot {
            chunks,
            ..NativeSnapshot::default()
        })),
    );
    update(
        &mut model,
        Msg::Mouse {
            event: MouseEvent::new(MouseKind::Down(Button::Left), 0, 0),
            target: Some(Widget::roadmap_row(0)),
        },
    );
    let links = Inspector::roadmap_links(&model);
    assert_eq!(links.len(), 2);
    assert!(
        links
            .iter()
            .all(|(name, target)| name.contains("unavailable") && target.is_none())
    );
}

#[test]
fn native_full_graph_retains_finished_work_outside_the_terminal_summary() {
    use magistr_cockpit::{engine::EngineEvent, harness::fixtures};
    use magistr_workflow_contracts::NativeSnapshot;
    use serde_json::json;
    let chunks = fixtures::chunks_from(
        (0..25)
            .map(|n| {
                json!({
                    "id": format!("done-{n}"), "name": format!("Finished item {n}"),
                    "status": "done", "priority": n + 1
                })
            })
            .collect(),
    );
    let mut model = Model::new(Size::new(120, 40))
        .with_surface(Surface::Native)
        .with_tab(StageTab::Roadmap);
    update(
        &mut model,
        Msg::Engine(EngineEvent::Seed(NativeSnapshot {
            chunks,
            ..NativeSnapshot::default()
        })),
    );
    let mut frame = Frame::new().unwrap();
    paint(&mut frame, &mut model);
    assert_eq!(model.roadmap().graph_ids().len(), 25);
    click(&frame, &mut model, Widget::TabTurn);
    paint(&mut frame, &mut model);
    click(&frame, &mut model, Widget::TabRoadmap);
    paint(&mut frame, &mut model);
    assert_eq!(model.roadmap().graph_ids().len(), 25);
}

use magistr_application::sessions::AgentEvent;
use magistr_cockpit::cockpit::{FrameClock, Model, Msg, Size, StageTab, Surface, Widget, update};
use magistr_cockpit::harness::Scenes;
use magistr_cockpit::input::{Button, MouseEvent, MouseKind};
use magistr_gui2::{Frame, comp_workspace::WorkspaceComposition as W};

fn paint(frame: &mut Frame, model: &mut Model) {
    frame
        .still(
            model,
            &FrameClock::new(0, std::time::Duration::ZERO),
            1100,
            760,
        )
        .unwrap();
}
fn click(frame: &Frame, model: &mut Model, widget: Widget) {
    let [x, y, w, h] = frame
        .rect_of(W::id(widget.id()))
        .expect("visible native control");
    let target = frame
        .hits()
        .hit_at(f64::from(x + w / 2.0), f64::from(y + h / 2.0));
    assert_eq!(target, Some(widget.id()));
    let commands = update(
        model,
        Msg::Mouse {
            event: MouseEvent::new(MouseKind::Down(Button::Left), 0, 0)
                .at_px((x + w / 2.0) as u32, (y + h / 2.0) as u32),
            target,
        },
    );
    assert!(!commands.iter().any(|c| matches!(
        c,
        magistr_cockpit::cockpit::Cmd::Prompt(_) | magistr_cockpit::cockpit::Cmd::RoadmapPrompt(_)
    )));
}
#[test]
fn full_roadmap_keeps_conversation_draft_and_reading_position() {
    let mut frame = Frame::new().unwrap();
    let mut model = Scenes::navigation(Size::new(120, 40)).with_surface(Surface::Native);
    paint(&mut frame, &mut model);
    let draft = model.prompt().clone();
    let first = model.visible_rows()[0].text.clone();
    click(&frame, &mut model, Widget::ReasoningToggle);
    click(&frame, &mut model, Widget::TabRoadmap);
    paint(&mut frame, &mut model);
    let says = frame
        .says()
        .into_iter()
        .map(|(s, _)| s)
        .collect::<Vec<_>>()
        .join("\n");
    for label in ["Work items", "Requests", "Item details"] {
        assert!(says.contains(label), "missing {label}");
    }
    assert!(
        frame
            .rect_of(magistr_gui2::comp_sky::SkyComposition::FIELD)
            .is_some()
    );
    click(&frame, &mut model, Widget::RoadmapRequests);
    paint(&mut frame, &mut model);
    assert!(model.roadmap_requests_open());
    update(
        &mut model,
        Msg::Agent(AgentEvent::Text(
            "Work continues while planning.\n\n".into(),
        )),
    );
    click(&frame, &mut model, Widget::TabTurn);
    paint(&mut frame, &mut model);
    assert_eq!(model.tab(), StageTab::Turn);
    assert_eq!(model.prompt(), &draft);
    assert_eq!(model.visible_rows()[0].text, first);
    assert!(model.reasoning_expanded());
    assert!(!model.uses_flight_recorder());
}

fn hit(frame: &Frame, target: magistr_cockpit::widget_id::WidgetId) -> Msg {
    let id = if target == Widget::Composer.id() {
        magistr_gui2::comp_composer::ComposerComposition::PILL
    } else {
        W::id(target)
    };
    let [x, y, w, h] = frame
        .rect_of(id)
        .unwrap_or_else(|| panic!("placed target {target:?}"));
    let hit = frame
        .hits()
        .hit_at(f64::from(x + w / 2.0), f64::from(y + h / 2.0));
    assert_eq!(hit, Some(target));
    Msg::Mouse {
        event: MouseEvent::new(MouseKind::Down(Button::Left), 0, 0)
            .at_px((x + w / 2.0) as u32, (y + h / 2.0) as u32),
        target: hit,
    }
}
fn says(frame: &Frame) -> String {
    frame
        .says()
        .into_iter()
        .map(|(s, _)| s)
        .collect::<Vec<_>>()
        .join("\n")
}
fn key(code: magistr_cockpit::input::KeyCode) -> Msg {
    Msg::Key(magistr_cockpit::input::KeyEvent::new(code))
}
#[test]
fn queued_planning_routes_to_its_owner_and_never_borrows_implementation_output() {
    use magistr_application::sessions::TurnEnd;
    use magistr_cockpit::cockpit::{Cmd, LoopEvent, RequestPhase};
    use magistr_cockpit::input::KeyCode;
    use magistr_cockpit_runtime::channel::Commands;
    use magistr_gui2::route::Route;
    let (sender, mut commands) = Commands::channel();
    let route = Route::new(sender);
    let mut model = Model::new(Size::new(120, 40))
        .with_surface(Surface::Native)
        .with_tab(StageTab::Roadmap);
    route.fold(
        &mut model,
        Msg::Loop(LoopEvent::Started {
            iteration: 2,
            max: 5,
        }),
    );
    route.fold(
        &mut model,
        Msg::Agent(AgentEvent::Text("IMPLEMENTATION BEFORE".into())),
    );
    route.fold(&mut model, Msg::Paste("Add recovery work".into()));
    route.fold(&mut model, key(KeyCode::Enter));
    assert_eq!(
        commands.try_next(),
        Some(Cmd::RoadmapPrompt("Add recovery work".into()))
    );
    route.fold(&mut model, Msg::RoadmapRequestQueued);
    route.fold(
        &mut model,
        Msg::Agent(AgentEvent::Text("\nIMPLEMENTATION AFTER".into())),
    );
    let mut frame = Frame::new().unwrap();
    paint(&mut frame, &mut model);
    assert!(!says(&frame).contains("IMPLEMENTATION"));
    assert!(says(&frame).contains("Queued"));
    route.fold(&mut model, hit(&frame, Widget::PromptAction.id()));
    assert_eq!(commands.try_next(), Some(Cmd::CancelRoadmapPrompt));
    route.fold(&mut model, Msg::RoadmapRequestCancelled);
    assert_eq!(model.roadmap_requests()[0].phase, RequestPhase::Stopped);
    assert_eq!(
        model.turn_state(),
        magistr_cockpit::cockpit::TurnState::Streaming
    );
    assert_eq!(model.prompt().text(), "Add recovery work");
    route.fold(&mut model, key(KeyCode::Enter));
    assert!(matches!(commands.try_next(), Some(Cmd::RoadmapPrompt(_))));
    route.fold(&mut model, Msg::RoadmapRequestQueued);
    route.fold(
        &mut model,
        Msg::Agent(AgentEvent::TurnEnded(TurnEnd::EndTurn)),
    );
    route.fold(&mut model, Msg::RoadmapRequestStarted);
    route.fold(
        &mut model,
        Msg::Agent(AgentEvent::Text("Recovery work is now planned.".into())),
    );
    route.fold(
        &mut model,
        Msg::Agent(AgentEvent::TurnEnded(TurnEnd::EndTurn)),
    );
    let requests = model.roadmap_requests().to_vec();
    let accounting = model.loop_status().cloned();
    route.fold(
        &mut model,
        Msg::Agent(AgentEvent::Text("LATER IMPLEMENTATION".into())),
    );
    paint(&mut frame, &mut model);
    let text = says(&frame);
    assert!(text.contains("Recovery work is now planned."));
    assert!(!text.contains("IMPLEMENTATION"));
    assert_eq!(model.roadmap_requests(), requests);
    assert_eq!(model.loop_status(), accounting.as_ref());
    assert!(commands.try_next().is_none(), "painting cannot dispatch");
}

#[test]
fn requests_and_details_hold_independent_offsets_at_every_scale() {
    use magistr_cockpit::input::KeyCode;
    use magistr_gui2::comp_roadmap::RoadmapComposition as R;
    for scale in [1.0, 1.5, 2.0] {
        let mut frame = Frame::new().unwrap();
        frame.rescale(scale).unwrap();
        let size = ((1100.0 * scale) as u32, (760.0 * scale) as u32);
        let mut model = Model::new(frame.metrics().grid(size.0, size.1))
            .with_surface(Surface::Native)
            .with_tab(StageTab::Roadmap);
        update(&mut model, Msg::Paste("Explain recovery".into()));
        update(&mut model, key(KeyCode::Enter));
        update(&mut model, Msg::RoadmapRequestStarted);
        update(
            &mut model,
            Msg::Agent(AgentEvent::Text("A separate recovery step.\n".repeat(100))),
        );
        let paint = |frame: &mut Frame, model: &mut Model| {
            frame
                .still(
                    model,
                    &FrameClock::new(0, std::time::Duration::ZERO),
                    size.0,
                    size.1,
                )
                .unwrap();
        };
        paint(&mut frame, &mut model);
        let [x, y, w, h] = frame.rect_of(R::REQUESTS).unwrap();
        let target = frame
            .hits()
            .hit_at(f64::from(x + w / 2.0), f64::from(y + h / 2.0));
        assert_eq!(target, Some(Widget::RequestBody.id()));
        update(
            &mut model,
            Msg::Mouse {
                event: MouseEvent::new(MouseKind::ScrollUp, 0, 0)
                    .at_px((x + w / 2.0) as u32, (y + h / 2.0) as u32),
                target,
            },
        );
        let held = model.roadmap_request_scroll();
        assert!(held > 0 && held < usize::MAX);
        update(&mut model, Msg::Agent(AgentEvent::Text("NEW REPLY".into())));
        paint(&mut frame, &mut model);
        assert_eq!(model.roadmap_request_scroll(), held);
        click(&frame, &mut model, Widget::RoadmapDetails);
        paint(&mut frame, &mut model);
        let inspector = model.inspector_offset();
        click(&frame, &mut model, Widget::RoadmapRequests);
        paint(&mut frame, &mut model);
        assert_eq!(model.roadmap_request_scroll(), held);
        assert_eq!(model.inspector_offset(), inspector);
        click(&frame, &mut model, Widget::RequestLatest);
        paint(&mut frame, &mut model);
        assert_eq!(model.roadmap_request_scroll(), usize::MAX);
        assert!(says(&frame).contains("NEW REPLY"));
    }
}

#[test]
fn compact_plan_can_return_from_requests_and_details_without_losing_its_draft() {
    use magistr_gui2::comp_roadmap::RoadmapComposition as R;
    let mut model = Scenes::navigation(Size::new(70, 32)).with_surface(Surface::Native);
    let draft = model.prompt().clone();
    update(
        &mut model,
        Msg::Mouse {
            event: MouseEvent::new(MouseKind::Down(Button::Left), 0, 0),
            target: Some(Widget::TabRoadmap.id()),
        },
    );
    let mut frame = Frame::new().unwrap();
    let paint = |frame: &mut Frame, model: &mut Model| {
        frame
            .still(
                model,
                &FrameClock::new(0, std::time::Duration::ZERO),
                620,
                640,
            )
            .unwrap();
    };
    paint(&mut frame, &mut model);
    for tab in [Widget::RoadmapRequests, Widget::RoadmapDetails] {
        click(&frame, &mut model, tab);
        paint(&mut frame, &mut model);
        assert!(frame.rect_of(R::LIST).is_none());
        click(&frame, &mut model, Widget::TabRoadmap);
        paint(&mut frame, &mut model);
        assert!(frame.rect_of(R::LIST).is_some());
        assert!(
            frame
                .rect_of(magistr_gui2::comp_sky::SkyComposition::FIELD)
                .is_some()
        );
        assert_eq!(model.prompt(), &draft);
    }
}
