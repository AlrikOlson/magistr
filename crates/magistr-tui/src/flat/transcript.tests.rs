use super::*;

#[test]
fn rich_requests_have_a_complete_linear_capture() {
    let model = magistr_cockpit::harness::Scenes::rich_requests(SIZE);
    let lines = crate::flat::lines(&model)
        .iter()
        .map(FlatLine::render)
        .collect::<Vec<_>>()
        .join("\n");
    assert!(lines.contains("Requests"));
    assert!(
        magistr_cockpit::harness::Scenes::RICH_BODY
            .lines()
            .all(|source| lines.contains(source))
    );
    insta::assert_snapshot!(lines);
}

use crate::flat::lines;
use magistr_application::sessions::{AgentEvent, ToolStatus, ToolVerb, TurnEnd};
use magistr_cockpit::cockpit::{Msg, Size, update};

const SIZE: Size = Size::new(80, 24);

fn agent(model: &mut Model, event: AgentEvent) {
    update(model, Msg::Agent(event));
}

#[test]
fn the_opening_lines_go_out_once_and_a_quiet_frame_says_nothing() {
    let mut model = Model::new(SIZE).with_title("magistr");
    let mut transcript = Transcript::new();
    assert_eq!(transcript.advance(&model), ["session: magistr (flat mode)"]);
    assert!(
        transcript.advance(&model).is_empty(),
        "a frame with nothing new writes nothing at all"
    );
    agent(&mut model, AgentEvent::Text("one\ntwo".to_owned()));
    assert_eq!(transcript.advance(&model), ["agent: one"]);
    assert!(
        transcript.advance(&model).is_empty(),
        "the open line is still growing"
    );
    agent(&mut model, AgentEvent::TurnEnded(TurnEnd::EndTurn));
    assert_eq!(transcript.advance(&model), ["agent: two"]);
}

#[test]
fn nothing_the_transcript_has_said_is_ever_said_differently() {
    // Every line the emitter hands out is recorded here and re-checked
    // against every later rendering: the redraw-free promise is that a
    // line, once read, is what the session says for ever.
    let mut model = Model::new(SIZE);
    let mut transcript = Transcript::new();
    let mut said: Vec<String> = Vec::new();
    let step = |model: &Model, transcript: &mut Transcript, said: &mut Vec<String>| {
        said.extend(transcript.advance(model));
        let now: Vec<String> = lines(model).iter().map(FlatLine::render).collect();
        assert!(
            now.len() >= said.len(),
            "the rendering shrank under what was already read"
        );
        assert_eq!(
            &now[..said.len()],
            said.as_slice(),
            "a line the reader has already heard changed"
        );
    };

    step(&model, &mut transcript, &mut said);
    agent(&mut model, AgentEvent::UserText("go\n".to_owned()));
    step(&model, &mut transcript, &mut said);
    agent(&mut model, AgentEvent::Text("first ".to_owned()));
    step(&model, &mut transcript, &mut said);
    agent(&mut model, AgentEvent::Text("and more\n".to_owned()));
    step(&model, &mut transcript, &mut said);
    agent(
        &mut model,
        AgentEvent::ToolCall {
            id: "c1".to_owned(),
            title: "cargo test".to_owned(),
            verb: ToolVerb::Run,
            status: ToolStatus::Pending,
            diff: None,
            images: Vec::new(),
            record: Default::default(),
        },
    );
    step(&model, &mut transcript, &mut said);
    agent(
        &mut model,
        AgentEvent::ToolCallUpdate {
            id: "c1".to_owned(),
            title: None,
            status: Some(ToolStatus::Completed),
            diff: None,
            duration_ms: Some(90),
            images: Vec::new(),
            record: Default::default(),
        },
    );
    step(&model, &mut transcript, &mut said);
    agent(&mut model, AgentEvent::Text("done".to_owned()));
    step(&model, &mut transcript, &mut said);
    agent(&mut model, AgentEvent::TurnEnded(TurnEnd::EndTurn));
    step(&model, &mut transcript, &mut said);

    assert_eq!(
        said,
        [
            "session: magistr cockpit (flat mode)",
            "you: go",
            "agent: first and more",
            "tool: run cargo test - done in 90 ms",
            "agent: done",
        ]
    );
    assert_eq!(transcript.emitted(), said.len());
}

#[test]
fn a_transport_error_settles_the_interrupted_call_before_the_error() {
    // A failed transport settles its running call so the screen reader can
    // announce both the interruption and its reason in source order.
    let mut model = Model::new(SIZE);
    let mut transcript = Transcript::new();
    transcript.advance(&model);
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
    agent(
        &mut model,
        AgentEvent::Error("the feed hiccuped".to_owned()),
    );
    assert_eq!(
        transcript.advance(&model),
        [
            "tool: run cargo test - no result reported",
            "error: the feed hiccuped"
        ]
    );
    agent(
        &mut model,
        AgentEvent::ToolCallUpdate {
            id: "c1".to_owned(),
            title: None,
            status: Some(ToolStatus::Completed),
            diff: None,
            duration_ms: None,
            images: Vec::new(),
            record: Default::default(),
        },
    );
    assert!(
        transcript.advance(&model).is_empty(),
        "a late status does not replay announced lines"
    );
}

#[test]
fn selected_documents_emit_once_during_streaming_and_refresh_without_losing_text() {
    use magistr_cockpit::cockpit::Widget;
    use magistr_cockpit::engine::EngineEvent;
    use magistr_cockpit::harness::fixtures;
    use magistr_cockpit::input::{Button, MouseEvent, MouseKind};
    use magistr_engine::NativeSnapshot;
    let mut model = Model::new(SIZE);
    let mut transcript = Transcript::new();
    transcript.advance(&model);
    agent(
        &mut model,
        AgentEvent::Text("Unfinished conversation".into()),
    );
    let seed = |description: &str| {
        Msg::Engine(EngineEvent::Seed(NativeSnapshot {
            chunks: vec![fixtures::chunk_from(serde_json::json!({
                "id":"record", "title":"Saved delivery", "description":description,
                "acceptance":["Keep every requirement", "Keep original bytes"]
            }))],
            ..NativeSnapshot::default()
        }))
    };
    update(&mut model, seed("**Original** source"));
    update(
        &mut model,
        Msg::Mouse {
            event: MouseEvent::new(MouseKind::Down(Button::Left), 0, 0),
            target: Some(Widget::roadmap_row(0)),
        },
    );
    let said = transcript.advance(&model).join("\n");
    assert!(said.contains("**Original** source"), "{said}");
    assert!(said.contains("2. Keep original bytes"), "{said}");
    assert!(transcript.advance(&model).is_empty());
    update(&mut model, seed("Updated source"));
    assert!(
        transcript
            .advance(&model)
            .join("\n")
            .contains("Updated source")
    );
    agent(&mut model, AgentEvent::TurnEnded(TurnEnd::EndTurn));
    assert_eq!(
        transcript.advance(&model),
        ["agent: Unfinished conversation"]
    );
}

#[test]
fn rich_selected_document_has_a_complete_linear_capture() {
    let model = magistr_cockpit::harness::Scenes::rich_record(SIZE);
    let capture = super::super::lines::detail_lines(&model)
        .iter()
        .map(FlatLine::render)
        .collect::<Vec<_>>()
        .join("\n");
    for text in [
        "1. Keep the original result",
        "2. Preserve reading position",
        "- [x] Recovery checked",
        "- [ ] Isolation review",
        "    let receipt = saved_result;",
        "Receipt storage",
    ] {
        assert!(capture.contains(text), "{text}: {capture}");
    }
    insta::assert_snapshot!(capture);
}
