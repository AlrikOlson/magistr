use super::*;
use crate::cockpit::{FlightShell, FrameClock, Msg, Size, StageTab, Widget, update};
use crate::hit::HitRegistry;
use crate::layout::{Basis, LayoutTree};
use crate::pixel::Typography;
use crate::theme::{MotionMode, Theme};

fn model(tab: StageTab, cell: (u16, u16)) -> Model {
    let mut model = Model::new(Size::new(132, 42))
        .with_tab(tab)
        .with_theme(Theme::forced(crate::theme::ThemeMode::Dark).with_motion(MotionMode::Reduced));
    model.pixels = true;
    model.reasoning_expanded = true;
    model.prompt.insert("Keep my draft");
    update(
        &mut model,
        Msg::Reply(crate::input::Reply::CellPixels {
            width: cell.0,
            height: cell.1,
        }),
    );
    model
}

fn centre(model: &Model, divider: D) -> (f32, f32) {
    let cell = model.cell_size();
    let mut face = Typography::default();
    face.set_cell(cell);
    let resolved = LayoutTree::new()
        .compute_on(
            &FlightShell::view(model, &FrameClock::new(0, std::time::Duration::ZERO)),
            model.flight_layout().screen_width,
            model.flight_layout().screen_height,
            Basis::new(f32::from(cell.width), f32::from(cell.height)),
            &mut face,
        )
        .unwrap();
    let rect = resolved
        .placed()
        .iter()
        .find(|p| p.id == Some(divider.widget().id()))
        .expect("visible divider")
        .px;
    assert!(rect.width > 0.0 && rect.height > 0.0);
    let point = (rect.x + rect.width / 2.0, rect.y + rect.height / 2.0);
    let mut hits = HitRegistry::new();
    hits.register_layout_px(&resolved, 0);
    assert_eq!(
        hits.hit_at(f64::from(point.0), f64::from(point.1)),
        Some(divider.widget().id())
    );
    let (origin, _, first) = divider.geometry(model);
    let boundary = if divider == D::Columns {
        rect.x
    } else {
        rect.y
    };
    assert!(
        (boundary - origin - first).abs() < 1.0,
        "{divider:?}: {rect:?}"
    );
    point
}

fn mouse(
    model: &mut Model,
    kind: MouseKind,
    point: (f32, f32),
    target: Option<WidgetId>,
    pixels: bool,
) {
    let cell = model.cell_size();
    let mut event = MouseEvent::new(
        kind,
        (point.0 / f32::from(cell.width)) as u16,
        (point.1 / f32::from(cell.height)) as u16,
    );
    if pixels {
        event = event.at_px(point.0 as u32, point.1 as u32);
    }
    let cmds = update(model, Msg::Mouse { event, target });
    assert!(!cmds.iter().any(|c| matches!(
        c,
        crate::cockpit::Cmd::Prompt(_) | crate::cockpit::Cmd::RoadmapPrompt(_)
    )));
}

#[test]
fn dragging_keeps_selection_draft_and_conversation_anchor_while_output_arrives() {
    let mut m = model(StageTab::Turn, (10, 20));
    update(
        &mut m,
        Msg::Agent(crate::agent::AgentEvent::Text(
            (0..80)
                .map(|i| {
                    format!(
                        "Recorded paragraph {i}. {}\n\n",
                        "Keep the explanation readable. ".repeat(12)
                    )
                })
                .collect(),
        )),
    );
    assert!(m.reveal_conversation(12));
    m.inspect(crate::cockpit::Selected::Step(7));
    let selected = m.selected().cloned();
    let anchor = m.stage_anchor().expect("held reading position").0;
    let at = centre(&m, D::Columns);
    mouse(
        &mut m,
        MouseKind::Down(Button::Left),
        at,
        Some(Widget::PanelColumns.id()),
        true,
    );
    mouse(
        &mut m,
        MouseKind::Drag(Button::Left),
        (at.0 - 140.0, at.1),
        None,
        true,
    );
    update(
        &mut m,
        Msg::Agent(crate::agent::AgentEvent::Text(
            "A new observation.\n\n".into(),
        )),
    );
    assert_eq!(m.stage_anchor().unwrap().0, anchor);
    assert_eq!(m.selected(), selected.as_ref());
    assert_eq!(m.prompt.text(), "Keep my draft");
    assert!(
        m.turn
            .lines()
            .iter()
            .any(|line| line.text().contains("A new observation."))
    );
}

#[test]
fn commands_open_keyboard_resizing_and_restore_initial_sizes() {
    let mut m = model(StageTab::Roadmap, (10, 20));
    let initial = m.flight_layout();
    let command = |m: &mut Model, query: &str| {
        update(
            m,
            Msg::Key(KeyEvent::new(KeyCode::Char('p')).with(Modifiers::CTRL)),
        );
        assert!(m.palette.is_some());
        for c in query.chars() {
            key(m, KeyCode::Char(c));
        }
        key(m, KeyCode::Enter);
    };
    command(&mut m, "Resize panels");
    assert_eq!(m.flight_panels.active(), Some(D::Columns));
    key(&mut m, KeyCode::Left);
    key(&mut m, KeyCode::Enter);
    assert_ne!(m.flight_layout(), initial);
    command(&mut m, "Reset panel sizes");
    assert_eq!(m.flight_layout(), initial);
    assert_eq!(m.prompt.text(), "Keep my draft");
}

fn key(model: &mut Model, code: KeyCode) {
    update(model, Msg::Key(KeyEvent::new(code)));
}

#[test]
fn every_panel_divider_resizes_live_with_pixel_and_cell_reports() {
    for pixels in [false, true] {
        for cell in [(10, 20), (14, 28)] {
            for divider in D::ALL {
                let tab = if divider == D::Graph {
                    StageTab::Roadmap
                } else {
                    StageTab::Turn
                };
                let mut m = model(tab, cell);
                let at = centre(&m, divider);
                let before = divider.geometry(&m).2;
                let target = Some(divider.widget().id());
                mouse(&mut m, MouseKind::Down(Button::Left), at, target, pixels);
                let delta = if divider == D::Columns {
                    (60.0, 0.0)
                } else {
                    (0.0, -40.0)
                };
                let to = (at.0 + delta.0, at.1 + delta.1);
                mouse(
                    &mut m,
                    MouseKind::Drag(Button::Left),
                    to,
                    Some(Widget::RoadmapGraph.id()),
                    pixels,
                );
                assert!(
                    (divider.geometry(&m).2 - before).abs() > 20.0,
                    "{divider:?} {cell:?}"
                );
                assert_eq!(
                    m.hovered(),
                    target,
                    "drag keeps its pointer outside the divider"
                );
                assert!(m.roadmap_drag.is_none());
                let resized = divider.geometry(&m).2;
                centre(&m, divider);
                mouse(&mut m, MouseKind::Up(Button::Left), to, None, pixels);
                assert!(m.flight_panels.active().is_none());
                mouse(&mut m, MouseKind::Drag(Button::Left), at, None, pixels);
                assert_eq!(divider.geometry(&m).2, resized, "release ends capture");
                assert_eq!(m.prompt.text(), "Keep my draft");
            }
        }
    }
}

#[test]
fn resizing_clamps_and_remembers_preferences_through_compact_and_tab_changes() {
    let mut m = model(StageTab::Turn, (10, 20));
    let at = centre(&m, D::Columns);
    mouse(
        &mut m,
        MouseKind::Down(Button::Left),
        at,
        Some(Widget::PanelColumns.id()),
        true,
    );
    mouse(
        &mut m,
        MouseKind::Drag(Button::Left),
        (9999.0, at.1),
        None,
        true,
    );
    assert!((m.flight_layout().right_width - 280.0).abs() < 0.1);
    mouse(
        &mut m,
        MouseKind::Up(Button::Left),
        (9999.0, at.1),
        None,
        true,
    );
    let wide = m.flight_layout().left_width;
    update(&mut m, Msg::Resize(Size::new(64, 24)));
    assert!(m.flight_layout().compact);
    update(&mut m, Msg::Resize(Size::new(132, 42)));
    assert_eq!(m.flight_layout().left_width, wide);
    m.show(StageTab::Roadmap);
    assert_eq!(m.flight_layout().left_width, wide);
    m.show(StageTab::Turn);
    assert_eq!(m.flight_layout().left_width, wide);
}

#[test]
fn resize_during_drag_release_and_cancellation_keep_state_consistent() {
    for cancel in [Msg::Key(KeyEvent::new(KeyCode::Esc)), Msg::Focus(false)] {
        let mut m = model(StageTab::Roadmap, (10, 20));
        let initial = m.flight_panels.proportions;
        let at = centre(&m, D::Columns);
        mouse(
            &mut m,
            MouseKind::Down(Button::Left),
            at,
            Some(Widget::PanelColumns.id()),
            true,
        );
        update(&mut m, Msg::Resize(Size::new(160, 50)));
        mouse(
            &mut m,
            MouseKind::Drag(Button::Left),
            (at.0 + 80.0, at.1),
            None,
            true,
        );
        centre(&m, D::Columns);
        assert_ne!(m.flight_panels.proportions, initial);
        update(&mut m, cancel);
        assert_eq!(m.flight_panels.proportions, initial);
        assert!(m.flight_panels.active().is_none());
        assert!(m.pressed.is_none());
    }
}

#[test]
fn keyboard_adjustment_cycles_only_visible_boundaries_and_can_reset_or_cancel() {
    let mut m = model(StageTab::Roadmap, (10, 20));
    let initial = m.flight_layout();
    FlightPanels::begin_keyboard(&mut m);
    key(&mut m, KeyCode::Home);
    assert!((m.flight_layout().left_width - 360.0).abs() < 0.1);
    key(&mut m, KeyCode::End);
    assert!((m.flight_layout().right_width - 280.0).abs() < 0.1);
    key(&mut m, KeyCode::Char('r'));
    key(&mut m, KeyCode::Left);
    assert!(m.flight_layout().left_width < initial.left_width);
    key(&mut m, KeyCode::Tab);
    assert_eq!(m.flight_panels.active(), Some(D::Graph));
    key(&mut m, KeyCode::Up);
    assert!(m.flight_layout().graph_height < initial.graph_height);
    key(&mut m, KeyCode::Char('r'));
    assert_eq!(m.flight_layout(), initial);
    key(&mut m, KeyCode::Down);
    key(&mut m, KeyCode::Enter);
    assert!(m.flight_panels.active().is_none());
    let saved = m.flight_layout();
    FlightPanels::begin_keyboard(&mut m);
    key(&mut m, KeyCode::Right);
    key(&mut m, KeyCode::Esc);
    assert_eq!(m.flight_layout(), saved);
    assert_eq!(m.prompt.text(), "Keep my draft");
}
