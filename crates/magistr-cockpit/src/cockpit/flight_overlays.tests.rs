use super::*;
use crate::agent::{AskPlan, PermissionChoice};
use crate::cockpit::{Cmd, Msg, Palette, Selected, Size, update};
use crate::harness::fixtures;
use crate::hit::HitRegistry;
use crate::input::{Button, KeyCode, KeyEvent, MouseEvent, MouseKind};
use crate::layout::{Basis, LayoutTree};
use serde_json::json;

fn model() -> Model {
    let mut model = Model::new(Size::new(128, 40));
    model.pixels = true;
    model.session.prompt.insert("Keep this draft");
    model.sync_flight_wrap();
    model
}

fn click(model: &mut Model, node: Node, id: crate::widget_id::WidgetId) -> Vec<Cmd> {
    let cell = model.cell_size();
    let mut face = Typography::default();
    face.set_cell(cell);
    let layout = LayoutTree::new()
        .compute_on(
            &node,
            model.flight_layout().screen_width,
            model.flight_layout().screen_height,
            Basis::new(f32::from(cell.width), f32::from(cell.height)),
            &mut face,
        )
        .unwrap();
    let placed = layout
        .placed()
        .iter()
        .find(|p| p.id == Some(id))
        .expect("drawn button")
        .px;
    let mut registry = HitRegistry::new();
    registry.register_layout_px(&layout, 0);
    let x = placed.x + placed.width * 0.5;
    let y = placed.y + placed.height * 0.5;
    let target = registry.hit_at(f64::from(x), f64::from(y));
    assert_eq!(target, Some(id));
    update(
        model,
        Msg::Mouse {
            event: MouseEvent::new(MouseKind::Down(Button::Left), 0, 0).at_px(x as u32, y as u32),
            target,
        },
    )
}

fn command(model: &mut Model, query: &str) {
    model.palette = Some(Palette::default());
    for c in query.chars() {
        model.palette.as_mut().unwrap().type_char(c);
    }
    let s = FlightStyle::for_model(model);
    let node = palette(model, &s);
    click(model, node, Widget::palette_row(0));
}

#[test]
fn commands_open_settings_and_theme_buttons_change_the_live_theme() {
    let mut model = model();
    command(&mut model, "Settings");
    assert!(model.settings().is_some());
    let s = FlightStyle::for_model(&model);
    let node = FlightSecondary::settings(&model, &s);
    click(&mut model, node, Widget::settings_theme(1));
    assert_eq!(
        model.settings_data().theme,
        crate::cockpit::ThemeChoice::Chart
    );
    let node = FlightSecondary::settings(&model, &FlightStyle::for_model(&model));
    click(&mut model, node, Widget::SettingsClose.id());
    assert!(model.settings().is_none());
    assert_eq!(model.session.prompt.text(), "Keep this draft");
}

#[test]
fn search_selects_the_record_without_changing_the_conversation_or_draft() {
    let mut model = model();
    model.trace.apply(&fixtures::frame(
        "think",
        "step_appended",
        json!({"step":{
            "step_number":864,"thought":"Keep customer budgets separate","purpose":"Retry limits"
        }}),
    ));
    let turn = model.session.turn.clone();
    let viewport = model.session.viewport;
    command(&mut model, "Search recorded work");
    for c in "customer".chars() {
        update(&mut model, Msg::Key(KeyEvent::new(KeyCode::Char(c))));
    }
    let node = FlightSecondary::search(&model, &FlightStyle::for_model(&model));
    click(&mut model, node, Widget::search_row(0));
    assert_eq!(model.selected(), Some(&Selected::Step(864)));
    assert_eq!(model.session.turn, turn);
    assert_eq!(model.session.viewport, viewport);
    assert_eq!(model.session.prompt.text(), "Keep this draft");
}

#[test]
fn secondary_controls_remain_inside_a_small_window_with_larger_text() {
    for (size, height) in [(Size::new(64, 24), 20), (Size::new(64, 24), 28)] {
        let mut model = model();
        update(&mut model, Msg::Resize(size));
        update(
            &mut model,
            Msg::CellSize(Some(crate::pixel::CellSize { width: 10, height })),
        );
        for query in ["Settings", "Search recorded work", "Keyboard help"] {
            model.settings = None;
            model.search = None;
            model.sheet = false;
            command(&mut model, query);
            let style = FlightStyle::for_model(&model);
            let (node, ids) = match query {
                "Settings" => (
                    FlightSecondary::settings(&model, &style),
                    vec![Widget::SettingsClose.id(), Widget::SettingsOpenFile.id()],
                ),
                "Search recorded work" => (
                    FlightSecondary::search(&model, &style),
                    vec![Widget::OverlayQuery.id(), Widget::search_scope(4)],
                ),
                _ => (FlightSecondary::help(&model, &style), Vec::new()),
            };
            let root = Node::column().child(Node::centered_overlay(node));
            let mut face = Typography::default();
            face.set_cell(model.cell_size());
            let width = model.flight_layout().screen_width;
            let screen_height = model.flight_layout().screen_height;
            let resolved = LayoutTree::new()
                .compute_on(
                    &root,
                    width,
                    screen_height,
                    Basis::new(10.0, f32::from(height)),
                    &mut face,
                )
                .unwrap();
            for id in ids {
                let rect = resolved
                    .placed()
                    .iter()
                    .find(|p| p.id == Some(id))
                    .unwrap()
                    .px;
                assert!(
                    rect.x >= 0.0
                        && rect.y >= 0.0
                        && rect.x + rect.width <= width
                        && rect.y + rect.height <= screen_height,
                    "{query}: {id:?} {rect:?} in {width}×{screen_height}"
                );
                assert!(
                    rect.height >= f32::from(height),
                    "{query}: clipped control {id:?} {rect:?}"
                );
            }
        }
    }
}

#[test]
fn clicking_a_query_keeps_the_overlay_and_the_original_message() {
    let mut model = model();
    model.palette = Some(Palette::default());
    let node = palette(&model, &FlightStyle::for_model(&model));
    click(&mut model, node, Widget::OverlayQuery.id());
    assert!(model.palette().is_some());
    update(&mut model, Msg::Key(KeyEvent::new(KeyCode::Char('s'))));
    assert_eq!(model.palette().unwrap().query(), "s");
    model.palette = None;
    command(&mut model, "Search recorded work");
    let node = FlightSecondary::search(&model, &FlightStyle::for_model(&model));
    click(&mut model, node, Widget::OverlayQuery.id());
    assert!(model.search().is_some());
    update(&mut model, Msg::Key(KeyEvent::new(KeyCode::Char('s'))));
    assert_eq!(model.search().unwrap().query(), "s");
    assert_eq!(model.session.prompt.text(), "Keep this draft");
}

#[test]
fn permission_scroll_is_bounded_and_the_answer_button_keeps_working_at_the_end() {
    let mut model = model();
    model.dock.push_permission(
        Permission::new(
            7,
            "Review the changes",
            vec![
                PermissionChoice::new("allow", PermissionKind::AllowOnce),
                PermissionChoice::new("deny", PermissionKind::RejectOnce),
            ],
        )
        .about(AskPlan::new(
            (0..200).map(|i| format!("Check file {i} and preserve its existing contents.")),
        )),
    );
    let viewport = model.session.viewport;
    FlightOverlays::scroll_permission(&mut model, isize::MAX);
    let last = model.decision().scroll();
    assert!(last > 0);
    FlightOverlays::scroll_permission(&mut model, 1);
    assert_eq!(model.decision().scroll(), last);
    FlightOverlays::scroll_permission(&mut model, -1);
    assert_eq!(model.decision().scroll(), last - 1);
    let node = permission_card(
        &model,
        model.moment().unwrap(),
        &FlightStyle::for_model(&model),
    );
    let cmds = click(&mut model, node, Widget::Deny.id());
    assert!(!cmds.is_empty());
    assert!(model.moment().is_none());
    assert_eq!(model.decision().scroll(), 0);
    assert_eq!(model.session.viewport, viewport);
    assert_eq!(model.session.prompt.text(), "Keep this draft");
}

#[test]
fn permission_clicks_and_help_wheels_do_not_reach_the_workspace() {
    let mut model = model();
    let viewport = model.session.viewport;
    command(&mut model, "Keyboard help");
    assert!(model.is_sheet_open());
    update(
        &mut model,
        Msg::Mouse {
            event: MouseEvent::new(MouseKind::ScrollDown, 0, 0),
            target: Some(Widget::Stage.id()),
        },
    );
    update(
        &mut model,
        Msg::Mouse {
            event: MouseEvent::new(MouseKind::Down(Button::Left), 0, 0),
            target: Some(Widget::PromptAction.id()),
        },
    );
    assert_eq!(model.session.viewport, viewport);
    assert_eq!(model.session.prompt.text(), "Keep this draft");
    update(&mut model, Msg::Key(KeyEvent::new(KeyCode::Esc)));
    model.dock.push_permission(
        Permission::new(
            7,
            "Review",
            vec![PermissionChoice::new("deny", PermissionKind::RejectOnce)],
        )
        .about(AskPlan::new(["Check changes"])),
    );
    let cmds = update(
        &mut model,
        Msg::Mouse {
            event: MouseEvent::new(MouseKind::Down(Button::Left), 0, 0),
            target: Some(Widget::Commands.id()),
        },
    );
    assert!(cmds.is_empty());
    assert!(model.palette().is_none());
    assert!(model.moment().is_some());
}
