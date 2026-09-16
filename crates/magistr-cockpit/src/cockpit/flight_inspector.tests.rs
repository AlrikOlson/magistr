use super::*;
use crate::cockpit::Size;
use crate::harness::fixtures;
use serde_json::json;

fn model() -> Model {
    let mut model = Model::new(Size::new(144, 50));
    model.trace.apply(&fixtures::frame("think", "step_appended", json!({
        "step": { "step_number": 864, "purpose":"Keep tenant budgets separate", "thought":"Test actual latency.",
        "outcome":"The shared budget was rejected.", "rationale":"It delayed other tenants.",
        "next_action":"Keep isolated counters.", "execution_ref":"task:internal-234" }
    })));
    model.rail.apply(&fixtures::frame(
        "ship",
        "task_added",
        json!({"task_id":"internal-234","title":"Save the retry schedule"}),
    ));
    model.selected = Some(Selected::Step(864));
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

fn roadmap_model() -> Model {
    let mut model = model();
    model.pixels = true;
    model.tab = crate::cockpit::StageTab::Roadmap;
    model.roadmap.seed(&[
        fixtures::chunk_from(json!({
            "id": "internal-retries", "title": "Saved retry limits",
            "description": "Remember the retry schedule after a restart.",
            "acceptance": ["Recovery keeps the saved delay.", "Each customer has a separate limit."],
            "deps": ["internal-receipts", "missing-internal-work"],
        })),
        fixtures::chunk_from(json!({
            "id": "internal-receipts", "title": "Receipt storage", "status": "done",
        })),
    ], &[]);
    model.inspect(Selected::Chunk("internal-retries".into()));
    model
}

fn has(node: &Node, id: WidgetId) -> bool {
    node.widget_id() == Some(id) || node.child_nodes().iter().any(|child| has(child, id))
}

#[test]
fn full_roadmap_details_show_success_conditions_and_dependencies_without_a_more_button() {
    let mut model = roadmap_model();
    let view = Inspector::flight_view(&model, 440.0, 800.0);
    let output = text(&view);
    assert_eq!(output.matches("Saved retry limits").count(), 1);
    assert!(output.contains("Recovery keeps the saved delay."));
    assert!(output.contains("Each customer has a separate limit."));
    assert!(output.contains("Receipt storage →"));
    assert!(output.contains("Unavailable work item"));
    assert!(!output.contains("internal-"));
    assert!(output.contains("Work item"));
    assert!(!has(&view, Widget::InspectorDetail.id()));
    assert!(has(&view, Widget::roadmap_dependency(0)));
    assert!(!has(&view, Widget::roadmap_dependency(1)));
    assert_eq!(Inspector::roadmap_dependency(&model, 1), None);

    model.tab = crate::cockpit::StageTab::Turn;
    let card = Card::of(&model).unwrap();
    assert_eq!(
        card.fields.len(),
        2,
        "conversation disclosure stays unchanged"
    );
    assert!(has(
        &Inspector::flight_view(&model, 440.0, 800.0),
        Widget::InspectorDetail.id()
    ));
    assert_eq!(Inspector::roadmap_dependency(&model, 0), None);
}

#[test]
fn related_work_resolves_explicit_targets_and_deduplicates_aliases_without_resolving_the_owner() {
    let mut model = roadmap_model();
    model.roadmap.seed(
        &[
            fixtures::chunk_from(json!({
                "id": "internal-retries", "title": "Saved retry limits",
                "cross_refs": [
                    "task:internal-234", "think:864", "step:864", "task:internal-234",
                    "chunk:internal-receipts", "internal-receipts",
                    "chunk:internal-retries", "internal-retries",
                    "task:missing-task", "think:9000", "missing-alias"
                ],
            })),
            fixtures::chunk_from(json!({
                "id": "internal-receipts", "title": "Receipt storage", "status": "done",
            })),
        ],
        &[],
    );
    let card = Card::of(&model).unwrap();
    let related = &card
        .fields
        .iter()
        .find(|(label, _)| label == "Related work")
        .unwrap()
        .1;
    assert_eq!(
        related,
        "Save the retry schedule, Step 864 — Keep tenant budgets separate, Receipt storage, Unavailable work item"
    );
    assert!(!related.contains("Saved retry limits"));
    assert!(!related.contains("internal-"));
    let view = Inspector::flight_view(&model, 700.0, 800.0);
    let output = text(&view);
    assert_eq!(output.matches("Saved retry limits").count(), 1);
    assert!(output.contains("Save the retry schedule"));
    assert!(output.contains("Step 864 — Keep tenant budgets separate"));
}

#[test]
fn roadmap_dependency_pointer_opens_exact_work_without_moving_the_draft_or_list() {
    use crate::cockpit::{FlightShell, FrameClock, Msg, StageTab, update};
    use crate::hit::HitRegistry;
    use crate::input::{Button, MouseEvent, MouseKind};
    use crate::layout::{Basis, LayoutTree};
    for size in [Size::new(144, 50), Size::new(64, 24)] {
        let mut model = roadmap_model();
        update(&mut model, Msg::Resize(size));
        model.prompt.insert("Keep this request");
        Inspector::flight_scroll(&mut model, isize::MAX);
        let draft = model.prompt.clone();
        let conversation = model.viewport;
        let list = model.roadmap_viewport;
        let cell = model.cell_size();
        let mut face = Typography::default();
        face.set_cell(cell);
        let view = FlightShell::view(&model, &FrameClock::new(0, std::time::Duration::ZERO));
        let resolved = LayoutTree::new()
            .compute_on(
                &view,
                f32::from(size.cols) * f32::from(cell.width),
                f32::from(size.rows) * f32::from(cell.height),
                Basis::new(f32::from(cell.width), f32::from(cell.height)),
                &mut face,
            )
            .unwrap();
        let id = Widget::roadmap_dependency(0);
        let rect = resolved
            .placed()
            .iter()
            .find(|placed| placed.id == Some(id))
            .unwrap()
            .px;
        let x = rect.x + rect.width / 2.0;
        let y = rect.y + rect.height / 2.0;
        let mut hits = HitRegistry::new();
        hits.register_layout_px(&resolved, 0);
        let target = hits.hit_at(f64::from(x), f64::from(y));
        assert_eq!(target, Some(id));
        update(
            &mut model,
            Msg::Mouse {
                event: MouseEvent::new(MouseKind::Down(Button::Left), 0, 0)
                    .at_px(x as u32, y as u32),
                target,
            },
        );
        assert_eq!(
            model.selected(),
            Some(&Selected::Chunk("internal-receipts".into()))
        );
        assert_eq!(model.tab(), StageTab::Roadmap);
        assert_eq!(model.prompt, draft);
        assert_eq!(model.viewport, conversation);
        assert_eq!(model.roadmap_viewport, list);
        assert_eq!(model.inspector_scroll, 0);
        if model.flight_layout().compact {
            let view = Inspector::flight_view(&model, 600.0, 400.0);
            assert!(text(&view).contains("Back to roadmap"));
            assert!(has(&view, Widget::TabRoadmap.id()));
        }
    }
}

#[test]
fn locally_stopped_terminal_is_not_shown_as_a_running_tool() {
    use crate::agent::{AgentEvent, ToolRecord, ToolStatus, ToolVerb};
    use crate::cockpit::{Msg, update};
    let mut model = model();
    update(
        &mut model,
        Msg::Agent(AgentEvent::Terminal {
            terminal_id: "checks".into(),
            bytes: b"Checking retry limits\r\n".to_vec(),
        }),
    );
    update(
        &mut model,
        Msg::Agent(AgentEvent::ToolCall {
            id: "run".into(),
            title: "Recovery checks".into(),
            verb: ToolVerb::Run,
            status: ToolStatus::InProgress,
            diff: None,
            images: Vec::new(),
            record: Box::new(ToolRecord {
                terminal: Some("checks".into()),
                ..ToolRecord::default()
            }),
        }),
    );
    model.selected = Some(Selected::Tool("run".into()));
    assert_eq!(Card::of(&model).unwrap().status, "Running");
    model.terminals.kill("checks");
    assert_eq!(Card::of(&model).unwrap().status, "Stopped by you");
    assert!(text(&Inspector::flight_view(&model, 440.0, 648.0)).contains("Stopped by you"));
}

#[test]
fn task_link_opens_named_task_even_before_any_recorded_actions() {
    let model = model();
    assert_eq!(
        Inspector::linked_work(&model, 864),
        Some((
            "Save the retry schedule".into(),
            Selected::Task("internal-234".into())
        ))
    );
}

#[test]
fn decision_details_preserve_missing_confidence_and_hide_internal_references() {
    let model = model();
    let output = text(&Inspector::flight_view(&model, 440.0, 648.0));
    assert!(output.contains("Step 864"));
    assert!(output.contains("Why"));
    assert!(output.contains("Next"));
    assert!(output.contains("Save the retry schedule"));
    assert!(!output.contains("internal-234"));
    assert!(!output.contains("0%"));
    assert!(!output.contains("execution ref"));
}

#[test]
fn missing_link_is_explained_without_exposing_a_workflow_id() {
    let mut model = model();
    model.rail = Default::default();
    assert_eq!(Inspector::linked_work(&model, 864), None);
    assert!(
        text(&Inspector::flight_view(&model, 440.0, 648.0))
            .contains("Linked work is no longer available.")
    );
}

#[test]
fn compact_details_scroll_back_immediately_after_reaching_the_end() {
    let mut model = model();
    crate::cockpit::update(&mut model, crate::cockpit::Msg::Resize(Size::new(64, 24)));
    model.trace.apply(&fixtures::frame(
        "think",
        "step_appended",
        json!({"step": {
            "step_number": 900,
            "thought": "A long finding about retry limits. ".repeat(100),
            "purpose": "Review every case",
            "outcome": "All restart cases passed."
        }}),
    ));
    model.selected = Some(Selected::Step(900));
    let layout = model.flight_layout();
    assert!(layout.compact);
    Inspector::flight_scroll(&mut model, isize::MAX);
    let bottom = text(&Inspector::flight_view(
        &model,
        layout.width,
        layout.body_height,
    ));
    Inspector::flight_scroll(&mut model, -5);
    let previous = text(&Inspector::flight_view(
        &model,
        layout.width,
        layout.body_height,
    ));
    assert_ne!(
        bottom, previous,
        "scrolling back responds at the first wheel movement"
    );
    assert_eq!(model.selected(), Some(&Selected::Step(900)));
}

#[test]
fn details_reveal_the_next_line_only_when_it_fits_completely() {
    let mut model = model();
    // The title, its spacing and the first line fit; only part of the result would fit.
    let output = text(&Inspector::flight_view(&model, 700.0, 148.0));
    assert!(output.contains("Test actual latency."));
    assert!(!output.contains("Result:"));
    model.inspector_scroll = 4;
    let scrolled = text(&Inspector::flight_view(&model, 700.0, 148.0));
    assert!(scrolled.contains("Result: The shared budget was rejected."));
    assert_eq!(model.selected(), Some(&Selected::Step(864)));
}

#[test]
fn long_details_scroll_without_changing_selection_or_message() {
    let mut model = model();
    model.trace.apply(&fixtures::frame(
        "think",
        "step_appended",
        json!({"step": {
            "step_number": 900, "thought": "A long finding about retry limits. ".repeat(150),
            "purpose": "Review every case"
        }}),
    ));
    model.selected = Some(Selected::Step(900));
    model.prompt.insert("keep my draft");
    let selection = model.selected.clone();
    Inspector::flight_scroll(&mut model, 1000);
    assert!(model.inspector_scroll > 0);
    Inspector::flight_scroll(&mut model, -1000);
    assert_eq!(model.inspector_scroll, 0);
    assert_eq!(model.selected, selection);
    assert_eq!(model.prompt.text(), "keep my draft");
}
