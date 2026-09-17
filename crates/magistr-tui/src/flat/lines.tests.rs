use super::*;
use magistr_application::sessions::{AgentEvent, ToolStatus, ToolVerb, TurnEnd};
use magistr_cockpit::capability::{Mode, TextReason};
use magistr_cockpit::cockpit::{Model, Msg, Size, TurnState, update};
use magistr_cockpit::engine::EngineEvent;

const SIZE: Size = Size::new(80, 24);

fn agent(model: &mut Model, event: AgentEvent) {
    update(model, Msg::Agent(event));
}

fn frame(model: &mut Model, family: &str, kind: &str, payload: serde_json::Value) {
    update(
        model,
        Msg::Engine(EngineEvent::local(
            magistr_cockpit::harness::fixtures::frame(family, kind, payload),
        )),
    );
}

fn flat(model: &Model) -> String {
    lines(model)
        .iter()
        .map(FlatLine::render)
        .collect::<Vec<_>>()
        .join("\n")
}

/// A screen-reader session mid-turn: the banner, the user's prompt, the
/// agent's prose, a finished tool call, a gate with its details, a
/// think-and-ship event carrying a glyph column, and an error.
fn session() -> Model {
    let mut model = Model::new(SIZE).with_title("magistr");
    update(
        &mut model,
        Msg::Mode(Mode::MinimalText(TextReason::ScreenReader)),
    );
    agent(
        &mut model,
        AgentEvent::UserText("run the gates\n".to_owned()),
    );
    agent(
        &mut model,
        AgentEvent::Text("Reading the config.\n".to_owned()),
    );
    agent(
        &mut model,
        AgentEvent::ToolCall {
            id: "c1".to_owned(),
            title: ".magistr/project.json".to_owned(),
            verb: ToolVerb::Read,
            status: ToolStatus::Pending,
            diff: None,
            images: Vec::new(),
            record: Default::default(),
        },
    );
    agent(
        &mut model,
        AgentEvent::ToolCallUpdate {
            id: "c1".to_owned(),
            title: None,
            status: Some(ToolStatus::Completed),
            diff: None,
            duration_ms: Some(12),
            images: Vec::new(),
            record: Default::default(),
        },
    );
    frame(
        &mut model,
        "ship",
        "task_added",
        serde_json::json!({"task_id": "t1", "title": "wire the banner"}),
    );
    frame(
        &mut model,
        "ship",
        "task_started",
        serde_json::json!({"task_id": "t1"}),
    );
    frame(
        &mut model,
        "ship",
        "check_recorded",
        serde_json::json!({"task_id": "t1", "check": {
            "name": "cargo nextest run", "passed": true, "exit_code": 0,
            "duration_ms": 1240, "details": "1284 tests run: 1284 passed"
        }}),
    );
    agent(
        &mut model,
        AgentEvent::Error("the transport went away".to_owned()),
    );
    agent(&mut model, AgentEvent::TurnEnded(TurnEnd::EndTurn));
    model
}

#[test]
fn shot_flat_session() {
    insta::assert_snapshot!(flat(&session()));
}

#[test]
fn the_opening_line_names_the_project_and_the_mode() {
    assert_eq!(
        lines(&Model::new(SIZE))[0].render(),
        "session: magistr cockpit (flat mode)"
    );
    assert_eq!(
        lines(&Model::new(SIZE).with_title("magistr"))[0].render(),
        "session: magistr (flat mode)"
    );
}

#[test]
fn the_agents_prose_arrives_as_written_and_the_cockpits_own_text_is_stripped() {
    let rendered = flat(&session());
    assert!(
        rendered.contains("agent: Reading the config."),
        "the agent's prose is content:\n{rendered}"
    );
    assert!(
        rendered.contains("event: task started - wire the banner"),
        "the glyph column and the separator dot are gone:\n{rendered}"
    );
    assert!(
        !rendered
            .chars()
            .any(|c| ('\u{2500}'..='\u{27bf}').contains(&c)),
        "no chrome glyph reaches a reader:\n{rendered}"
    );
}

#[test]
fn a_label_is_never_said_twice_in_a_row() {
    let rendered = flat(&session());
    assert!(
        rendered.contains("error: the transport went away"),
        "{rendered}"
    );
    assert!(!rendered.contains("error: error:"), "{rendered}");
}

#[test]
fn a_gates_details_follow_it_as_indented_continuations() {
    let rendered = flat(&session());
    assert!(
        rendered.contains("gate: cargo nextest run passed, exit 0 in 1240 ms"),
        "{rendered}"
    );
    assert!(
        rendered.contains("\n  1284 tests run: 1284 passed"),
        "the detail is under its head, not labelled again:\n{rendered}"
    );
}

#[test]
fn a_finished_call_settles_and_a_running_one_does_not() {
    let mut model = Model::new(SIZE);
    agent(&mut model, AgentEvent::Text("looking".to_owned()));
    agent(
        &mut model,
        AgentEvent::ToolCall {
            id: "c1".to_owned(),
            title: "cargo test".to_owned(),
            verb: ToolVerb::Run,
            status: ToolStatus::InProgress,
            diff: None,
            images: Vec::new(),
            record: Default::default(),
        },
    );
    assert_eq!(model.turn_state(), TurnState::Streaming);
    let call = lines(&model).pop().expect("the call");
    assert_eq!(call.render(), "tool: run cargo test - running");
    assert!(!call.is_settled(), "a running call can still report");

    agent(
        &mut model,
        AgentEvent::ToolCallUpdate {
            id: "c1".to_owned(),
            title: None,
            status: Some(ToolStatus::Failed),
            diff: None,
            duration_ms: None,
            images: Vec::new(),
            record: Default::default(),
        },
    );
    let call = lines(&model).pop().expect("the call");
    assert_eq!(call.render(), "tool: run cargo test - failed");
    assert!(call.is_settled(), "a failed call reports nothing more");
}

#[test]
fn the_open_prose_line_is_the_only_unsettled_one_while_deltas_arrive() {
    let mut model = Model::new(SIZE);
    agent(&mut model, AgentEvent::Text("first\nsecond".to_owned()));
    let open = lines(&model);
    let prose: Vec<_> = open
        .iter()
        .filter(|line| line.role() == Some(Role::Agent))
        .collect();
    assert_eq!(prose.len(), 2);
    assert!(
        prose[0].is_settled(),
        "a prose line that is not last can no longer grow"
    );
    assert!(!prose[1].is_settled(), "the last one is still growing");
}

#[test]
fn a_turn_that_stopped_streaming_is_settled_whole() {
    let mut model = Model::new(SIZE);
    agent(&mut model, AgentEvent::Text("half a thought".to_owned()));
    agent(
        &mut model,
        AgentEvent::ToolCall {
            id: "c1".to_owned(),
            title: "a call nobody answered".to_owned(),
            verb: ToolVerb::Tool,
            status: ToolStatus::Pending,
            diff: None,
            images: Vec::new(),
            record: Default::default(),
        },
    );
    assert!(lines(&model).iter().any(|line| !line.is_settled()));
    agent(&mut model, AgentEvent::TurnEnded(TurnEnd::Cancelled));
    assert!(
        lines(&model).iter().all(FlatLine::is_settled),
        "a cancelled call must not wedge everything behind it for ever"
    );
}
