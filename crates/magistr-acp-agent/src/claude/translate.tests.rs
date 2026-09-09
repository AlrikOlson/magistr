//! The translator over the frames recorded from `claude` 2.1.258
//! (tests/fixtures/claude). A wire change fails here, by name.

use agent_client_protocol::schema::v1::{ToolCallStatus, ToolKind};

use super::*;

const TURN_THINKING: &str = include_str!("../../tests/fixtures/claude/turn_thinking.ndjson");
const PERMISSION_ALLOW: &str =
    include_str!("../../tests/fixtures/claude/tool_permission_allow.ndjson");
const PERMISSION_ALWAYS: &str =
    include_str!("../../tests/fixtures/claude/tool_permission_always.ndjson");
const PERMISSION_DENY: &str =
    include_str!("../../tests/fixtures/claude/tool_permission_deny.ndjson");
const CANCELLED: &str = include_str!("../../tests/fixtures/claude/turn_cancelled.ndjson");
const RESUME: &str = include_str!("../../tests/fixtures/claude/session_resume.ndjson");
const SET_MODEL: &str = include_str!("../../tests/fixtures/claude/set_model.ndjson");
const MCP: &str = include_str!("../../tests/fixtures/claude/mcp_connected.ndjson");

/// The frames the CLI wrote, in order; the driver's own `_sent` markers are
/// returned as `None` so a test can react where the driver did.
fn recorded(fixture: &str) -> Vec<Option<Frame>> {
    fixture
        .lines()
        .filter(|l| !l.trim().is_empty())
        .map(|line| {
            let value: serde_json::Value = serde_json::from_str(line).expect("fixture json");
            value
                .get("_sent")
                .is_none()
                .then(|| Frame::from_value(&value))
        })
        .collect()
}

/// Replay a fixture: every step, with an interrupt marked where the driver
/// sent one.
fn replay(fixture: &str) -> (TurnTranslator, Vec<Step>) {
    let mut translator = TurnTranslator::new();
    let mut steps = Vec::new();
    for frame in recorded(fixture).into_iter().flatten() {
        steps.extend(translator.translate(&frame));
    }
    (translator, steps)
}

fn text_of(update: &SessionUpdate) -> Option<&str> {
    let chunk = match update {
        SessionUpdate::AgentMessageChunk(c) => c,
        _ => return None,
    };
    match &chunk.content {
        AcpContent::Text(t) => Some(t.text.as_str()),
        _ => None,
    }
}

fn thought_of(update: &SessionUpdate) -> Option<&str> {
    let chunk = match update {
        SessionUpdate::AgentThoughtChunk(c) => c,
        _ => return None,
    };
    match &chunk.content {
        AcpContent::Text(t) => Some(t.text.as_str()),
        _ => None,
    }
}

fn messages(steps: &[Step]) -> String {
    steps
        .iter()
        .filter_map(|s| match s {
            Step::Update(u) => text_of(u),
            _ => None,
        })
        .collect()
}

fn thoughts(steps: &[Step]) -> String {
    steps
        .iter()
        .filter_map(|s| match s {
            Step::Update(u) => thought_of(u),
            _ => None,
        })
        .collect()
}

fn ends(steps: &[Step]) -> Vec<&TurnEnd> {
    steps
        .iter()
        .filter_map(|s| match s {
            Step::Done(end) => Some(end),
            _ => None,
        })
        .collect()
}

fn tool_calls(steps: &[Step]) -> Vec<&ToolCall> {
    steps
        .iter()
        .filter_map(|s| match s {
            Step::Update(u) => match u.as_ref() {
                SessionUpdate::ToolCall(call) => Some(call),
                _ => None,
            },
            _ => None,
        })
        .collect()
}

fn tool_updates(steps: &[Step]) -> Vec<&ToolCallUpdate> {
    steps
        .iter()
        .filter_map(|s| match s {
            Step::Update(u) => match u.as_ref() {
                SessionUpdate::ToolCallUpdate(update) => Some(update),
                _ => None,
            },
            _ => None,
        })
        .collect()
}

fn usages(steps: &[Step]) -> Vec<&UsageUpdate> {
    steps
        .iter()
        .filter_map(|s| match s {
            Step::Update(u) => match u.as_ref() {
                SessionUpdate::UsageUpdate(usage) => Some(usage),
                _ => None,
            },
            _ => None,
        })
        .collect()
}

fn asks(steps: &[Step]) -> Vec<(&str, &PermissionAsk)> {
    steps
        .iter()
        .filter_map(|s| match s {
            Step::Ask { request_id, ask } => Some((request_id.as_str(), ask)),
            _ => None,
        })
        .collect()
}

#[test]
fn two_turns_on_one_process_stream_their_text_once_and_end_with_usage() {
    let (translator, steps) = replay(TURN_THINKING);
    assert_eq!(
        messages(&steps),
        "pongping",
        "each delta once, the assistant frame not again"
    );
    assert_eq!(
        ends(&steps),
        [
            &TurnEnd::Stop(StopReason::EndTurn),
            &TurnEnd::Stop(StopReason::EndTurn)
        ]
    );
    let usage = usages(&steps);
    assert_eq!(usage.len(), 2);
    assert_eq!(usage[0].used, 2 + 17576 + 19923 + 4);
    assert_eq!(usage[0].size, 1_000_000, "sonnet's window from modelUsage");
    let cost = usage[1].cost.as_ref().expect("cost");
    assert_eq!(cost.currency, "USD");
    assert!(
        cost.amount > usage[0].cost.as_ref().expect("cost").amount,
        "cumulative"
    );
    assert_eq!(
        translator.session_id(),
        Some("00000000-0000-4000-8000-0000000001e3")
    );
    assert_eq!(translator.model(), Some("claude-sonnet-5"));
    assert!(tool_calls(&steps).is_empty());
}

#[test]
fn a_tool_call_is_announced_with_its_input_then_asked_then_completed() {
    let (_, steps) = replay(PERMISSION_ALLOW);
    assert!(!thoughts(&steps).is_empty(), "haiku thinks before the call");
    let calls = tool_calls(&steps);
    assert_eq!(calls.len(), 1);
    let call = calls[0];
    assert_eq!(call.kind, ToolKind::Edit);
    assert_eq!(call.status, ToolCallStatus::Pending);
    assert!(call.title.starts_with("Write"), "{}", call.title);
    assert_eq!(
        call.raw_input.as_ref().and_then(|i| i["content"].as_str()),
        Some("hello")
    );
    assert_eq!(call.locations.len(), 1, "the file the Write names");
    assert!(call.locations[0].path.ends_with("allow.txt"));
    assert_eq!(call.locations[0].line, None);
    let asked = asks(&steps);
    assert_eq!(asked.len(), 1);
    let (request_id, ask) = asked[0];
    assert_eq!(request_id, "00000000-0000-4000-8000-00000000016f");
    assert_eq!(ask.tool_use_id, call.tool_call_id.to_string());
    assert_eq!(ask.tool_name, "Write");
    assert_eq!(ask.permission_suggestions.len(), 1);
    // The ask comes after the announcement.
    let announced = steps.iter().position(
        |s| matches!(s, Step::Update(u) if matches!(u.as_ref(), SessionUpdate::ToolCall(_))),
    );
    let asked_at = steps.iter().position(|s| matches!(s, Step::Ask { .. }));
    assert!(announced < asked_at);
    let updates = tool_updates(&steps);
    assert_eq!(updates.len(), 1);
    assert_eq!(updates[0].tool_call_id, call.tool_call_id);
    assert_eq!(updates[0].fields.status, Some(ToolCallStatus::Completed));
    let content = serde_json::to_string(&updates[0].fields.content).expect("json");
    assert!(content.contains("File created successfully"), "{content}");
    let raw = updates[0]
        .fields
        .raw_output
        .as_ref()
        .and_then(|o| o.as_str())
        .expect("the tool_result content whole, as rawOutput");
    assert!(raw.starts_with("File created successfully"), "{raw}");
    assert_eq!(messages(&steps), "written");
    assert_eq!(ends(&steps), [&TurnEnd::Stop(StopReason::EndTurn)]);
}

#[test]
fn a_calls_location_is_the_file_its_input_names_with_a_reads_offset_as_the_line() {
    let mut translator = TurnTranslator::new();
    let steps = translator.translate(&Frame::parse(
        r#"{"type":"assistant","message":{"role":"assistant","content":[
            {"type":"tool_use","id":"t1","name":"Read","input":{"file_path":"/w/src/main.rs","offset":40,"limit":20}},
            {"type":"tool_use","id":"t2","name":"Bash","input":{"command":"ls"}}]}}"#,
    ));
    let calls = tool_calls(&steps);
    assert_eq!(calls.len(), 2);
    assert_eq!(
        calls[0].locations[0].path,
        std::path::PathBuf::from("/w/src/main.rs")
    );
    assert_eq!(calls[0].locations[0].line, Some(40));
    assert!(calls[1].locations.is_empty(), "a command names no file");
    assert_eq!(
        calls[1].raw_input,
        Some(serde_json::json!({"command": "ls"}))
    );
}

#[test]
fn allow_always_round_trips_the_same_ask() {
    let (_, steps) = replay(PERMISSION_ALWAYS);
    let asked = asks(&steps);
    assert_eq!(asked.len(), 1);
    assert_eq!(asked[0].1.permission_suggestions[0]["mode"], "acceptEdits");
    assert_eq!(
        tool_updates(&steps)[0].fields.status,
        Some(ToolCallStatus::Completed)
    );
    assert_eq!(messages(&steps), "written");
}

#[test]
fn a_denied_call_fails_with_the_denial_message_and_the_turn_still_ends() {
    let (_, steps) = replay(PERMISSION_DENY);
    assert_eq!(asks(&steps).len(), 1);
    let updates = tool_updates(&steps);
    assert_eq!(updates.len(), 1);
    assert_eq!(updates[0].fields.status, Some(ToolCallStatus::Failed));
    let content = serde_json::to_string(&updates[0].fields.content).expect("json");
    assert!(content.contains("the user declined"), "{content}");
    assert_eq!(messages(&steps), "refused");
    assert_eq!(ends(&steps), [&TurnEnd::Stop(StopReason::EndTurn)]);
}

#[test]
fn an_interrupted_turn_ends_cancelled_and_shows_no_interrupt_note() {
    // As the session drives it: the interrupt is marked where the driver sent it.
    let mut translator = TurnTranslator::new();
    let mut steps = Vec::new();
    for frame in recorded(CANCELLED) {
        match frame {
            Some(frame) => steps.extend(translator.translate(&frame)),
            None => translator.mark_interrupted(),
        }
    }
    assert_eq!(ends(&steps), [&TurnEnd::Stop(StopReason::Cancelled)]);
    assert!(
        !thoughts(&steps).is_empty(),
        "the thinking streamed before the interrupt is delivered"
    );
    assert_eq!(
        messages(&steps),
        "",
        "the CLI's interrupt note is not a message"
    );
    assert_eq!(usages(&steps).len(), 1);
}

#[test]
fn the_interrupt_note_alone_marks_the_turn_cancelled() {
    // Without the mark: the CLI's own note does it.
    let (translator, steps) = replay(CANCELLED);
    assert!(translator.is_interrupted());
    assert_eq!(ends(&steps), [&TurnEnd::Stop(StopReason::Cancelled)]);
}

#[test]
fn next_turn_forgets_the_interrupt() {
    let (mut translator, _) = replay(CANCELLED);
    translator.next_turn();
    assert!(!translator.is_interrupted());
}

#[test]
fn a_resumed_session_answers_from_its_history() {
    let (translator, steps) = replay(RESUME);
    assert_eq!(
        translator.session_id(),
        Some("00000000-0000-4000-8000-0000000001e3")
    );
    assert_eq!(messages(&steps), "pong");
    assert_eq!(ends(&steps), [&TurnEnd::Stop(StopReason::EndTurn)]);
}

#[test]
fn set_model_takes_effect_on_the_next_turn_and_its_response_is_not_a_step() {
    let (translator, steps) = replay(SET_MODEL);
    assert_eq!(
        translator.model(),
        Some("claude-sonnet-5"),
        "spawned on haiku, switched to sonnet"
    );
    assert_eq!(messages(&steps), "pong");
    assert!(
        steps.iter().all(|s| !matches!(s, Step::Refuse { .. })),
        "control responses are the process layer's, never refused"
    );
}

#[test]
fn magistr_mcp_serve_connects_under_the_direct_cli() {
    // The protocol cap (src/mcp_tools/protocol_cap.rs) holds without the npm
    // shim in between: the CLI negotiated with `magistr mcp-serve` and lists
    // it connected, tools and all.
    let (translator, steps) = replay(MCP);
    let servers = translator.mcp_servers();
    assert_eq!(servers.len(), 1);
    assert_eq!(servers[0].name, "magistr");
    assert_eq!(servers[0].status, "connected");
    assert_eq!(messages(&steps), "pong");
}

#[test]
fn a_control_request_the_provider_does_not_serve_is_refused_by_id() {
    let mut translator = TurnTranslator::new();
    let steps = translator.translate(&Frame::parse(
        r#"{"type":"control_request","request_id":"h-1","request":{"subtype":"hook_callback","callback_id":"x"}}"#,
    ));
    assert_eq!(
        steps,
        [Step::Refuse {
            request_id: "h-1".to_owned(),
            subtype: "hook_callback".to_owned()
        }]
    );
    let steps = translator.translate(&Frame::parse(
        r#"{"type":"control_request","request_id":"p-1","request":{"subtype":"can_use_tool","tool_name":"Bash"}}"#,
    ));
    assert!(
        matches!(&steps[0], Step::Refuse { subtype, .. } if subtype == "can_use_tool"),
        "an ask naming no tool call: {steps:?}"
    );
}

#[test]
fn unstreamed_assistant_text_is_delivered_whole_and_streamed_text_is_not_repeated() {
    let mut translator = TurnTranslator::new();
    let whole = translator.translate(&Frame::parse(
        r#"{"type":"assistant","message":{"content":[{"type":"text","text":"complete"}]}}"#,
    ));
    assert_eq!(messages(&whole), "complete");
    translator.translate(&Frame::parse(
        r#"{"type":"stream_event","event":{"type":"content_block_delta","index":0,"delta":{"type":"text_delta","text":"strea"}}}"#,
    ));
    let after = translator.translate(&Frame::parse(
        r#"{"type":"assistant","message":{"content":[{"type":"text","text":"streamed"}]}}"#,
    ));
    assert!(after.is_empty(), "{after:?}");
}

#[test]
fn failures_and_limits_map_to_the_turn_end() {
    let mut translator = TurnTranslator::new();
    let steps = translator.translate(&Frame::parse(
        r#"{"type":"result","subtype":"error_max_turns","is_error":true}"#,
    ));
    assert_eq!(ends(&steps), [&TurnEnd::Stop(StopReason::MaxTurnRequests)]);
    let steps = translator.translate(&Frame::parse(
        r#"{"type":"result","subtype":"error_during_execution","is_error":true,"errors":["boom","bang"]}"#,
    ));
    assert_eq!(ends(&steps), [&TurnEnd::Failed("boom; bang".to_owned())]);
    let steps = translator.translate(&Frame::parse(
        r#"{"type":"result","subtype":"success","is_error":false,"stop_reason":"max_tokens"}"#,
    ));
    assert_eq!(ends(&steps), [&TurnEnd::Stop(StopReason::MaxTokens)]);
    assert_eq!(usages(&steps)[0].size, TurnTranslator::DEFAULT_WINDOW);
}
