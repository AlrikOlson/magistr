use std::future::pending;

use rmcp::model::{CallToolResult, ContentBlock};
use serde_json::json;

use super::settle;

#[test]
fn a_structured_answer_is_the_value() {
    let value = settle(async { Ok(CallToolResult::structured(json!({"chunk": {"id": "c"}}))) })
        .expect("answer");
    assert_eq!(value["chunk"]["id"], "c");
}

#[test]
fn a_soft_refusal_is_its_message_and_an_mcp_error_its_text() {
    let refused = settle(async {
        Ok(CallToolResult::structured(json!({
            "ok": false, "error_kind": "invalid_state", "message": "no chunk 'x'"
        })))
    })
    .expect_err("refused");
    assert_eq!(refused, "no chunk 'x'");

    let kind_only = settle(async {
        Ok(CallToolResult::structured(json!({
            "ok": false, "error_kind": "not_found", "message": ""
        })))
    })
    .expect_err("refused");
    assert_eq!(kind_only, "refused (not_found)");

    let errored = settle(async {
        Ok(CallToolResult::error(vec![ContentBlock::text(
            "the trace refused the step",
        )]))
    })
    .expect_err("errored");
    assert_eq!(errored, "the trace refused the step");

    let mcp = settle(async {
        Err(rmcp::ErrorData::internal_error(
            "engine mutex poisoned",
            None,
        ))
    })
    .expect_err("mcp error");
    assert_eq!(mcp, "engine mutex poisoned");
}

#[test]
fn prose_without_structure_comes_back_as_text() {
    let value = settle(async { Ok(CallToolResult::success(vec![ContentBlock::text("plain")])) })
        .expect("answer");
    assert_eq!(value, serde_json::Value::String("plain".to_owned()));
}

#[test]
fn a_handler_that_suspends_is_refused_not_spun() {
    let refused = settle(pending::<Result<CallToolResult, rmcp::ErrorData>>())
        .expect_err("a pending handler is a contract break");
    assert!(refused.contains("suspended"), "{refused}");
}
