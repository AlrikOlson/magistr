use super::*;
use magistr_cockpit::agent::{AgentEvent, ToolStatus, ToolVerb, TurnEnd};
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
fn a_line_behind_an_unsettled_one_waits_its_turn() {
    // The emitter takes the settled PREFIX, not every settled line: a
    // note pushed while a call is still running is read after it,
    // because that is the order it happened in.
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
    assert!(
        transcript.advance(&model).is_empty(),
        "the error is behind a call that has not reported"
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
    assert_eq!(
        transcript.advance(&model),
        ["tool: run cargo test - done", "error: the feed hiccuped",]
    );
}
