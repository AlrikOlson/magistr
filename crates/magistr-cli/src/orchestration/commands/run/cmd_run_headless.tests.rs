use super::*;
use crate::agent::message::{AgentMessage, AssistantContent, ContentBlock, SessionId};
use crate::agent::mock::mock_session;
use crate::cli::HeadlessFormat;

#[tokio::test]
async fn partial_tool_output_survives_an_interrupted_turn() {
    let (mut session, tx) = mock_session();
    tx.send(Ok(tool_use_msg("Check"))).unwrap();
    tx.send(Ok(AgentMessage::StreamEvent { uuid:"tool-1".into(),session_id:SessionId::new("partial"),
        parent_tool_use_id:None,event:serde_json::json!({"type":"tool_call_update","tool_call_id":"tool-1","content":"compiler stopped before a newline","status":""}) })).unwrap();
    tx.send(Err(crate::error::MagistrError::Other(
        "connection lost".into(),
    )))
    .unwrap();
    let mut out = Vec::new();
    let mut err = Vec::new();
    assert!(
        drain_headless(
            &mut session,
            HeadlessFormat::Plain,
            None,
            &mut out,
            &mut err
        )
        .await
        .unwrap_err()
        .to_string()
        .contains("connection lost")
    );
    let activity = String::from_utf8(err).unwrap();
    assert!(
        activity.contains("compiler stopped before a newline"),
        "{activity}"
    );
    assert!(
        !activity.contains("[done]"),
        "interruption never invents successful tool completion"
    );
}

#[tokio::test]
async fn json_keeps_tool_updates_user_results_and_terminal_bytes() {
    use agent_client_protocol::schema::v1::{
        SessionUpdate, ToolCall, ToolCallStatus, ToolCallUpdate,
    };
    let (mut session, tx) = mock_session();
    let call = ToolCall::new("tool-json", "Read file")
        .status(ToolCallStatus::Completed)
        .raw_output(serde_json::json!("contents"));
    for message in crate::agent::types::messages_from_session_update(
        &SessionUpdate::ToolCallUpdate(ToolCallUpdate::from(call)),
        &SessionId::new("terminal"),
    ) {
        tx.send(Ok(message)).unwrap();
    }
    tx.send(Ok(AgentMessage::System {
        subtype: "terminal_output".into(),
        data: serde_json::json!({"terminal_id":"terminal","bytes":b"raw output"}),
    }))
    .unwrap();
    tx.send(Ok(result_msg(false, 1, None))).unwrap();
    let mut out = Vec::new();
    let mut err = Vec::new();
    drain_headless(&mut session, HeadlessFormat::Json, None, &mut out, &mut err)
        .await
        .unwrap();
    let text = String::from_utf8(out).unwrap();
    let records: Vec<serde_json::Value> = text
        .lines()
        .map(|line| serde_json::from_str(line).unwrap())
        .collect();
    assert_eq!(
        records
            .iter()
            .map(|record| record["type"].as_str().unwrap())
            .collect::<Vec<_>>(),
        ["tool_update", "tool_result", "terminal_output", "result"]
    );
    assert_eq!(records[1]["content"], "contents");
    assert_eq!(
        records[2]["data"]["bytes"],
        serde_json::json!(b"raw output")
    );
    assert!(err.is_empty(), "protocol output belongs on stdout");
}

#[tokio::test]
async fn plain_tool_updates_show_command_output_and_gate_outcome() {
    use agent_client_protocol::schema::v1::{
        SessionUpdate, ToolCall, ToolCallStatus, ToolCallUpdate, ToolCallUpdateFields,
    };
    let (mut session, tx) = mock_session();
    tx.send(Ok(text_msg("I’m checking the change."))).unwrap();
    let call = SessionUpdate::ToolCall(
        ToolCall::new("gate", "mcp__magistr__ship_check")
            .raw_input(serde_json::json!({"name":"check","command":"cargo check"})),
    );
    let update = SessionUpdate::ToolCallUpdate(ToolCallUpdate::new("gate",
        ToolCallUpdateFields::new().status(ToolCallStatus::Completed).raw_output(
            serde_json::json!({"content":[],"structuredContent":{"name":"check","passed":true,"exit_code":0,"details":"workspace compiled"}}))));
    for event in [call, update] {
        for message in
            crate::agent::types::messages_from_session_update(&event, &SessionId::new("terminal"))
        {
            tx.send(Ok(message)).unwrap();
        }
    }
    tx.send(Ok(text_msg("The checks passed."))).unwrap();
    tx.send(Ok(result_msg(false, 1, None))).unwrap();
    let mut out = Vec::new();
    let mut err = Vec::new();
    assert_eq!(
        drain_headless(
            &mut session,
            HeadlessFormat::Plain,
            None,
            &mut out,
            &mut err
        )
        .await
        .unwrap(),
        HeadlessOutcome::Ok
    );
    let text = String::from_utf8(out).unwrap();
    let activity = String::from_utf8(err).unwrap();
    assert!(text.contains("I’m checking the change.\n"), "{text}");
    assert!(activity.contains("cargo check"), "{activity}");
    assert!(
        activity.contains("Check check: passed (exit 0)"),
        "{activity}"
    );
    assert!(activity.contains("workspace compiled"), "{activity}");
    assert_eq!(
        activity.matches("workspace compiled").count(),
        1,
        "one result, not both projections"
    );
}

fn text_msg(text: &str) -> AgentMessage {
    AgentMessage::Assistant {
        parent_tool_use_id: None,
        message: AssistantContent {
            model: "test-model".to_string(),
            content: vec![ContentBlock::Text {
                text: text.to_string(),
            }],
        },
        session_id: None,
    }
}

fn tool_use_msg(name: &str) -> AgentMessage {
    AgentMessage::Assistant {
        parent_tool_use_id: None,
        message: AssistantContent {
            model: "test-model".to_string(),
            content: vec![ContentBlock::ToolUse {
                id: "tool-1".to_string(),
                name: name.to_string(),
                input: serde_json::json!({"file": "test.rs"}),
                kind: None,
                locations: vec![],
                tool_name: None,
            }],
        },
        session_id: None,
    }
}

fn result_msg(is_error: bool, num_turns: u32, cost: Option<f64>) -> AgentMessage {
    AgentMessage::Result {
        subtype: "result".to_string(),
        duration_ms: 1500,
        duration_api_ms: 1200,
        is_error,
        num_turns,
        session_id: SessionId::new("sess-headless"),
        total_cost_usd: cost,
        usage: None,
        result: None,
    }
}

fn usage_msg(cost: Option<f64>) -> AgentMessage {
    AgentMessage::UsageUpdate {
        context_tokens_used: 5000,
        context_window_size: 200_000,
        cost_usd: cost,
    }
}

fn cancelled_msg(reason: &str) -> AgentMessage {
    AgentMessage::Cancelled {
        session_id: SessionId::new("sess-headless"),
        reason: reason.to_string(),
    }
}

// ── Plain format tests ──────────────────────────────────────────

#[tokio::test]
async fn plain_streams_text_to_stdout() {
    let (mut session, tx) = mock_session();
    tx.send(Ok(text_msg("Hello, headless!"))).unwrap();
    tx.send(Ok(result_msg(false, 1, Some(0.05)))).unwrap();

    let mut out = Vec::new();
    let mut err = Vec::new();
    let outcome = drain_headless(
        &mut session,
        HeadlessFormat::Plain,
        None,
        &mut out,
        &mut err,
    )
    .await
    .unwrap();

    assert_eq!(outcome, HeadlessOutcome::Ok);
    let out_str = String::from_utf8(out).unwrap();
    assert!(out_str.contains("Hello, headless!"));
}

#[tokio::test]
async fn plain_tool_calls_go_to_stderr() {
    let (mut session, tx) = mock_session();
    tx.send(Ok(tool_use_msg("Read"))).unwrap();
    tx.send(Ok(result_msg(false, 1, None))).unwrap();

    let mut out = Vec::new();
    let mut err = Vec::new();
    let outcome = drain_headless(
        &mut session,
        HeadlessFormat::Plain,
        None,
        &mut out,
        &mut err,
    )
    .await
    .unwrap();

    assert_eq!(outcome, HeadlessOutcome::Ok);
    let err_str = String::from_utf8(err).unwrap();
    assert!(err_str.contains("[tool] Read"));
}

#[tokio::test]
async fn plain_summary_to_stderr() {
    let (mut session, tx) = mock_session();
    tx.send(Ok(text_msg("output"))).unwrap();
    tx.send(Ok(result_msg(false, 3, Some(0.12)))).unwrap();

    let mut out = Vec::new();
    let mut err = Vec::new();
    drain_headless(
        &mut session,
        HeadlessFormat::Plain,
        None,
        &mut out,
        &mut err,
    )
    .await
    .unwrap();

    let err_str = String::from_utf8(err).unwrap();
    assert!(err_str.contains("done: 3 turns"));
    assert!(err_str.contains("$0.1200"));
    assert!(err_str.contains("sess-headless"));
}

#[tokio::test]
async fn plain_error_result_returns_agent_error() {
    let (mut session, tx) = mock_session();
    tx.send(Ok(result_msg(true, 0, None))).unwrap();

    let mut out = Vec::new();
    let mut err = Vec::new();
    let outcome = drain_headless(
        &mut session,
        HeadlessFormat::Plain,
        None,
        &mut out,
        &mut err,
    )
    .await
    .unwrap();

    assert_eq!(outcome, HeadlessOutcome::AgentError);
    assert_eq!(outcome.exit_code(), 1);
}

// ── JSON format tests ───────────────────────────────────────────

#[tokio::test]
async fn json_streams_ndjson_text_events() {
    let (mut session, tx) = mock_session();
    tx.send(Ok(text_msg("Hello JSON"))).unwrap();
    tx.send(Ok(result_msg(false, 1, Some(0.03)))).unwrap();

    let mut out = Vec::new();
    let mut err = Vec::new();
    let outcome = drain_headless(&mut session, HeadlessFormat::Json, None, &mut out, &mut err)
        .await
        .unwrap();

    assert_eq!(outcome, HeadlessOutcome::Ok);

    let out_str = String::from_utf8(out).unwrap();
    let lines: Vec<&str> = out_str.trim().lines().collect();
    assert_eq!(lines.len(), 2);

    // First line: text event
    let text_event: serde_json::Value = serde_json::from_str(lines[0]).unwrap();
    assert_eq!(text_event["type"], "text");
    assert_eq!(text_event["content"], "Hello JSON");

    // Second line: result event
    let result_event: serde_json::Value = serde_json::from_str(lines[1]).unwrap();
    assert_eq!(result_event["type"], "result");
    assert_eq!(result_event["num_turns"], 1);
    assert_eq!(result_event["session_id"], "sess-headless");
}

#[tokio::test]
async fn json_tool_use_events() {
    let (mut session, tx) = mock_session();
    tx.send(Ok(tool_use_msg("Write"))).unwrap();
    tx.send(Ok(result_msg(false, 1, None))).unwrap();

    let mut out = Vec::new();
    let mut err = Vec::new();
    drain_headless(&mut session, HeadlessFormat::Json, None, &mut out, &mut err)
        .await
        .unwrap();

    let out_str = String::from_utf8(out).unwrap();
    let lines: Vec<&str> = out_str.trim().lines().collect();
    let tool_event: serde_json::Value = serde_json::from_str(lines[0]).unwrap();
    assert_eq!(tool_event["type"], "tool_use");
    assert_eq!(tool_event["name"], "Write");
    assert_eq!(tool_event["input"]["file"], "test.rs");
}

#[tokio::test]
async fn json_usage_events() {
    let (mut session, tx) = mock_session();
    tx.send(Ok(usage_msg(Some(0.02)))).unwrap();
    tx.send(Ok(result_msg(false, 1, Some(0.02)))).unwrap();

    let mut out = Vec::new();
    let mut err = Vec::new();
    drain_headless(&mut session, HeadlessFormat::Json, None, &mut out, &mut err)
        .await
        .unwrap();

    let out_str = String::from_utf8(out).unwrap();
    let lines: Vec<&str> = out_str.trim().lines().collect();
    let usage_event: serde_json::Value = serde_json::from_str(lines[0]).unwrap();
    assert_eq!(usage_event["type"], "usage");
    assert_eq!(usage_event["context_tokens_used"], 5000);
}

// ── Budget enforcement tests ────────────────────────────────────

#[tokio::test]
async fn budget_exceeded_cancels_session() {
    let (mut session, tx) = mock_session();
    tx.send(Ok(usage_msg(Some(5.50)))).unwrap();
    // Result would follow but budget should trigger first
    tx.send(Ok(result_msg(false, 1, Some(5.50)))).unwrap();

    let mut out = Vec::new();
    let mut err = Vec::new();
    let outcome = drain_headless(
        &mut session,
        HeadlessFormat::Plain,
        Some(5.00),
        &mut out,
        &mut err,
    )
    .await
    .unwrap();

    assert_eq!(outcome, HeadlessOutcome::BudgetExceeded);
    assert_eq!(outcome.exit_code(), 2);
    let err_str = String::from_utf8(err).unwrap();
    assert!(err_str.contains("budget limit exceeded"));
}

#[tokio::test]
async fn budget_not_exceeded_continues() {
    let (mut session, tx) = mock_session();
    tx.send(Ok(usage_msg(Some(3.00)))).unwrap();
    tx.send(Ok(result_msg(false, 1, Some(3.00)))).unwrap();

    let mut out = Vec::new();
    let mut err = Vec::new();
    let outcome = drain_headless(
        &mut session,
        HeadlessFormat::Plain,
        Some(5.00),
        &mut out,
        &mut err,
    )
    .await
    .unwrap();

    assert_eq!(outcome, HeadlessOutcome::Ok);
}

#[tokio::test]
async fn json_budget_exceeded_emits_error_event() {
    let (mut session, tx) = mock_session();
    tx.send(Ok(usage_msg(Some(10.0)))).unwrap();
    tx.send(Ok(result_msg(false, 1, Some(10.0)))).unwrap();

    let mut out = Vec::new();
    let mut err = Vec::new();
    let outcome = drain_headless(
        &mut session,
        HeadlessFormat::Json,
        Some(5.00),
        &mut out,
        &mut err,
    )
    .await
    .unwrap();

    assert_eq!(outcome, HeadlessOutcome::BudgetExceeded);

    let out_str = String::from_utf8(out).unwrap();
    let lines: Vec<&str> = out_str.trim().lines().collect();
    // Should have usage event + error event
    assert!(lines.len() >= 2);
    let error_event: serde_json::Value = serde_json::from_str(lines[1]).unwrap();
    assert_eq!(error_event["type"], "error");
    assert!(error_event["message"].as_str().unwrap().contains("budget"));
}

// ── Cancellation test ───────────────────────────────────────────

#[tokio::test]
async fn cancelled_returns_cancelled_outcome() {
    let (mut session, tx) = mock_session();
    tx.send(Ok(cancelled_msg("user requested"))).unwrap();

    let mut out = Vec::new();
    let mut err = Vec::new();
    let outcome = drain_headless(
        &mut session,
        HeadlessFormat::Plain,
        None,
        &mut out,
        &mut err,
    )
    .await
    .unwrap();

    assert_eq!(outcome, HeadlessOutcome::Cancelled);
    assert_eq!(outcome.exit_code(), 0);
    let err_str = String::from_utf8(err).unwrap();
    assert!(err_str.contains("cancelled: user requested"));
}

#[tokio::test]
async fn json_cancelled_emits_event() {
    let (mut session, tx) = mock_session();
    tx.send(Ok(cancelled_msg("timeout"))).unwrap();

    let mut out = Vec::new();
    let mut err = Vec::new();
    let outcome = drain_headless(&mut session, HeadlessFormat::Json, None, &mut out, &mut err)
        .await
        .unwrap();

    assert_eq!(outcome, HeadlessOutcome::Cancelled);
    let out_str = String::from_utf8(out).unwrap();
    let event: serde_json::Value = serde_json::from_str(out_str.trim()).unwrap();
    assert_eq!(event["type"], "cancelled");
    assert_eq!(event["reason"], "timeout");
}

// ── Empty stream test ───────────────────────────────────────────

#[tokio::test]
async fn empty_stream_returns_agent_error() {
    let (mut session, tx) = mock_session();
    drop(tx);

    let mut out = Vec::new();
    let mut err = Vec::new();
    let outcome = drain_headless(
        &mut session,
        HeadlessFormat::Plain,
        None,
        &mut out,
        &mut err,
    )
    .await
    .unwrap();

    assert_eq!(outcome, HeadlessOutcome::AgentError);
}

// ── Exit code tests ─────────────────────────────────────────────

#[test]
fn exit_codes_are_correct() {
    assert_eq!(HeadlessOutcome::Ok.exit_code(), 0);
    assert_eq!(HeadlessOutcome::Cancelled.exit_code(), 0);
    assert_eq!(HeadlessOutcome::AgentError.exit_code(), 1);
    assert_eq!(HeadlessOutcome::BudgetExceeded.exit_code(), 2);
}

#[tokio::test]
async fn completion_output_keeps_the_reported_price() {
    let (mut session, tx) = mock_session();
    tx.send(Ok(result_msg(false, 3, Some(0.75)))).unwrap();
    let mut out = Vec::new();
    drain_headless(
        &mut session,
        HeadlessFormat::Json,
        None,
        &mut out,
        &mut Vec::new(),
    )
    .await
    .unwrap();
    let rows: Vec<serde_json::Value> = String::from_utf8(out)
        .unwrap()
        .lines()
        .map(|line| serde_json::from_str(line).unwrap())
        .collect();
    let result = rows.iter().find(|row| row["type"] == "result").unwrap();
    assert_eq!(result["total_cost_usd"], 0.75);
    assert_eq!(result["num_turns"], 3);
}

#[tokio::test]
async fn a_stream_without_completion_reports_failure_without_a_result() {
    let (mut session, tx) = mock_session();
    tx.send(Ok(text_msg("no result follows"))).unwrap();
    drop(tx);
    let mut out = Vec::new();
    let outcome = drain_headless(
        &mut session,
        HeadlessFormat::Json,
        None,
        &mut out,
        &mut Vec::new(),
    )
    .await
    .unwrap();
    assert_eq!(outcome, HeadlessOutcome::AgentError);
    assert!(String::from_utf8(out).unwrap().lines().all(|line| {
        let row: serde_json::Value = serde_json::from_str(line).unwrap();
        row["type"] != "result"
    }));
}
