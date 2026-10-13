use super::*;
use serde_json::json;

#[test]
fn workspace_replies_show_source_and_edits_instead_of_envelope_metadata() {
    let output = display_output(&json!({"v":1,"ok":true,"budget":{"used":42},"results":[
        {"kind":"unit","file":"file.rs","contentHash":"hash"},
        {"kind":"lines","text":"fn main() {}"},
        {"path":"file.rs","action":"modified","applied":true}
    ]}));
    assert_eq!(output, "fn main() {}\nmodified: file.rs");
    assert!(!output.contains("budget"));
}

#[test]
fn checks_remain_visible_even_when_the_transport_call_succeeded() {
    let result = json!({"content":[],"structuredContent":{"name":"test","passed":false,"exit_code":100,"details":"2 passed, 1 failed"}});
    assert_eq!(
        display_output(&result),
        "Check test: failed (exit 100)\n2 passed, 1 failed"
    );
}

#[test]
fn structured_findings_do_not_repeat_the_entire_reasoning_record() {
    let result = json!({"step_number":1348,"estimated_total":6,"outcome_excerpt":"kept","recent_steps":[{"thought_excerpt":"old"}]});
    assert_eq!(display_output(&result), "Finding recorded.");
    assert_eq!(
        display_output(&json!({"ok":false,"message":"writer refused"})),
        "Failed: writer refused"
    );
}

#[test]
fn general_text_and_unrecognized_results_stay_available() {
    assert_eq!(
        display_output(&json!({"content":[{"type":"text","text":"file contents"}]})),
        "file contents"
    );
    assert!(display_output(&json!({"custom":"result"})).contains("result"));
}
