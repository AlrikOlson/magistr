use super::*;

#[test]
fn rich_geometry_reuses_measurements_and_rulers_are_distinct() {
    let mut model = Model::new(super::super::Size::new(80, 24));
    model
        .roadmap_requests
        .push(super::super::RoadmapRequest::new(
            "A long **review** of the delivery".into(),
        ));
    let history = RequestHistory::of(&model);
    let first = history.rows(12.0, 0, &mut |text, _| text.chars().count() as f32);
    let second = history.rows(12.0, 0, &mut |_, _| {
        panic!("unchanged geometry must not measure")
    });
    assert!(Rc::ptr_eq(&first, &second));
    let other = history.rows(12.0, 1, &mut |text, _| text.chars().count() as f32 * 2.0);
    assert!(!Rc::ptr_eq(&first, &other));
}

#[test]
fn request_controls_copy_the_source_and_named_address_without_submitting_a_draft() {
    let mut model = crate::harness::Scenes::rich_requests(super::super::Size::new(120, 40));
    let draft = model.prompt().clone();
    let source = RequestHistory::of(&model).original().to_owned();
    assert!(
        matches!(RequestHistory::activate(&mut model, Widget::request_copy()).unwrap().as_slice(), [Cmd::Clipboard(text)] if text == &source)
    );
    assert!(
        matches!(RequestHistory::activate(&mut model, Widget::request_link(0)).unwrap().as_slice(), [Cmd::Clipboard(text)] if text == "https://example.com/guide")
    );
    RequestHistory::activate(&mut model, Widget::request_jump(1)).unwrap();
    assert!(model.request_reading.borrow().anchor.is_some());
    assert_eq!(model.prompt(), &draft);
}
#[test]
fn request_link_keyboard_activation_uses_real_routing_and_can_leave_the_panel() {
    let mut model = crate::harness::Scenes::rich_requests(super::super::Size::new(120, 40));
    model.pane = Pane::Inspector;
    let draft = model.prompt().clone();
    super::super::update(
        &mut model,
        super::super::Msg::Key(KeyEvent::new(KeyCode::Tab)),
    );
    assert!(RequestHistory::is_focused_link(
        &model,
        Widget::request_link(0)
    ));
    let commands = super::super::update(
        &mut model,
        super::super::Msg::Key(KeyEvent::new(KeyCode::Enter)),
    );
    assert!(
        matches!(commands.as_slice(), [Cmd::Clipboard(text)] if text == "https://example.com/guide")
    );
    assert_eq!(model.prompt(), &draft);
    super::super::update(
        &mut model,
        super::super::Msg::Key(KeyEvent::new(KeyCode::Tab)),
    );
    assert_ne!(model.pane(), Pane::Inspector);
}

#[test]
#[ignore = "release-mode rich request latency report"]
fn report_rich_request_latency() {
    use std::hint::black_box;
    use std::time::Instant;
    let mut model = Model::new(super::super::Size::new(120, 40));
    model
        .roadmap_requests
        .push(super::super::RoadmapRequest::new("Review".into()));
    model.roadmap_requests[0].source_start = Some(0);
    model.session.turn.push_text(
        Voice::Agent,
        &"A **saved** result and [guide](https://example.com).\n".repeat(10_000),
    );
    let started = Instant::now();
    let history = RequestHistory::of(&model);
    history.rows(600.0, 0, &mut |text, _| text.chars().count() as f32 * 8.0);
    let cold = started.elapsed();
    let started = Instant::now();
    for _ in 0..100 {
        let history = RequestHistory::of(&model);
        black_box(history.rows(600.0, 0, &mut |_, _| {
            panic!("quiet frames reuse measured rows")
        }));
    }
    let steady = started.elapsed() / 100;
    let started = Instant::now();
    model
        .session
        .turn
        .push_text(Voice::Agent, "A new reply fragment.\n");
    let history = RequestHistory::of(&model);
    history.rows(600.0, 0, &mut |text, _| text.chars().count() as f32 * 8.0);
    let append = started.elapsed();
    eprintln!("rich requests 10000 lines: cold={cold:?} steady={steady:?} append={append:?}");
    assert!(steady < std::time::Duration::from_millis(16));
}

#[test]
fn request_tool_details_keep_the_evidence_owner_and_the_draft() {
    use magistr_application::sessions::{ToolStatus, ToolVerb};
    let mut model =
        Model::new(super::super::Size::new(80, 24)).with_tab(super::super::StageTab::Roadmap);
    model.roadmap_requests_open = true;
    model
        .roadmap_requests
        .push(super::super::RoadmapRequest::new("Review".into()));
    model.roadmap_requests[0].source_start = Some(0);
    model.session.turn.push_tool(
        "read-source",
        "Read source",
        ToolVerb::Read,
        ToolStatus::Failed,
    );
    model.session.prompt.insert("Keep this draft");
    let draft = model.prompt().clone();
    let history = RequestHistory::of(&model);
    assert!(
        history
            .sections
            .iter()
            .any(|section| section.heading.contains("Failed"))
    );
    RequestHistory::activate(&mut model, Widget::request_tool(0)).unwrap();
    assert_eq!(
        model.selected(),
        Some(&super::super::Selected::Tool("read-source".into()))
    );
    assert!(!model.roadmap_requests_open());
    assert_eq!(model.prompt(), &draft);
}

#[test]
fn empty_history_has_no_fabricated_reply() {
    let model = Model::new(super::super::Size::new(80, 24));
    let history = RequestHistory::of(&model);
    assert!(history.sections.is_empty());
    assert!(history.original().is_empty());
    assert!(history.links.is_empty());
}

#[test]
fn unchanged_sources_reuse_the_semantic_document() {
    let mut model = Model::new(super::super::Size::new(80, 24));
    model
        .roadmap_requests
        .push(super::super::RoadmapRequest::new("**Review**".into()));
    let first = RequestHistory::of(&model);
    let second = RequestHistory::of(&model);
    assert!(Rc::ptr_eq(&first, &second));
}
#[test]
fn native_section_head_anchor_does_not_drift_to_the_last_word() {
    let mut model = crate::harness::Scenes::rich_requests(super::super::Size::new(120, 40));
    let heading = ReadingPosition {
        section: "request:0".into(),
        source: None,
    };
    model.request_reading.borrow_mut().anchor = Some(heading.clone());
    let word = ReadingPosition {
        section: "request:0".into(),
        source: Some(crate::turn::DocumentAnchor { line: 0, byte: 12 }),
    };
    RequestHistory::report_native_positions(&mut model, vec![(heading, 0), (word, 24)]);
    assert_eq!(model.roadmap_request_scroll(), 0);
}
