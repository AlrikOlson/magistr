use super::*;

fn roadmap_model() -> Model {
    use crate::harness::fixtures;
    use serde_json::json;
    let mut model = Model::new(Size::new(144, 50)).with_title("Harbor");
    model.pixels = true;
    model.tab = StageTab::Roadmap;
    model.roadmap.seed(
        &[fixtures::chunk_from(json!({
            "id": "internal-work-73",
            "title": "Save the retry schedule",
            "name": "Retry schedule"
        }))],
        &[],
    );
    model.inspect(Selected::Chunk("internal-work-73".into()));
    model
}

fn text(node: &Node) -> String {
    let mut out = node
        .text_metrics()
        .map_or(String::new(), |m| m.text().to_owned());
    for child in node.child_nodes() {
        out.push_str(&text(child));
        out.push('\n');
    }
    out
}

fn click(model: &mut Model, widget: Widget) -> Vec<crate::cockpit::Cmd> {
    use crate::cockpit::{FlightShell, FrameClock};
    use crate::hit::HitRegistry;
    use crate::input::{Button, MouseEvent, MouseKind};
    use crate::layout::{Basis, LayoutTree};
    let cell = model.cell_size();
    let mut face = Typography::default();
    face.set_cell(cell);
    let view = FlightShell::view(model, &FrameClock::new(0, std::time::Duration::ZERO));
    let resolved = LayoutTree::new()
        .compute_on(
            &view,
            f32::from(model.size().cols) * f32::from(cell.width),
            f32::from(model.size().rows) * f32::from(cell.height),
            Basis::new(f32::from(cell.width), f32::from(cell.height)),
            &mut face,
        )
        .unwrap();
    let rect = resolved
        .placed()
        .iter()
        .find(|p| p.id == Some(widget.id()))
        .unwrap()
        .px;
    assert!(
        rect.width > 0.0 && rect.height > 0.0,
        "{widget:?}: {rect:?}"
    );
    let x = rect.x + rect.width / 2.0;
    let y = rect.y + rect.height / 2.0;
    let mut hits = HitRegistry::new();
    hits.register_layout_px(&resolved, 0);
    let target = hits.hit_at(f64::from(x), f64::from(y));
    assert_eq!(target, Some(widget.id()), "{widget:?}: {rect:?}");
    update(
        model,
        Msg::Mouse {
            event: MouseEvent::new(MouseKind::Down(Button::Left), 0, 0).at_px(x as u32, y as u32),
            target,
        },
    )
    .into_iter()
    .filter(|cmd| !matches!(cmd, crate::cockpit::Cmd::Animate(_)))
    .collect()
}

#[test]
fn roadmap_request_pointer_multiline_send_and_stop_use_the_real_prompt_path() {
    use crate::cockpit::Cmd;
    use crate::input::Reply;
    for (size, cell_width, cell_height) in [
        (Size::new(144, 50), 10, 20),
        (Size::new(128, 40), 10, 20),
        (Size::new(110, 34), 12, 28),
        (Size::new(64, 24), 10, 20),
    ] {
        let mut model = roadmap_model();
        update(&mut model, Msg::Resize(size));
        update(
            &mut model,
            Msg::Reply(Reply::CellPixels {
                width: cell_width,
                height: cell_height,
            }),
        );
        assert!(click(&mut model, Widget::RoadmapRequest).is_empty());
        assert_eq!(model.prompt().text(), "For “Save the retry schedule”, ");
        update(
            &mut model,
            Msg::Paste("split this into two smaller items.".into()),
        );
        update(
            &mut model,
            Msg::Key(KeyEvent::new(KeyCode::Enter).with(Modifiers::SHIFT)),
        );
        update(
            &mut model,
            Msg::Paste("Keep recovery checks with each item.".into()),
        );
        let message = "For “Save the retry schedule”, split this into two smaller items.\nKeep recovery checks with each item.";
        assert_eq!(model.prompt().text(), message);
        assert_eq!(
            click(&mut model, Widget::PromptAction),
            vec![Cmd::Prompt(message.into())]
        );
        assert_eq!(model.turn_state(), TurnState::Streaming);
        update(&mut model, Msg::Paste("Keep my next request".into()));
        assert_eq!(click(&mut model, Widget::PromptAction), vec![Cmd::Cancel]);
        assert_eq!(model.prompt().text(), "Keep my next request");
    }
}

#[test]
fn roadmap_composer_names_its_request_and_selected_work_without_mutating_the_draft() {
    let mut model = roadmap_model();
    let view = FlightComposer::view(&model, &FlightStyle::for_model(&model), 1400.0, 112.0);
    let words = text(&view);
    assert!(words.contains("Request roadmap changes"));
    assert!(words.contains("Describe what you want to add or change…"));
    assert!(words.contains("Selected: Save the retry schedule"));
    assert!(!words.contains("internal-work-73"));
    assert!(model.prompt().is_empty());

    model.prompt.insert("Keep this draft");
    model.show(StageTab::Turn);
    assert_eq!(model.prompt().text(), "Keep this draft");
    let view = FlightComposer::view(&model, &FlightStyle::for_model(&model), 1400.0, 112.0);
    assert!(text(&view).contains("Message Magistr"));
    assert!(!text(&view).contains("Request roadmap changes"));
}

#[test]
fn selected_work_request_is_visible_editable_and_sent_unchanged() {
    use crate::agent::AgentEvent;
    use crate::cockpit::Cmd;
    let mut model = roadmap_model();
    FlightComposer::request_change(&mut model);
    assert_eq!(model.prompt().text(), "For “Save the retry schedule”, ");
    assert_eq!(model.pane(), Pane::Stage);
    model.prompt.insert("split this into two smaller items.");
    let draft = model.prompt().text().to_owned();
    let commands = update(&mut model, Msg::Key(KeyEvent::new(KeyCode::Enter)))
        .into_iter()
        .filter(|cmd| !matches!(cmd, Cmd::Animate(_)))
        .collect::<Vec<_>>();
    assert_eq!(commands, vec![Cmd::Prompt(draft.clone())]);
    assert!(model.prompt().is_empty());
    assert_eq!(model.prompt().history().last(), Some(&draft));
    update(
        &mut model,
        Msg::Agent(AgentEvent::PromptRejected {
            text: draft.clone(),
            error: "The message could not be sent. Try again.".into(),
        }),
    );
    assert_eq!(model.prompt().text(), draft);
}

#[test]
fn requesting_a_change_preserves_existing_text_cursor_and_missing_work() {
    let mut model = roadmap_model();
    model.prompt.insert("A different question");
    model.prompt.left();
    let draft = model.prompt.clone();
    FlightComposer::request_change(&mut model);
    assert_eq!(model.prompt, draft);
    assert_eq!(model.pane(), Pane::Stage);

    model.prompt.take();
    model.selected = Some(Selected::Chunk("missing-work".into()));
    FlightComposer::request_change(&mut model);
    assert!(model.prompt().is_empty());
    let view = FlightComposer::view(&model, &FlightStyle::for_model(&model), 1400.0, 112.0);
    assert!(text(&view).contains("Harbor"));
    assert!(!text(&view).contains("missing-work"));
}

#[test]
fn large_fonts_keep_the_full_action_and_file_control_inside_the_composer() {
    use crate::input::Reply;
    use crate::layout::{Basis, LayoutTree};
    for (width, height, cell_width, cell_height) in [
        (1440.0, 1000.0, 10, 20),
        (1280.0, 800.0, 12, 28),
        (640.0, 480.0, 10, 24),
    ] {
        let mut model = Model::new(Size::new(144, 50));
        update(
            &mut model,
            Msg::Reply(Reply::CellPixels {
                width: cell_width,
                height: cell_height,
            }),
        );
        model.prompt.insert("Check the retry schedule");
        let extent = FlightComposer::height(&model, width, height);
        let view = FlightComposer::view(&model, &FlightStyle::for_model(&model), width, extent);
        let mut face = Typography::default();
        face.set_cell(model.cell_size());
        let resolved = LayoutTree::new()
            .compute_on(
                &view,
                width,
                extent,
                Basis::new(f32::from(cell_width), f32::from(cell_height)),
                &mut face,
            )
            .unwrap();
        for widget in [Widget::PromptAction, Widget::PromptFiles] {
            let rect = resolved
                .placed()
                .iter()
                .find(|p| p.id == Some(widget.id()))
                .unwrap()
                .px;
            assert!(
                rect.height >= f32::from(cell_height),
                "{widget:?}: {rect:?}"
            );
            assert!(
                rect.y + rect.height <= extent - 1.0,
                "{widget:?}: {rect:?} in {extent}"
            );
        }
    }
}

#[test]
fn refused_attachment_restores_original_message_without_overwriting_newer_draft() {
    use crate::agent::AgentEvent;
    let mut model = Model::new(Size::new(144, 50));
    let original = "Read @missing.rs";
    model.prompt.insert(original);
    model.prompt.take();
    update(
        &mut model,
        Msg::Agent(AgentEvent::PromptRejected {
            text: original.into(),
            error: "Cannot attach missing.rs: the file is missing.".into(),
        }),
    );
    assert_eq!(model.prompt.text(), original);
    assert_eq!(model.turn_state(), TurnState::AwaitingInput);
    model.prompt.take();
    model.prompt.insert("My newer message");
    update(
        &mut model,
        Msg::Agent(AgentEvent::PromptRejected {
            text: original.into(),
            error: "Cannot attach missing.rs: the file is missing.".into(),
        }),
    );
    assert_eq!(model.prompt.text(), "My newer message");
    assert!(model.prompt.history().iter().any(|entry| entry == original));
}

use crate::cockpit::{Msg, Size, update};
use crate::input::{KeyCode, KeyEvent, Modifiers};

#[test]
fn add_files_keeps_draft_and_completion_quotes_spaces() {
    let mut model =
        Model::new(Size::new(144, 50)).with_workspace_files(vec!["docs/retry guide.md".into()]);
    model.prompt.insert("Check this");
    FlightComposer::add_files(&mut model);
    assert_eq!(model.prompt.text(), "Check this @");
    let completion = model.completion().expect("file chooser");
    assert_eq!(completion.items(), &["docs/retry guide.md"]);
    assert_eq!(
        completion.replacement().as_deref(),
        Some("@\"docs/retry guide.md\" ")
    );
    update(
        &mut model,
        Msg::Key(KeyEvent::new(KeyCode::Enter).with(Modifiers::NONE)),
    );
    assert_eq!(model.prompt.text(), "Check this @\"docs/retry guide.md\" ");
}

#[test]
fn composer_grows_from_measured_proportional_text_and_stops_at_bound() {
    let mut model = Model::new(Size::new(144, 50));
    assert_eq!(FlightComposer::height(&model, 1400.0, 1000.0), 112.0);
    model
        .prompt
        .insert(&"Long text including broad letters WWW ".repeat(100));
    let height = FlightComposer::height(&model, 600.0, 800.0);
    assert!(height > 112.0);
    assert!(height <= 240.0);
}

#[test]
fn empty_workspace_still_explains_file_chooser() {
    let mut model = Model::new(Size::new(80, 40));
    FlightComposer::add_files(&mut model);
    assert!(model.completion().is_some());
    assert_eq!(model.prompt.text(), "@");
}

#[test]
fn stop_preserves_draft_and_send_button_is_absent_when_empty() {
    let mut model = Model::new(Size::new(144, 50));
    let style = FlightStyle::for_model(&model);
    fn has(node: &Node, id: crate::widget_id::WidgetId) -> bool {
        node.widget_id() == Some(id) || node.child_nodes().iter().any(|n| has(n, id))
    }
    let view = FlightComposer::view(&model, &style, 1400.0, 112.0);
    assert!(!has(&view, Widget::PromptAction.id()));
    model.turn_state = TurnState::Streaming;
    model.prompt.insert("Keep this draft");
    let view = FlightComposer::view(&model, &style, 1400.0, 112.0);
    assert!(has(&view, Widget::PromptAction.id()));
    assert_eq!(model.prompt.text(), "Keep this draft");
}
