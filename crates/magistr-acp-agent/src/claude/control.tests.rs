use super::*;

#[test]
fn the_three_requests_serialise_as_the_cli_expects() {
    assert_eq!(
        ControlRequest::Initialize.frame("init-1"),
        serde_json::json!({"type":"control_request","request_id":"init-1","request":{"subtype":"initialize"}})
    );
    assert_eq!(
        ControlRequest::Interrupt.frame("int-1"),
        serde_json::json!({"type":"control_request","request_id":"int-1","request":{"subtype":"interrupt"}})
    );
    assert_eq!(
        ControlRequest::SetModel(Some("sonnet".to_owned())).frame("sm-1"),
        serde_json::json!({"type":"control_request","request_id":"sm-1","request":{"subtype":"set_model","model":"sonnet"}})
    );
    assert_eq!(
        ControlRequest::SetModel(None).frame("sm-2")["request"]["model"],
        serde_json::Value::Null
    );
}

#[test]
fn responses_carry_the_request_id_as_success_or_error() {
    assert_eq!(
        ControlRequest::response("r-1", Ok(serde_json::json!({"behavior": "allow"}))),
        serde_json::json!({"type":"control_response","response":{"subtype":"success","request_id":"r-1","response":{"behavior":"allow"}}})
    );
    assert_eq!(
        ControlRequest::response("r-2", Err("unsupported: hook_callback".to_owned())),
        serde_json::json!({"type":"control_response","response":{"subtype":"error","request_id":"r-2","error":"unsupported: hook_callback"}})
    );
}

#[test]
fn a_user_message_is_one_text_block() {
    assert_eq!(
        ControlRequest::user_message("hello"),
        serde_json::json!({
            "type":"user",
            "message":{"role":"user","content":[{"type":"text","text":"hello"}]},
            "parent_tool_use_id":null,
            "session_id":""
        })
    );
}
