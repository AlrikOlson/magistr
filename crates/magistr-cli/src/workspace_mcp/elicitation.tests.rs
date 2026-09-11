use super::*;
use serde_json::json;

#[test]
fn the_message_is_the_sealed_error_plus_the_capped_notes() {
    let sealed = json!({
        "ok": false,
        "error": {"code": "CONFIRM_REQUIRED", "message": "2 files would change"},
        "notes": ["--- a/x", "+++ b/x"]
    });
    assert_eq!(
        message(&sealed),
        "2 files would change\n\n--- a/x\n+++ b/x\n\nApply these changes?"
    );
    let long = "x".repeat(ELICIT_DIFF_CHARS + 50);
    let capped = message(&json!({"ok": false, "error": {"message": "m"}, "notes": [long]}));
    assert_eq!(
        capped.len(),
        "m\n\n".len() + ELICIT_DIFF_CHARS + "\n\nApply these changes?".len()
    );
}

#[test]
fn a_sealed_success_yields_an_empty_question_body() {
    assert_eq!(
        message(&json!({"ok": true, "results": []})),
        "\n\n\n\nApply these changes?"
    );
}
