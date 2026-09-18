//! The session feature's own contract: the draft and the turn live on
//! the session, a stop by the key and a stop by the button are the same
//! stop, a rejected prompt gives the draft back, a pinned window holds
//! its line and not its row across a re-wrap, the transcript's card is
//! selected by Enter and by a click through the same fold, and what
//! the supervisor reports is consumed by reference — a check by name, a
//! receipt by the spine head it reaches, a terminal by id.

use super::*;
use crate::cockpit::update::update;
use crate::cockpit::{Cmd, Eased, Msg, Selected, Size, StageTab, TurnState, Widget};
use crate::input::{Button, KeyCode, KeyEvent, Modifiers, MouseEvent, MouseKind};
use crate::turn::{LineKind, RowKind, Voice};
use magistr_application::sessions::{
    AgentEvent, EditDiff, ToolRecord, ToolStatus, ToolVerb, TurnEnd,
};

fn model() -> Model {
    Model::new(Size::new(80, 24)).with_tab(StageTab::Turn)
}

fn key(code: KeyCode) -> Msg {
    Msg::Key(KeyEvent::new(code))
}

fn ctrl(c: char) -> Msg {
    Msg::Key(KeyEvent::new(KeyCode::Char(c)).with(Modifiers::CTRL))
}

fn click(target: crate::WidgetId) -> Msg {
    Msg::Mouse {
        event: MouseEvent::new(MouseKind::Down(Button::Left), 10, 10),
        target: Some(target),
    }
}

fn type_text(m: &mut Model, text: &str) {
    for c in text.chars() {
        update(m, key(KeyCode::Char(c)));
    }
}

fn agent(m: &mut Model, event: AgentEvent) -> Vec<Cmd> {
    update(m, Msg::Agent(event))
}

fn settle(m: &mut Model) {
    for _ in 0..Eased::STEPS {
        update(m, Msg::Tick);
    }
}

fn last_row(m: &Model) -> (RowKind, String) {
    let row = m.turn().rows().last().expect("a row");
    (row.kind, row.text.clone())
}

/// The last source line, whole — a note wraps to more than one row on a
/// narrow stage.
fn last_note(m: &Model) -> String {
    let line = m.turn().lines().last().expect("a line");
    assert!(matches!(line.kind(), LineKind::Note), "{:?}", line.kind());
    line.text().to_owned()
}

fn tool_call(id: &str, status: ToolStatus, record: ToolRecord) -> AgentEvent {
    AgentEvent::ToolCall {
        id: id.to_owned(),
        title: format!("run {id}"),
        verb: ToolVerb::Run,
        status,
        diff: None,
        images: Vec::new(),
        record: Box::new(record),
    }
}

/// The draft, where the turn stands and its notice, the completion and
/// the reading position are the session's; the model's selectors read
/// them through it, so a sent prompt moves the same state a renderer
/// or the flat transcript reads.
#[test]
fn the_draft_and_the_turn_live_on_the_session() {
    let mut m = model();
    type_text(&mut m, "fix it");
    assert_eq!(m.session().prompt().text(), "fix it");
    assert_eq!(m.session().state(), TurnState::Idle);
    assert_eq!(m.session().notice(), None);
    let cmds = update(&mut m, key(KeyCode::Enter));
    assert!(cmds.contains(&Cmd::Prompt("fix it".to_owned())));
    assert!(m.session().prompt().is_empty());
    assert_eq!(m.session().state(), TurnState::Streaming);
    assert_eq!(m.turn_state(), m.session().state());
    assert!(
        m.session().viewport().is_following(),
        "the echo is at the tail"
    );
    assert_eq!(
        last_row(&m),
        (RowKind::Prose(Voice::User), "fix it".to_owned())
    );
    assert!(std::ptr::eq(m.turn(), m.session().turn()));
    agent(&mut m, AgentEvent::TurnEnded(TurnEnd::EndTurn));
    assert_eq!(m.session().state(), TurnState::AwaitingInput);
    assert_eq!(m.session().notice(), Some("Ready"));
}

/// ctrl-C and the composer's button both stop a streaming turn with one
/// `Cmd::Cancel`, leave the draft as it was, and say so on the stage;
/// the turn stays streaming until the agent confirms the stop.
#[test]
fn stopping_by_key_and_by_button_is_one_stop_that_keeps_the_draft() {
    let mut by_key = model();
    agent(&mut by_key, AgentEvent::Text("working…".to_owned()));
    type_text(&mut by_key, "next");
    assert_eq!(update(&mut by_key, ctrl('c')), vec![Cmd::Cancel]);
    assert_eq!(by_key.session().prompt().text(), "next");
    assert_eq!(by_key.session().state(), TurnState::Streaming);
    assert_eq!(
        last_row(&by_key),
        (RowKind::Note, "Stopping. Your draft is kept.".to_owned())
    );

    let mut by_button = model();
    agent(&mut by_button, AgentEvent::Text("working…".to_owned()));
    update(&mut by_button, Msg::Paste("next".to_owned()));
    assert_eq!(
        update(&mut by_button, click(Widget::PromptAction.id())),
        vec![Cmd::Cancel]
    );
    assert_eq!(by_button.session().prompt().text(), "next");
    assert_eq!(by_button.session().state(), TurnState::Streaming);
    assert_eq!(
        last_row(&by_button),
        (RowKind::Note, "Stopping. Your draft is kept.".to_owned())
    );

    for m in [&mut by_key, &mut by_button] {
        agent(m, AgentEvent::TurnEnded(TurnEnd::Cancelled));
        assert_eq!(m.session().state(), TurnState::AwaitingInput);
        assert_eq!(m.session().notice(), Some("Stopped"));
        assert_eq!(m.session().prompt().text(), "next");
    }
}

/// A prompt the supervisor did not admit comes back as the draft when
/// the draft is empty; a newer draft is kept and the rejected text
/// stays in the history.
#[test]
fn a_rejected_prompt_restores_an_empty_draft_and_keeps_a_newer_one() {
    let mut m = model();
    type_text(&mut m, "first");
    update(&mut m, key(KeyCode::Enter));
    agent(
        &mut m,
        AgentEvent::PromptRejected {
            text: "first".to_owned(),
            error: "not admitted".to_owned(),
        },
    );
    assert_eq!(m.session().state(), TurnState::AwaitingInput);
    assert_eq!(m.session().prompt().text(), "first");
    assert_eq!(
        last_note(&m),
        "Your message was not sent. Your draft is ready to edit."
    );

    update(&mut m, key(KeyCode::Enter));
    type_text(&mut m, "second");
    agent(
        &mut m,
        AgentEvent::PromptRejected {
            text: "first".to_owned(),
            error: "not admitted".to_owned(),
        },
    );
    assert_eq!(m.session().prompt().text(), "second");
    assert!(
        m.session()
            .prompt()
            .history()
            .iter()
            .any(|entry| entry == "first")
    );
    assert!(last_note(&m).contains("Your newer draft is kept"));
}

/// The anchor a pinned window is held by is a *line* of the transcript
/// and an offset within it — never a row index, which every re-wrap
/// moves. A narrower wrap and rows arriving under the window leave the
/// same line at the top; a window following the tail has no anchor.
#[test]
fn a_pinned_window_holds_its_line_across_a_re_wrap_and_arriving_rows() {
    let mut m = model();
    for i in 0..300 {
        agent(
            &mut m,
            AgentEvent::Text(format!("paragraph {i} holds its place\n")),
        );
    }
    assert_eq!(m.stage_anchor(), None, "following the tail holds nothing");
    update(&mut m, key(KeyCode::Home));
    for _ in 0..8 {
        update(&mut m, key(KeyCode::PageDown));
    }
    settle(&mut m);
    let (anchor, within) = m.stage_anchor().expect("a pinned window is anchored");
    assert_eq!(within, 0, "a cell surface holds whole rows");
    let line = anchor.line();
    assert!(line > 0, "pages down from the top");
    assert_eq!(
        m.turn().anchor_at(m.top_row()).map(|a| a.line()),
        Some(line)
    );
    let width = m.turn().width();
    update(&mut m, Msg::Resize(Size::new(40, 24)));
    assert!(m.turn().width() < width, "the turn re-wrapped narrower");
    assert_eq!(
        m.stage_anchor().map(|(a, _)| a.line()),
        Some(line),
        "the same line under the top after the re-wrap"
    );
    for i in 300..320 {
        agent(&mut m, AgentEvent::Text(format!("paragraph {i}\n")));
    }
    assert_eq!(
        m.stage_anchor().map(|(a, _)| a.line()),
        Some(line),
        "rows arriving under the window move nothing"
    );
    update(&mut m, key(KeyCode::End));
    settle(&mut m);
    assert_eq!(m.stage_anchor(), None);
}

/// Enter on an empty draft and a click on the card's row select the
/// transcript's card the same way: the tool becomes the selection and
/// the card turns.
#[test]
fn enter_on_an_empty_draft_and_a_row_click_select_the_transcripts_card() {
    let mut by_key = model();
    agent(
        &mut by_key,
        tool_call("c1", ToolStatus::Completed, ToolRecord::default()),
    );
    let rows = by_key.turn().len();
    assert_eq!(update(&mut by_key, key(KeyCode::Enter)), vec![]);
    assert_eq!(by_key.selected(), Some(&Selected::Tool("c1".to_owned())));
    assert!(by_key.turn().len() > rows, "the card opened");
    // With the call selected, Enter is the inspector's: it opens the
    // call's record rather than folding the card again.
    update(&mut by_key, key(KeyCode::Enter));
    assert!(by_key.inspector_open());

    let mut by_click = model();
    agent(
        &mut by_click,
        tool_call("c1", ToolStatus::Completed, ToolRecord::default()),
    );
    let head = by_click
        .turn()
        .rows()
        .iter()
        .position(|row| matches!(row.kind, RowKind::Tool(_)))
        .expect("the card's head row");
    assert_eq!(
        update(&mut by_click, click(Widget::stage_row(head))),
        vec![]
    );
    assert_eq!(by_click.selected(), Some(&Selected::Tool("c1".to_owned())));
    assert!(by_click.turn().len() > rows, "the card opened");
}

/// What the supervisor and the store report lands on the transcript as
/// a reference to its owner, never as a copy of its authority: a
/// recorded check is a card by the check's name, a completed call's
/// receipt reaches the spine's own head by id, and a call's terminal is
/// held by id while its bytes stay with the terminals.
#[test]
fn supervisor_outcomes_are_consumed_by_reference() {
    let mut m = model();
    update(
        &mut m,
        Msg::Engine(crate::engine::EngineEvent::local(
            crate::harness::fixtures::frame(
                "ship",
                "check_recorded",
                serde_json::json!({"task_id": "t1", "check": {
                    "name": "cargo test", "passed": true, "exit_code": 0,
                    "details": "42 passed"
                }}),
            ),
        )),
    );
    assert!(matches!(
        m.turn().lines().last().map(crate::turn::Line::kind),
        Some(crate::turn::LineKind::Check { name, .. }) if name == "cargo test"
    ));

    let cp = m.rail_mut().spine_mut().open(
        magistr_domain::workflow::CheckpointReason::Iteration,
        "iter 1",
    );
    agent(
        &mut m,
        AgentEvent::ToolCall {
            id: "e1".to_owned(),
            title: "edit src/lib.rs".to_owned(),
            verb: ToolVerb::Edit,
            status: ToolStatus::Completed,
            diff: Some(EditDiff {
                path: "src/lib.rs".to_owned(),
                old_text: String::new(),
                new_text: "pub fn one() {}\n".to_owned(),
            }),
            images: Vec::new(),
            record: Box::new(ToolRecord::default()),
        },
    );
    let receipt = m.turn().receipt("e1").expect("a completed call's receipt");
    assert_eq!(receipt.reach.as_deref(), Some(cp.as_str()));
    assert_eq!(receipt.ordinal, 1);

    agent(
        &mut m,
        AgentEvent::Terminal {
            terminal_id: "t1".to_owned(),
            bytes: b"hello\n".to_vec(),
        },
    );
    agent(
        &mut m,
        tool_call(
            "c1",
            ToolStatus::InProgress,
            ToolRecord {
                terminal: Some("t1".to_owned()),
                ..Default::default()
            },
        ),
    );
    assert_eq!(
        m.turn()
            .tool_record("c1")
            .and_then(|record| record.terminal.as_deref()),
        Some("t1")
    );
    assert!(m.term("t1").is_some(), "the bytes live with the terminals");
}
