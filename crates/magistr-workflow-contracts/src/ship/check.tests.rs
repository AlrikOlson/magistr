use super::*;

#[test]
fn current_check_round_trip_preserves_canonical_receipt_link_and_execution() {
    let value = serde_json::json!({
        "receipt_id":"attempt-42", "type":"test", "name":"unit", "passed":true,
        "details":"finished", "required":true, "verified":true, "command":"cargo test",
        "exit_code":0, "timestamp":"2026-10-14T12:00:00Z"
    });
    let check: Check = serde_json::from_value(value.clone()).unwrap();
    assert!(check.verified);
    assert_eq!(check.receipt_id.as_deref(), Some("attempt-42"));
    assert_eq!(serde_json::to_value(check).unwrap(), value);
}
