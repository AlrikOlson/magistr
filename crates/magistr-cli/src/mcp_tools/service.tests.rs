use super::*;

// ── check_record: what ship_check answered, read without losing the why ──

#[test]
fn a_real_check_record_is_the_record() {
    let result = CallToolResult::structured(json!({
        "name": "test", "passed": true, "verified": true, "exit_code": 0,
        "details": "ok"
    }));
    let record = check_record(&result).expect("a check record");
    assert_eq!(record["exit_code"], 0);
    assert_eq!(record["passed"], true);
}

#[test]
fn a_record_carried_as_text_is_still_the_record() {
    let text = json!({"name": "lint", "passed": false, "exit_code": 3}).to_string();
    let result = CallToolResult::success(vec![ContentBlock::text(text)]);
    let record = check_record(&result).expect("a check record");
    assert_eq!(record["exit_code"], 3);
}

#[test]
fn a_soft_refusal_names_its_message() {
    // think-and-ship's soft error: is_error stays false, the envelope says why.
    let result = CallToolResult::structured(json!({
        "ok": false,
        "error_kind": "invalid_state",
        "message": "no task_id provided and no active task"
    }));
    let reason = check_record(&result).expect_err("refused");
    assert_eq!(reason, "no task_id provided and no active task");
}

#[test]
fn a_soft_refusal_without_a_message_names_its_kind() {
    let result = CallToolResult::structured(json!({"ok": false, "error_kind": "invalid_args"}));
    let reason = check_record(&result).expect_err("refused");
    assert!(reason.contains("invalid_args"), "{reason}");
}

#[test]
fn an_mcp_error_result_names_its_text() {
    let result = CallToolResult::error(vec![ContentBlock::text("engine poisoned")]);
    let reason = check_record(&result).expect_err("errored");
    assert!(reason.contains("engine poisoned"), "{reason}");
}

#[test]
fn an_answer_that_is_not_a_check_record_says_so() {
    // No exit_code: whatever this is, it is not a command-run check.
    let result = CallToolResult::structured(json!({"name": "test", "passed": true}));
    let reason = check_record(&result).expect_err("not a record");
    assert!(reason.contains("no check record"), "{reason}");

    let empty = CallToolResult::success(vec![ContentBlock::text("not json")]);
    let reason = check_record(&empty).expect_err("not a record");
    assert!(reason.contains("no check record"), "{reason}");
}

#[test]
fn the_unrecorded_note_carries_the_reason() {
    let note = unrecorded_note("no task_id provided and no active task");
    assert!(note.starts_with("not recorded: "), "{note}");
    assert!(note.contains("no active task"), "{note}");
    // The remedy stays on the note — the agent must know what to do next.
    assert!(note.contains("ship_start"), "{note}");
}
