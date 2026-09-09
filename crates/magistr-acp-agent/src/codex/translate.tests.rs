use super::*;
use serde_json::json;

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
