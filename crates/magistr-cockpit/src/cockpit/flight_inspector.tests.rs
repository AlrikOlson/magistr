use super::*;
use crate::cockpit::{Msg, Size, StageTab, update};
use crate::harness::fixtures;
use crate::widget_id::WidgetId;
use serde_json::json;
fn model() -> Model {
    let mut model = Model::new(Size::new(144, 50));
    model.trace.apply(&fixtures::frame(
        "think",
        "step_appended",
        json!({"step":{
        "step_number":864,"purpose":"Keep budgets separate","thought":"Test actual latency.",
        "outcome":"Shared budget rejected.","rationale":"It delayed other tenants.",
        "next_action":"Keep isolated counters.","execution_ref":"task:verify"}}),
    ));
    model.rail.apply(&fixtures::frame(
        "ship",
        "task_added",
        json!({"task_id":"verify","title":"Check delivery"}),
    ));
    model.inspect(Selected::Step(864));
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
fn has(node: &Node, id: WidgetId) -> bool {
    node.widget_id() == Some(id) || node.child_nodes().iter().any(|c| has(c, id))
}
#[test]
fn every_page_uses_complete_requirements_and_named_record_links() {
    let mut model = model();
    model.pixels = true;
    model.roadmap.seed(&[fixtures::chunk_from(json!({"id":"source","title":"Saved retry limits",
        "description":"Remember the schedule.","acceptance":["Keep the delay.","Separate each limit."],
        "deps":["receipts","missing"]})),fixtures::chunk_from(json!({"id":"receipts","title":"Receipt storage","group":"Storage"}))],&[]);
    model.inspect(Selected::Chunk("source".into()));
    for tab in StageTab::ALL {
        model.tab = tab;
        let view = Inspector::flight_view(&model, 500.0, 800.0);
        let out = text(&view);
        assert!(out.contains("Keep the delay."), "{out}");
        assert!(out.contains("Separate each limit."));
        assert!(out.contains("Receipt storage · Storage →"));
        assert!(out.contains("This record is no longer available."));
        assert!(!has(&view, Widget::InspectorDetail.id()));
        assert!(has(&view, Widget::detail_link(0)));
    }
}
#[test]
fn check_evidence_names_its_origin_and_keeps_output_bytes() {
    for verified in [false, true] {
        for passed in [false, true] {
            let mut model = model();
            model.trace.apply(&fixtures::frame(
                "ship",
                "check_recorded",
                json!({"task_id":"verify","check":{
            "name":"Delivery checks","passed":passed,"verified":verified,"command":"cargo test",
            "details":"  > raw **output**\n\nlast\n","exit_code":if passed{0}else{1}}}),
            ));
            let selection = SessionItem::Moment(0, &model.trace.moments()[0]).selection();
            model.inspect(selection);
            let card = Inspection::of(&model).unwrap();
            assert_eq!(card.status.contains("self-reported"), !verified);
            let out = card.original();
            assert!(out.contains(if verified {
                "Magistr ran this check"
            } else {
                "The agent reported this result"
            }));
            assert!(out.contains("cargo test"));
            assert!(out.contains("  > raw **output**\n\nlast\n"));
            model.projection.seeded();
            model
                .projection
                .feed(magistr_workflow_contracts::FeedStatus::Disconnected);
            assert!(
                Inspection::of(&model)
                    .unwrap()
                    .original()
                    .contains("Waiting for current results")
            );
        }
    }
}
#[test]
fn long_details_hold_selection_draft_and_source_across_resize() {
    let mut model = model();
    model.pixels = true;
    model.trace.apply(&fixtures::frame("think","step_appended",json!({"step":{
        "step_number":900,"purpose":"Review every case","thought":"A finding about retry limits. ".repeat(150)}})));
    model.inspect(Selected::Step(900));
    model.session.prompt.insert("keep my draft");
    Inspector::flight_scroll(&mut model, 8);
    let anchor = model.inspection_state.borrow().anchor.clone().unwrap();
    update(&mut model, Msg::Resize(Size::new(64, 24)));
    let _ = Inspector::flight_view(&model, 640.0, 380.0);
    assert_eq!(
        model.inspection_state.borrow().anchor.as_ref(),
        Some(&anchor)
    );
    Inspector::flight_scroll(&mut model, isize::MAX);
    let bottom = model.inspector_scroll;
    Inspector::flight_scroll(&mut model, -1);
    assert!(model.inspector_scroll < bottom);
    Inspector::flight_scroll(&mut model, isize::MIN);
    assert_eq!(model.inspector_scroll, 0);
    assert_eq!(model.selected(), Some(&Selected::Step(900)));
    assert_eq!(model.prompt().text(), "keep my draft");
}
#[test]
fn refused_undo_leads_the_change_document() {
    let mut model = model();
    let entry = crate::workbench::JournalEntry {
        id: "7".into(),
        path: "src/lib.rs".into(),
        parse_ok: true,
        ..Default::default()
    };
    model.inspect(Selected::Change(entry.identity()));
    model.changes.set_entries(vec![entry]);
    model
        .changes
        .refuse("7", "The file changed after this edit.");
    let out = text(&Inspector::flight_view(&model, 600.0, 800.0));
    assert!(out.contains("Undo refused"));
    assert!(out.contains("The file changed after this edit."));
}
#[test]
fn locally_stopped_terminal_is_not_shown_as_a_running_tool() {
    use crate::cockpit::{Msg, update};
    use magistr_application::sessions::{AgentEvent, ToolRecord, ToolStatus, ToolVerb};
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
    assert_eq!(Inspection::of(&model).unwrap().status, "Running");
    model.terminals.kill("checks");
    assert_eq!(Inspection::of(&model).unwrap().status, "Stopped by you");
    assert!(text(&Inspector::flight_view(&model, 440.0, 648.0)).contains("Stopped by you"));
}
