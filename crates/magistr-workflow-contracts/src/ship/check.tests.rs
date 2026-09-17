use super::*;
#[test]
fn legacy_checks_never_gain_verified_evidence() {
    let check: Check = serde_json::from_value(serde_json::json!({
        "type":"test","name":"unit","passed":true,"details":"","required":true,"timestamp":"then"
    }))
    .unwrap();
    assert!(!check.verified);
    assert!(check.command.is_none() && check.report.is_none() && check.results.is_none());
    assert_eq!(serde_json::to_value(check).unwrap()["verified"], false);
}
