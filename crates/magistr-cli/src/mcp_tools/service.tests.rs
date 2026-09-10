use super::*;

#[test]
fn the_unrecorded_note_carries_the_reason() {
    let note = unrecorded_note("no task_id provided and no active task");
    assert!(note.starts_with("not recorded: "), "{note}");
    assert!(note.contains("no active task"), "{note}");
    // The remedy stays on the note — the agent must know what to do next.
    assert!(note.contains("ship_start"), "{note}");
}

/// The junit failures a record parsed become rows once each: a test the
/// output scan already named is not repeated, one it missed is added with
/// its message's first line.
#[test]
fn junit_failures_become_rows_once() {
    let check: Check = serde_json::from_value(json!({
        "type": "test", "name": "test", "passed": false, "details": "", "required": true,
        "verified": true, "exit_code": 100, "timestamp": "2026-09-10T00:00:00Z",
        "results": {"total": 3, "passed": 1, "failed": 2, "skipped": 0, "failures": [
            {"name": "a::seen", "message": "assertion failed\nleft: 1"},
            {"name": "b::unseen", "message": "panicked at x\nmore"},
            {"name": "c::terse"}
        ]}
    }))
    .expect("a check record");
    let mut rows = vec![magistr_core::GateFailureRow {
        severity: magistr_core::GateRowSeverity::TestFailure,
        code: None,
        message: "test a::seen ... FAILED".to_owned(),
        file: None,
        line: None,
        column: None,
    }];
    junit_rows(&check, &mut rows);
    let messages: Vec<&str> = rows.iter().map(|row| row.message.as_str()).collect();
    assert_eq!(
        messages,
        vec![
            "test a::seen ... FAILED",
            "b::unseen: panicked at x",
            "c::terse"
        ]
    );
}
