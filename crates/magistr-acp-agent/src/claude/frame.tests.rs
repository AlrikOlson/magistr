use super::*;

#[test]
fn a_non_json_line_and_an_unknown_type_are_other() {
    assert_eq!(Frame::parse("not json"), Frame::Other);
    assert_eq!(
        Frame::parse(r#"{"type":"rate_limit_event","x":1}"#),
        Frame::Other
    );
    assert_eq!(
        Frame::parse(r#"{"type":"system","subtype":"status","status":"requesting"}"#),
        Frame::Other
    );
}

#[test]
fn init_reads_the_session_the_model_and_the_mcp_statuses() {
    let frame = Frame::parse(
        r#"{"type":"system","subtype":"init","session_id":"s-1","model":"claude-sonnet-5",
            "mcp_servers":[{"name":"magistr","status":"connected"}],"permissionMode":"default",
            "tools":["Bash"],"unknown_future_field":{"a":1}}"#,
    );
    let Frame::Init(init) = frame else {
        panic!("{frame:?}");
    };
    assert_eq!(init.session_id, "s-1");
    assert_eq!(init.model, "claude-sonnet-5");
    assert_eq!(init.permission_mode, "default");
    assert_eq!(init.mcp_servers[0].name, "magistr");
    assert_eq!(init.mcp_servers[0].status, "connected");
}

#[test]
fn stream_events_are_read_by_delta_type() {
    let text = Frame::parse(
        r#"{"type":"stream_event","event":{"type":"content_block_delta","index":1,"delta":{"type":"text_delta","text":"po"}}}"#,
    );
    assert_eq!(
        text,
        Frame::Stream(StreamEvent::TextDelta {
            index: 1,
            text: "po".to_owned()
        })
    );
    let thinking = Frame::parse(
        r#"{"type":"stream_event","event":{"type":"content_block_delta","index":0,"delta":{"type":"thinking_delta","thinking":"hm"}}}"#,
    );
    assert_eq!(
        thinking,
        Frame::Stream(StreamEvent::ThinkingDelta {
            index: 0,
            thinking: "hm".to_owned()
        })
    );
    let start = Frame::parse(
        r#"{"type":"stream_event","event":{"type":"content_block_start","index":1,"content_block":{"type":"tool_use","id":"t1","name":"Bash","input":{}}}}"#,
    );
    assert_eq!(
        start,
        Frame::Stream(StreamEvent::BlockStart {
            index: 1,
            block: ContentBlock::ToolUse {
                id: "t1".to_owned(),
                name: "Bash".to_owned(),
                input: serde_json::json!({}),
            }
        })
    );
    let json_delta = Frame::parse(
        r#"{"type":"stream_event","event":{"type":"content_block_delta","index":1,"delta":{"type":"input_json_delta","partial_json":"{"}}}"#,
    );
    assert_eq!(json_delta, Frame::Stream(StreamEvent::Other));
    let stop = Frame::parse(
        r#"{"type":"stream_event","event":{"type":"message_delta","delta":{"stop_reason":"tool_use"}}}"#,
    );
    assert_eq!(
        stop,
        Frame::Stream(StreamEvent::MessageDelta {
            stop_reason: Some("tool_use".to_owned())
        })
    );
}

#[test]
fn assistant_and_user_blocks_are_typed_and_unknown_blocks_survive() {
    let assistant = Frame::parse(
        r#"{"type":"assistant","message":{"role":"assistant","content":[
            {"type":"thinking","thinking":"why","signature":"x"},
            {"type":"tool_use","id":"t1","name":"Write","input":{"file_path":"a.txt"}},
            {"type":"image","source":{}}]}}"#,
    );
    assert_eq!(
        assistant,
        Frame::Assistant(vec![
            ContentBlock::Thinking {
                thinking: "why".to_owned()
            },
            ContentBlock::ToolUse {
                id: "t1".to_owned(),
                name: "Write".to_owned(),
                input: serde_json::json!({"file_path": "a.txt"}),
            },
            ContentBlock::Other,
        ])
    );
    let user = Frame::parse(
        r#"{"type":"user","message":{"role":"user","content":[{"type":"tool_result","tool_use_id":"t1","content":"done","is_error":false}]}}"#,
    );
    assert_eq!(
        user,
        Frame::User(vec![ContentBlock::ToolResult {
            tool_use_id: "t1".to_owned(),
            content: serde_json::Value::String("done".to_owned()),
            is_error: false,
        }])
    );
    let note = Frame::parse(
        r#"{"type":"user","message":{"role":"user","content":"[Request interrupted by user]"}}"#,
    );
    assert_eq!(
        note,
        Frame::User(vec![ContentBlock::Text {
            text: "[Request interrupted by user]".to_owned()
        }])
    );
}

#[test]
fn tool_result_text_reads_strings_and_block_arrays() {
    assert_eq!(
        ContentBlock::result_text(&serde_json::json!("plain")),
        "plain"
    );
    assert_eq!(
        ContentBlock::result_text(
            &serde_json::json!([{"type":"text","text":"a"},{"type":"image"},{"type":"text","text":"b"}])
        ),
        "a\nb"
    );
    assert_eq!(ContentBlock::result_text(&serde_json::json!(null)), "");
}

#[test]
fn result_reads_usage_model_usage_and_cost_with_defaults_for_the_rest() {
    let frame = Frame::parse(
        r#"{"type":"result","subtype":"success","is_error":false,"stop_reason":"end_turn","result":"pong",
            "usage":{"input_tokens":2,"cache_creation_input_tokens":17576,"cache_read_input_tokens":19923,"output_tokens":4},
            "modelUsage":{"claude-sonnet-5":{"inputTokens":2,"contextWindow":200000,"costUSD":0.07}},
            "total_cost_usd":0.0752996,"permission_denials":[]}"#,
    );
    let Frame::Result(result) = frame else {
        panic!("{frame:?}");
    };
    assert_eq!(result.subtype, "success");
    assert!(!result.is_error);
    assert_eq!(result.stop_reason.as_deref(), Some("end_turn"));
    assert_eq!(result.usage.cache_creation_input_tokens, 17576);
    assert_eq!(
        result.model_usage["claude-sonnet-5"].context_window,
        200_000
    );
    assert!(result.errors.is_empty());
    let interrupted = Frame::parse(
        r#"{"type":"result","subtype":"error_during_execution","is_error":true,"stop_reason":null,"total_cost_usd":0.0009}"#,
    );
    let Frame::Result(result) = interrupted else {
        panic!("{interrupted:?}");
    };
    assert!(result.is_error);
    assert_eq!(result.stop_reason, None);
    assert_eq!(result.usage, Usage::default());
}

#[test]
fn control_frames_carry_their_request_id() {
    let request = Frame::parse(
        r#"{"type":"control_request","request_id":"r-9","request":{"subtype":"can_use_tool","tool_name":"Write","input":{}}}"#,
    );
    let Frame::ControlRequest(req) = request else {
        panic!("{request:?}");
    };
    assert_eq!(req.request_id, "r-9");
    assert_eq!(req.subtype, "can_use_tool");
    assert_eq!(req.request["tool_name"], "Write");

    let ok = Frame::parse(
        r#"{"type":"control_response","response":{"subtype":"success","request_id":"init-1","response":{"models":[]}}}"#,
    );
    let Frame::ControlResponse(res) = ok else {
        panic!("{ok:?}");
    };
    assert_eq!(res.request_id, "init-1");
    assert_eq!(res.error, None);
    assert_eq!(res.payload["models"], serde_json::json!([]));

    let failed = Frame::parse(
        r#"{"type":"control_response","response":{"subtype":"error","request_id":"sm-1","error":"no such model"}}"#,
    );
    let Frame::ControlResponse(res) = failed else {
        panic!("{failed:?}");
    };
    assert_eq!(res.error.as_deref(), Some("no such model"));
}
