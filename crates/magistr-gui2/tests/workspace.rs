//! Exercise the placed desktop workspace through its actual hit and focus targets.
use magistr_application::sessions::AgentEvent;
use magistr_cockpit::cockpit::{
    FrameClock, Model, Msg, Selected, Size, StageTab, Surface, Widget, update,
};
use magistr_cockpit::engine::EngineEvent;
use magistr_cockpit::harness::{Scenes, fixtures};
use magistr_cockpit::input::{Button, KeyCode, KeyEvent, MouseEvent, MouseKind};
use magistr_cockpit::theme::{MotionMode, Theme, ThemeMode};
use magistr_cockpit::widget_id::WidgetId;
use magistr_gui2::{Frame, comp_root::RootComposition, comp_workspace::WorkspaceComposition as W};
use serde_json::json;

fn clock() -> FrameClock {
    FrameClock::new(0, std::time::Duration::ZERO)
}
fn paint(frame: &mut Frame, model: &mut Model, size: (u32, u32)) {
    frame.still(model, &clock(), size.0, size.1).unwrap();
}
fn click(frame: &mut Frame, model: &mut Model, target: WidgetId) {
    let id = if target == Widget::Composer.id() {
        magistr_gui2::comp_composer::ComposerComposition::PILL
    } else {
        W::id(target)
    };
    let [x, y, w, h] = frame.rect_of(id).expect("control is placed");
    let event = MouseEvent::new(MouseKind::Down(Button::Left), 0, 0)
        .at_px((x + w / 2.0) as u32, (y + h / 2.0) as u32);
    let hit = frame
        .hits()
        .hit_at(f64::from(x + w / 2.0), f64::from(y + h / 2.0));
    assert_eq!(hit, Some(target), "painted control must own its pixels");
    let msg = Msg::Mouse { event, target: hit };
    frame.fold_state(&msg);
    let commands = update(model, msg);
    assert!(
        !commands.iter().any(|c| matches!(
            c,
            magistr_cockpit::cockpit::Cmd::Prompt(_)
                | magistr_cockpit::cockpit::Cmd::RoadmapPrompt(_)
        )),
        "inspection and resizing must not submit the draft"
    );
}
fn text(frame: &Frame) -> String {
    frame
        .says()
        .into_iter()
        .map(|(s, _)| s)
        .collect::<Vec<_>>()
        .join("")
}
fn step(model: &mut Model, number: u32) {
    update(
        model,
        Msg::Engine(EngineEvent::local(fixtures::frame(
            "think",
            "step_appended",
            json!({"step": {"step_number":number, "purpose":"Keep deliveries separate",
        "thought":"Each customer keeps a separate allowance.", "outcome":"Save remaining attempts.",
        "rationale":"A busy customer must not delay another.", "next_action":"Check recovery after restart.",
        "execution_ref":"chunk:retry-work"}}),
        ))),
    );
}

#[test]
fn inspection_and_arrivals_preserve_the_draft_disclosure_and_reading_position() {
    let mut frame = Frame::new().unwrap();
    let mut model = Scenes::navigation(Size::new(120, 40)).with_surface(Surface::Native);
    let size = (1100, 760);
    paint(&mut frame, &mut model, size);
    let draft = model.prompt().clone();
    let first = model.visible_rows()[0].text.clone();
    assert!(!model.reasoning_expanded());
    step(&mut model, 950);
    paint(&mut frame, &mut model, size);
    assert!(
        !model.reasoning_expanded(),
        "incoming records do not open reasoning"
    );
    click(&mut frame, &mut model, Widget::ReasoningToggle.id());
    paint(&mut frame, &mut model, size);
    click(&mut frame, &mut model, Widget::LatestDecision.id());
    paint(&mut frame, &mut model, size);
    assert_eq!(model.selected(), Some(&Selected::Step(950)));
    for field in [
        "Result",
        "Why",
        "Next",
        "Save remaining attempts",
        "Check recovery",
    ] {
        assert!(text(&frame).contains(field), "missing {field}");
        let access = frame.access();
        let details = access.find(Widget::Inspector).unwrap();
        assert!(
            details.value.as_deref().unwrap_or_default().contains(field),
            "unreadable {field}"
        );
        assert!(!details.actions.contains(&accesskit::Action::SetValue));
    }
    step(&mut model, 980);
    update(
        &mut model,
        Msg::Agent(AgentEvent::Text("Another delivery is ready.\n\n".into())),
    );
    paint(&mut frame, &mut model, size);
    assert_eq!(model.selected(), Some(&Selected::Step(950)));
    assert_eq!(model.visible_rows()[0].text, first);
    assert_eq!(model.prompt(), &draft);
    click(&mut frame, &mut model, Widget::TracePrevious.id());
    assert_eq!(model.selected(), Some(&Selected::Step(941)));
    paint(&mut frame, &mut model, size);
    click(&mut frame, &mut model, Widget::TraceNext.id());
    assert_eq!(model.selected(), Some(&Selected::Step(950)));
    paint(&mut frame, &mut model, size);
    let link = magistr_cockpit::cockpit::Inspection::of(&model)
        .expect("step document")
        .links()
        .into_iter()
        .find(|l| {
            l.target
                == magistr_cockpit::cockpit::RecordTarget::Record(Selected::Chunk(
                    "retry-work".into(),
                ))
        })
        .expect("named work link")
        .widget;
    click(&mut frame, &mut model, Widget::InspectorTitle.id());
    update(&mut model, Msg::Key(KeyEvent::new(KeyCode::End)));
    paint(&mut frame, &mut model, size);
    click(&mut frame, &mut model, link);
    assert_eq!(
        model.selected(),
        Some(&Selected::Chunk("retry-work".into()))
    );
    // Return from the linked work through the shared primary-view command.
    update(
        &mut model,
        Msg::Mouse {
            event: MouseEvent::new(MouseKind::Down(Button::Left), 0, 0),
            target: Some(Widget::TabTurn.id()),
        },
    );
    paint(&mut frame, &mut model, size);
    assert_eq!(model.tab(), StageTab::Turn);
    assert_eq!(model.prompt(), &draft);
    assert_eq!(model.visible_rows()[0].text, first);
    assert!(model.reasoning_expanded());
    assert!(!model.uses_flight_recorder());
}

#[test]
fn native_dividers_resize_cancel_and_retain_the_composer_at_multiple_scales() {
    use magistr_cockpit::cockpit::flight_divider::FlightDivider as D;
    for scale in [1.0, 1.5, 2.0] {
        let mut frame = Frame::new().unwrap();
        frame.rescale(scale).unwrap();
        let size = ((1100.0 * scale) as u32, (760.0 * scale) as u32);
        let mut model =
            Scenes::navigation(frame.metrics().grid(size.0, size.1)).with_surface(Surface::Native);
        paint(&mut frame, &mut model, size);
        click(&mut frame, &mut model, Widget::ReasoningToggle.id());
        paint(&mut frame, &mut model, size);
        let draft = model.prompt().clone();
        let first = model.visible_rows()[0].text.clone();
        let composer = frame
            .rect_of(magistr_gui2::comp_composer::ComposerComposition::PILL)
            .unwrap();
        for divider in [D::Columns, D::Roadmap, D::Reasoning] {
            let target = divider.widget().id();
            let [x, y, w, h] = frame.rect_of(W::id(target)).unwrap();
            click(&mut frame, &mut model, target);
            update(
                &mut model,
                Msg::Mouse {
                    event: MouseEvent::new(MouseKind::Drag(Button::Left), 0, 0).at_px(
                        (x + w / 2.0 + 60.0 * scale as f32) as u32,
                        (y + h / 2.0 + 60.0 * scale as f32) as u32,
                    ),
                    target: None,
                },
            );
            paint(&mut frame, &mut model, size);
            assert_eq!(model.active_panel(), Some(divider));
            update(&mut model, Msg::Key(KeyEvent::new(KeyCode::Esc)));
            paint(&mut frame, &mut model, size);
            assert_eq!(model.active_panel(), None);
            assert_eq!(
                frame.rect_of(magistr_gui2::comp_composer::ComposerComposition::PILL),
                Some(composer)
            );
            assert_eq!(model.prompt(), &draft);
            assert_eq!(model.visible_rows()[0].text, first);
        }
        click(&mut frame, &mut model, Widget::Commands.id());
        for ch in "Resize panels".chars() {
            update(&mut model, Msg::Key(KeyEvent::new(KeyCode::Char(ch))));
        }
        update(&mut model, Msg::Key(KeyEvent::new(KeyCode::Enter)));
        assert_eq!(model.active_panel(), Some(D::Columns));
        update(&mut model, Msg::Key(KeyEvent::new(KeyCode::Home)));
        let minimum = model.panel_fraction(D::Columns, 0.65);
        update(&mut model, Msg::Key(KeyEvent::new(KeyCode::End)));
        assert!(model.panel_fraction(D::Columns, 0.65) > minimum);
        update(&mut model, Msg::Key(KeyEvent::new(KeyCode::Esc)));
        paint(&mut frame, &mut model, size);
        click(&mut frame, &mut model, Widget::Commands.id());
        for ch in "Reset panel sizes".chars() {
            update(&mut model, Msg::Key(KeyEvent::new(KeyCode::Char(ch))));
        }
        update(&mut model, Msg::Key(KeyEvent::new(KeyCode::Enter)));
        assert_eq!(model.panel_fraction(D::Columns, 0.65), 0.65);
        assert_eq!(model.prompt(), &draft);
        paint(&mut frame, &mut model, size);
        let access = frame.access();
        assert_eq!(
            access.find(Widget::PanelColumns).unwrap().role,
            accesskit::Role::Splitter
        );
        assert!(frame.rect_of(RootComposition::STAGE).unwrap()[2] < composer[2]);
    }
}

#[test]
fn details_scroll_without_moving_the_conversation_or_reaching_hidden_controls() {
    let mut frame = Frame::new().unwrap();
    let mut model = Scenes::navigation(Size::new(70, 32)).with_surface(Surface::Native);
    step(&mut model, 950);
    let size = (620, 640);
    paint(&mut frame, &mut model, size);
    click(&mut frame, &mut model, Widget::ReasoningToggle.id());
    paint(&mut frame, &mut model, size);
    click(&mut frame, &mut model, Widget::LatestDecision.id());
    paint(&mut frame, &mut model, size);
    let first = model.visible_rows()[0].text.clone();
    let draft = model.prompt().clone();
    let before = frame.rect_of(W::DETAIL_CONTENT).unwrap()[1];
    let [x, y, w, h] = frame.rect_of(W::DETAILS).unwrap();
    for _ in 0..8 {
        let target = frame
            .hits()
            .hit_at(f64::from(x + w - 2.0), f64::from(y + h / 2.0));
        assert!(target.is_some(), "the visible details receive the wheel");
        update(
            &mut model,
            Msg::Mouse {
                event: MouseEvent::new(MouseKind::ScrollDown, 0, 0)
                    .at_px((x + w - 2.0) as u32, (y + h / 2.0) as u32),
                target,
            },
        );
        paint(&mut frame, &mut model, size);
    }
    assert!(model.inspector_offset() > 0);
    assert!(frame.rect_of(W::DETAIL_CONTENT).unwrap()[1] < before);
    assert!(frame.access().find(Widget::LinkedWork).is_none());
    assert_eq!(model.visible_rows()[0].text, first);
    assert_eq!(model.prompt(), &draft);
}

#[test]
fn compact_panels_clip_focus_and_accessibility_to_the_visible_controls() {
    let mut frame = Frame::new().unwrap();
    let mut model = Scenes::navigation(Size::new(70, 32))
        .with_surface(Surface::Native)
        .with_theme(Theme::forced(ThemeMode::Light).with_motion(MotionMode::Reduced));
    let size = (620, 640);
    paint(&mut frame, &mut model, size);
    click(&mut frame, &mut model, Widget::ReasoningToggle.id());
    paint(&mut frame, &mut model, size);
    click(&mut frame, &mut model, Widget::LatestDecision.id());
    paint(&mut frame, &mut model, size);
    assert!(frame.rect_of(W::id(Widget::PanelColumns.id())).is_none());
    let draft = model.prompt().clone();
    for _ in 0..30 {
        frame.fold_state(&Msg::Key(KeyEvent::new(KeyCode::Tab)));
        if let Some(target) = frame.focus_target() {
            let access = frame.access();
            assert!(
                access
                    .nodes()
                    .iter()
                    .any(|node| node.target == Some(target)),
                "focus must be published only for visible controls: {target:?}"
            );
        }
    }
    assert_eq!(model.prompt(), &draft);
    let settled = model.clone();
    paint(&mut frame, &mut model, size);
    assert_eq!(model, settled, "rendering cannot advance semantic state");
}
