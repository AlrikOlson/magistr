use super::*;
use agent_client_protocol::schema::v1::{ContentBlock, ToolCallStatus};
use serde_json::json;

fn text(update: &SessionUpdate) -> Option<(&str, &str)> {
    let (role, chunk) = match update {
        SessionUpdate::UserMessageChunk(chunk) => ("user", chunk),
        SessionUpdate::AgentMessageChunk(chunk) => ("agent", chunk),
        SessionUpdate::AgentThoughtChunk(chunk) => ("thought", chunk),
        _ => return None,
    };
    match &chunk.content {
        ContentBlock::Text(text) => Some((role, text.text.as_str())),
        _ => None,
    }
}

#[test]
fn recorded_resume_preserves_the_persisted_identity_model_and_message_order() {
    let response = include_str!("../../tests/fixtures/codex/thread_resume.ndjson")
        .lines()
        .map(|line| serde_json::from_str::<Value>(line).expect("recorded JSON"))
        .find_map(|frame| {
            frame["result"]
                .get("thread")
                .map(|_| frame["result"].clone())
        })
        .expect("recorded resume response");
    let record = ThreadRecord::parse(&response).expect("hydrated recording");
    assert_eq!(record.id, "00000000-0000-4000-8000-000000000103");
    assert_eq!(record.model.as_deref(), Some("gpt-5.6-sol"));
    assert_eq!(record.raw, response["thread"]);
    assert_eq!(
        record.replay.iter().filter_map(text).collect::<Vec<_>>(),
        vec![
            ("user", "Reply with exactly the single word: pong"),
            ("agent", "pong")
        ]
    );
}

#[test]
fn replay_keeps_turn_order_reasoning_and_failed_tool_outcomes() {
    let record = ThreadRecord::parse(&json!({"thread": {
        "id": "thread", "model": "persisted-model", "turns": [
            {"itemsView": "full", "items": [
                {"id": "user-1", "type": "userMessage", "content": [{"type": "text", "text": "first"}]},
                {"id": "reasoning", "type": "reasoning", "summary": ["considering"]},
                {"id": "command", "type": "commandExecution", "command": "false", "status": "completed", "exitCode": 1, "aggregatedOutput": "failure details"},
                {"id": "reply", "type": "agentMessage", "text": "first reply"}
            ]},
            {"itemsView": "full", "items": [
                {"id": "user-2", "type": "userMessage", "content": [{"type": "text", "text": "second"}]},
                {"id": "reply", "type": "agentMessage", "text": "second reply"}
            ]}
        ]
    }})).expect("hydrated history");
    assert_eq!(record.model.as_deref(), Some("persisted-model"));
    assert_eq!(
        record.replay.iter().filter_map(text).collect::<Vec<_>>(),
        vec![
            ("user", "first"),
            ("thought", "considering"),
            ("agent", "first reply"),
            ("user", "second"),
            ("agent", "second reply")
        ]
    );
    assert!(
        matches!(&record.replay[2], SessionUpdate::ToolCall(call) if call.tool_call_id.to_string() == "command")
    );
    assert!(
        matches!(&record.replay[3], SessionUpdate::ToolCallUpdate(update) if update.fields.status == Some(ToolCallStatus::Failed))
    );
}

#[test]
fn effective_response_model_takes_precedence_over_stored_metadata() {
    let record = ThreadRecord::parse(&json!({"model": "effective-model", "thread": {
        "id": "thread", "model": "stored-model", "turns": []
    }}))
    .expect("runtime response");
    assert_eq!(record.model.as_deref(), Some("effective-model"));
}

#[test]
fn incomplete_or_malformed_history_is_refused_instead_of_replaying_a_partial_record() {
    for response in [
        json!({"thread": {"turns": []}}),
        json!({"thread": {"id": "" , "turns": []}}),
        json!({"thread": {"id": "thread"}}),
        json!({"thread": {"id": "thread", "turns": [{}]}}),
        json!({"thread": {"id": "thread", "turns": [{"itemsView": "summary", "items": []}]}}),
        json!({"thread": {"id": "thread", "turns": [{"items": [{"type": "agentMessage", "text": "missing item id"}]}]}}),
    ] {
        assert!(
            ThreadRecord::parse(&response).is_err(),
            "must refuse {response}"
        );
    }
}
