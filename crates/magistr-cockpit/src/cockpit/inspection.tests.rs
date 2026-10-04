use super::*;
use crate::cockpit::Size;
use crate::harness::fixtures;
use serde_json::json;

fn model() -> Model {
    Model::new(Size::new(144, 50))
}

fn body(card: &Inspection) -> String {
    card.sections
        .iter()
        .map(|s| format!("{}\n{}", s.heading, s.text.source()))
        .collect::<Vec<_>>()
        .join("\n")
}

#[test]
fn requirements_are_numbered_and_only_stored_checklists_have_boxes_on_every_page() {
    let mut model = model();
    model.roadmap.seed(&[fixtures::chunk_from(json!({
        "id":"record", "title":"Saved delivery", "status":"done",
        "description":"A different **authored** description.", "acceptance":["Keep the `receipt`.", "Never resend."],
        "content":{"version":1, "summary":"Read the saved requirements.",
            "facts":[{"label":"Owner", "value":"Delivery"}],
            "sections":[{"heading":"Evidence", "prose":"> Recorded findings.",
                "list":[{"text":"Actually checked", "done":true}, {"text":"Still open", "done":false}, {"text":"Plain item"}]}]}
    }))], &[]);
    model.inspect(Selected::Chunk("record".into()));
    for tab in super::super::StageTab::ALL {
        model.tab = tab;
        let card = Inspection::of(&model).unwrap();
        let content = body(&card);
        assert!(content.contains("1. Keep the `receipt`."), "{content}");
        assert!(content.contains("2. Never resend."));
        assert!(content.contains("- [x] Actually checked"));
        assert!(content.contains("- [ ] Still open"));
        assert!(!content.contains("[x] Keep"));
        assert!(content.contains("Owner"));
        assert_eq!(content.matches("Read the saved requirements.").count(), 1);
        assert!(
            !card
                .sections
                .iter()
                .find(|s| s.heading == "Acceptance criteria")
                .unwrap()
                .secondary
        );
    }
}

#[test]
fn cached_content_ignores_hover_and_ticks_and_rebuilds_for_changed_authored_content() {
    let mut model = model();
    model.roadmap.seed(
        &[fixtures::chunk_from(
            json!({"id":"record", "description":"First"}),
        )],
        &[],
    );
    model.inspect(Selected::Chunk("record".into()));
    let first = Inspection::of(&model).unwrap();
    model.hovered = Some(Widget::Inspector.id());
    model.ticks += 10;
    assert!(Rc::ptr_eq(&first, &Inspection::of(&model).unwrap()));
    model.roadmap.seed(
        &[fixtures::chunk_from(
            json!({"id":"record", "description":"Changed"}),
        )],
        &[],
    );
    let changed = Inspection::of(&model).unwrap();
    assert!(!Rc::ptr_eq(&first, &changed));
    assert!(body(&changed).contains("Changed"));
}

#[test]
fn recorded_results_lead_reasoning_and_named_links_resolve_or_explain_absence() {
    let mut model = model();
    model.trace.apply(&fixtures::frame(
        "think",
        "step_appended",
        json!({"step":{
            "step_number":7, "purpose":"Review delivery", "thought":"The finding.",
            "outcome":"The recorded result.", "dependencies":[8,99], "execution_ref":"task:verify"
        }}),
    ));
    model.trace.apply(&fixtures::frame(
        "think",
        "step_appended",
        json!({"step":{
            "step_number":8, "purpose":"Restart evidence", "thought":"A test."
        }}),
    ));
    model.rail.apply(&fixtures::frame(
        "ship",
        "task_added",
        json!({"task_id":"verify","title":"Check delivery"}),
    ));
    model.inspect(Selected::Step(7));
    let card = Inspection::of(&model).unwrap();
    assert_eq!(card.sections[0].heading, "Result");
    let links: Vec<_> = card.sections.iter().flat_map(|s| &s.links).collect();
    assert!(links.iter().any(|l| l.label.contains("Restart evidence")
        && l.target == RecordTarget::Record(Selected::Step(8))));
    assert!(links.iter().any(|l| l.target == RecordTarget::Unavailable));
    assert!(links.iter().any(|l| l.label == "Check delivery"));
}

#[test]
fn every_selected_kind_retains_its_own_recorded_content() {
    use crate::cockpit::{Msg, WorkbenchEvent, update};
    use crate::workbench::{JournalEntry, RunRow};
    use magistr_application::sessions::{AgentEvent, ToolRecord, ToolStatus, ToolVerb};
    use magistr_workflow_contracts::NativeSnapshot;
    let mut model = Model::new(Size::new(144,50)).with_snapshot(&NativeSnapshot {
        chunks: vec![fixtures::chunk_from(json!({"id":"work","title":"Work","description":"Chunk source"}))],
        steps: vec![fixtures::step_from(json!({"step_number":7,"purpose":"Finding","outcome":"Step result"}))],
        tasks: vec![fixtures::task_from(json!({"id":"task","title":"Task title","status":"completed","artifacts":[{"type":"file","ref":"result.txt","description":"Task artifact"}],
            "actions":[{"id":1,"description":"Action title","result":"Action result","type":"code"}]}))],
        signals: vec![fixtures::signal_from(json!({"id":"signal","body":"Signal source","status":"new",
            "content":{"version":1,"summary":"Signal summary","facts":[{"label":"Topic","value":"Recovery"}]}}))],
        ..NativeSnapshot::default()
    });
    let entry = JournalEntry {
        id: "change".into(),
        path: "src/result.rs".into(),
        diff: "+saved result".into(),
        parse_ok: true,
        ..Default::default()
    };
    let change = entry.identity();
    update(
        &mut model,
        Msg::Workbench(WorkbenchEvent::Journal {
            entries: vec![entry],
        }),
    );
    model.changes.set_runs(
        "test",
        vec![RunRow {
            name: "Recovery".into(),
            passed: true,
            detail: "Run result".into(),
        }],
    );
    update(
        &mut model,
        Msg::Agent(AgentEvent::ToolCall {
            id: "tool".into(),
            title: "Tool title".into(),
            verb: ToolVerb::Read,
            status: ToolStatus::Completed,
            diff: None,
            images: vec![],
            record: Box::new(ToolRecord {
                output: Some(magistr_application::sessions::ToolPayload::Text(
                    "Tool result".into(),
                )),
                ..ToolRecord::default()
            }),
        }),
    );
    let receipt =
        crate::cockpit::session_item::SessionItem::Moment(0, &model.trace.moments()[0]).selection();
    for (selected, needle) in [
        (Selected::Chunk("work".into()), "Chunk source"),
        (Selected::Step(7), "Step result"),
        (Selected::Task("task".into()), "Task artifact"),
        (receipt, "Action result"),
        (Selected::Signal("signal".into()), "Signal source"),
        (Selected::Change(change), "+saved result"),
        (Selected::Run(0), "Run result"),
        (Selected::Tool("tool".into()), "Tool result"),
    ] {
        model.inspect(selected.clone());
        let card = Inspection::of(&model).unwrap_or_else(|| panic!("missing {selected:?}"));
        assert!(
            card.original().contains(needle),
            "{selected:?}: {}",
            card.original()
        );
    }
}

#[test]
fn geometry_cache_reuses_measurement_until_width_changes() {
    let mut model = crate::harness::Scenes::rich_record(Size::new(144, 50));
    let card = Inspection::of(&model).unwrap();
    let mut calls = 0;
    let first = card.rows(&model, 40.0, 0, &mut |s, _| {
        calls += 1;
        s.chars().count() as f32
    });
    assert!(calls > 0);
    let count = calls;
    model.ticks += 1;
    let second = card.rows(&model, 40.0, 0, &mut |s, _| {
        calls += 1;
        s.chars().count() as f32
    });
    assert!(Rc::ptr_eq(&first, &second));
    assert_eq!(calls, count);
    card.rows(&model, 20.0, 0, &mut |s, _| {
        calls += 1;
        s.chars().count() as f32
    });
    assert!(calls > count);
}
#[test]
fn table_links_resolve_to_named_records() {
    let mut model = model();
    model.roadmap.seed(&[fixtures::chunk_from(json!({"id":"a","title":"First","description":"| Evidence |\n| --- |\n| [Saved result](chunk:b) |"})), fixtures::chunk_from(json!({"id":"b","title":"Named result"}))],&[]);
    model.inspect(Selected::Chunk("a".into()));
    let card = Inspection::of(&model).unwrap();
    assert!(card.links().iter().any(|l| l.label == "Named result"
        && l.target == RecordTarget::Record(Selected::Chunk("b".into()))));
}
#[test]
fn dependency_completion_refreshes_available_actions() {
    let mut model = model();
    let chunks = |status| {
        fixtures::chunks_from(vec![
            json!({"id":"work","status":"pending","deps":["dep"]}),
            json!({"id":"dep","status":status}),
        ])
    };
    model.roadmap.seed(&chunks("in_progress"), &[]);
    model.inspect(Selected::Chunk("work".into()));
    assert!(
        !Inspection::of(&model)
            .unwrap()
            .actions
            .iter()
            .any(|(label, _)| *label == "Start")
    );
    model.roadmap.seed(&chunks("done"), &[]);
    assert!(
        Inspection::of(&model)
            .unwrap()
            .actions
            .iter()
            .any(|(label, _)| *label == "Start")
    );
}
#[test]
fn work_readiness_labels_reasons_and_start_actions_agree() {
    let mut model = model();
    let chunks = fixtures::chunks_from(vec![
        json!({"id":"ground","name":"Saved foundation","status":"done"}),
        json!({"id":"active","name":"Active foundation","status":"in_progress"}),
        json!({"id":"ready","status":"pending","deps":["ground"]}),
        json!({"id":"waiting","status":"pending","deps":["active","absent"]}),
        json!({"id":"blocked","status":"pending","blocked_by":{"kind":"awaiting_human","reason":"Access is needed."}}),
    ]);
    model.roadmap.seed(&chunks, &[]);
    for (id, label, reason, can_start) in [
        ("ready", "Ready", "", true),
        (
            "waiting",
            "Waiting",
            "Waiting for Active foundation, an unavailable work item",
            false,
        ),
        ("blocked", "Blocked", "Access is needed.", false),
    ] {
        model.inspect(Selected::Chunk(id.into()));
        let card = Inspection::of(&model).unwrap();
        assert_eq!(card.status, label, "{id}");
        assert!(body(&card).contains(reason), "{id}: {}", body(&card));
        assert_eq!(
            card.actions.iter().any(|(label, _)| *label == "Start"),
            can_start
        );
    }
}

#[test]
fn partial_dependency_completion_refreshes_waiting_reasons_before_work_is_ready() {
    let mut model = crate::harness::Scenes::roadmap_readiness(Size::new(144, 50));
    let first = Inspection::of(&model).unwrap();
    assert!(body(&first).contains("Active foundation"));
    let changed = fixtures::frame(
        "roadmap",
        "chunk_changed",
        json!({"chunk":{"id":"active","name":"Active foundation","status":"done"}}),
    );
    model.roadmap.apply(&changed, 0.0, false);
    let second = Inspection::of(&model).unwrap();
    assert!(!Rc::ptr_eq(&first, &second));
    assert_eq!(second.status, "Waiting");
    let waiting = second
        .sections
        .iter()
        .find(|section| section.heading == "Waiting")
        .unwrap();
    assert_eq!(
        waiting.text.source(),
        "Waiting for an unavailable work item"
    );
    assert!(!second.actions.iter().any(|(label, _)| *label == "Start"));
}

#[test]
fn stale_projection_keeps_snapshot_readiness_and_its_freshness_visible() {
    use crate::cockpit::Freshness;
    use magistr_workflow_contracts::FeedStatus;
    let mut model = crate::harness::Scenes::roadmap_readiness(Size::new(144, 50));
    model.projection.feed(FeedStatus::Disconnected);
    assert_eq!(model.projection().freshness(), Freshness::Stale);
    assert_eq!(model.roadmap().next_ready().unwrap().id, "z-first");
    let card = Inspection::of(&model).unwrap();
    assert_eq!(card.status, "Waiting");
    assert_eq!(
        model.projection().freshness(),
        Freshness::Stale,
        "reading readiness never marks a stale snapshot current"
    );
}
