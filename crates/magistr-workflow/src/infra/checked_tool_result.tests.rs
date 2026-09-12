use super::*;
use crate::infra::{tool_failure::ToolFailure, tool_result};
use rmcp::model::ContentBlock;
use serde_json::json;

#[test]
fn error_without_structured_content_preserves_original_diagnostic() {
    let original = CallToolResult::error(vec![ContentBlock::text("disk sync failed after rename")]);
    let error = CheckedToolResult::body(&original).unwrap_err();
    assert_eq!(*error, original);
}

#[test]
fn success_supports_json_text_and_checks_every_failure_shape() {
    let success = CallToolResult::success(vec![ContentBlock::text(r#"{"step_number":7}"#)]);
    assert_eq!(CheckedToolResult::body(&success).unwrap()["step_number"], 7);
    for body in [
        json!({"ok":false,"message":"legacy refusal"}),
        json!({"status":"failed","error":"native refusal"}),
    ] {
        let original = CallToolResult::structured(body.clone());
        let error = CheckedToolResult::body(&original).unwrap_err();
        assert_eq!(error.is_error, Some(true));
        assert_eq!(error.structured_content, Some(body));
        assert_eq!(error.content, original.content);
    }
}

#[test]
fn malformed_success_keeps_original_response_for_read_back() {
    let original = CallToolResult::success(vec![ContentBlock::text("response interrupted")]);
    let error = CheckedToolResult::body(&original).unwrap_err();
    let body = error.structured_content.as_ref().unwrap();
    assert_eq!(body["effect"], "unknown");
    assert_eq!(body["recovery"]["action"], "read_back");
    assert_eq!(
        body["original_result"],
        serde_json::to_value(original).unwrap()
    );
}

#[test]
fn uncertainty_stops_dependent_write_but_not_an_independent_call() {
    let writes = std::cell::Cell::new(0);
    // The upstream fixture applies its effect, then loses its receipt.
    writes.set(writes.get() + 1);
    let original = ToolFailure::uncertain("receipt_lost", "write applied; receipt unavailable");
    let chain = CheckedToolResult::body(&original).map(|body| {
        writes.set(writes.get() + 1);
        body["step_number"].clone()
    });
    assert!(chain.is_err());
    assert_eq!(writes.get(), 1, "no dependent write or blind retry");
    assert!(CheckedToolResult::body(&tool_result::ok(json!({"independent":true}))).is_ok());
}

#[test]
fn recovery_actions_distinguish_correction_transience_and_unknown_effects() {
    let invalid = ToolFailure::rejected(
        "invalid_args",
        "outcome is blank",
        vec![json!({"field":"outcome","message":"Supply a nonblank string."})],
    );
    let transient = ToolFailure::transient("engine_busy", "queue is full; nothing started", 50);
    let uncertain = ToolFailure::uncertain("storage_failed", "write settlement is unknown");
    for result in [&invalid, &transient, &uncertain] {
        assert_eq!(result.is_error, Some(true));
        let text: Value = serde_json::from_str(&result.content[0].as_text().unwrap().text).unwrap();
        assert_eq!(Some(text), result.structured_content);
    }
    let invalid = invalid.structured_content.unwrap();
    assert_eq!(invalid["effect"], "none");
    assert_eq!(invalid["recovery"]["action"], "correct_arguments");
    let transient = transient.structured_content.unwrap();
    assert_eq!(transient["effect"], "none");
    assert_eq!(transient["recovery"]["action"], "retry_bounded");
    assert_eq!(transient["recovery"]["max_attempts"], 3);
    assert_eq!(transient["recovery"]["backoff_ms"], 50);
    let uncertain = uncertain.structured_content.unwrap();
    assert_eq!(uncertain["effect"], "unknown");
    assert_eq!(uncertain["recovery"]["action"], "read_back");
}
