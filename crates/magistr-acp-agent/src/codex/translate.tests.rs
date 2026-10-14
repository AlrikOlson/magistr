use super::*;
use serde_json::json;

#[test]
fn mcp_tools_publish_actual_arguments_results_and_failure() {
    let mut translator = Translator::default();
    let result = json!({"content":[{"type":"text","text":"check failed"}],"structuredContent":{"name":"check","passed":false,"exit_code":1}});
    let item = json!({"id":"gate","type":"mcpToolCall","server":"magistr","tool":"ship_check","arguments":{"name":"check","command":"cargo check"},"status":"failed","result":result});
    let updates = translator
        .notification("item/completed", &json!({"item":item}))
        .unwrap();
    let SessionUpdate::ToolCall(call) = &updates[0] else {
        panic!("tool call")
    };
    assert_eq!(call.raw_input, Some(item["arguments"].clone()));
    let SessionUpdate::ToolCallUpdate(update) = updates.last().unwrap() else {
        panic!("update")
    };
    assert_eq!(update.fields.raw_output, Some(result));
    assert_eq!(update.fields.status, Some(ToolCallStatus::Failed));
    assert!(
        matches!(&update.fields.content.as_ref().unwrap()[0], agent_client_protocol::schema::v1::ToolCallContent::Content(c) if matches!(&c.content, ContentBlock::Text(t) if t.text == "check failed"))
    );
}

#[test]
fn dynamic_tool_failure_and_text_are_reported() {
    let mut translator = Translator::default();
    let item = json!({"id":"dynamic","type":"dynamicToolCall","namespace":"functions","tool":"exec","arguments":{"code":"inspect()"},"status":"completed","success":false,"contentItems":[{"type":"inputText","text":"permission denied"}]});
    let updates = translator
        .notification("item/completed", &json!({"item":item}))
        .unwrap();
    let SessionUpdate::ToolCall(call) = &updates[0] else {
        panic!("tool call")
    };
    assert_eq!(call.title, "functions.exec");
    assert_eq!(call.raw_input, Some(item["arguments"].clone()));
    let SessionUpdate::ToolCallUpdate(update) = updates.last().unwrap() else {
        panic!("update")
    };
    assert_eq!(update.fields.status, Some(ToolCallStatus::Failed));
    assert!(
        matches!(&update.fields.content.as_ref().unwrap()[0], agent_client_protocol::schema::v1::ToolCallContent::Content(c) if matches!(&c.content, ContentBlock::Text(t) if t.text == "permission denied"))
    );
}

#[test]
fn command_output_updates_are_complete_current_snapshots() {
    let mut translator = Translator::default();
    for (delta, expected) in [("first\n", "first\n"), ("second\n", "first\nsecond\n")] {
        let updates = translator
            .notification(
                "item/commandExecution/outputDelta",
                &json!({"itemId":"cmd","delta":delta}),
            )
            .unwrap();
        let SessionUpdate::ToolCallUpdate(update) = &updates[0] else {
            panic!("update")
        };
        assert!(
            matches!(&update.fields.content.as_ref().unwrap()[0], agent_client_protocol::schema::v1::ToolCallContent::Content(c) if matches!(&c.content, ContentBlock::Text(t) if t.text == expected))
        );
    }
}

#[test]
fn completed_messages_do_not_duplicate_streamed_text_per_item() {
    let mut translator = Translator::default();
    assert_eq!(
        translator
            .notification(
                "item/agentMessage/delta",
                &json!({"itemId":"a","delta":"hello"})
            )
            .expect("delta")
            .len(),
        1
    );
    assert!(
        translator
            .notification(
                "item/completed",
                &json!({"item":{"id":"a","type":"agentMessage","text":"hello"}})
            )
            .expect("completion")
            .is_empty()
    );
    assert_eq!(
        translator
            .notification(
                "item/completed",
                &json!({"item":{"id":"b","type":"agentMessage","text":"second"}})
            )
            .expect("second message")
            .len(),
        1
    );
}

#[test]
fn command_nonzero_exit_and_decline_are_failed_tool_results() {
    let mut translator = Translator::default();
    for item in [
        json!({"id":"a","type":"commandExecution","command":"false","status":"completed","exitCode":1}),
        json!({"id":"b","type":"commandExecution","command":"write","status":"declined"}),
    ] {
        let updates = translator
            .notification("item/completed", &json!({"item":item}))
            .expect("tool");
        let SessionUpdate::ToolCallUpdate(update) = updates.last().expect("update") else {
            panic!("tool update")
        };
        assert_eq!(update.fields.status, Some(ToolCallStatus::Failed));
    }
}

#[test]
fn malformed_known_delta_is_reported() {
    assert!(
        Translator::default()
            .notification("item/agentMessage/delta", &json!({"itemId":"a"}))
            .is_err()
    );
}
