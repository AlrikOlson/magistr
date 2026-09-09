use super::*;

#[test]
fn event_serializes_with_tag() {
    let event = HeadlessEvent::GatePass {
        name: "check".into(),
        duration_ms: 100,
    };
    let json = serde_json::to_value(&event).unwrap();
    assert_eq!(json["event"], "gate_pass");
    assert_eq!(json["data"]["name"], "check");
}

#[test]
fn error_event_serializes() {
    let event = HeadlessEvent::Error {
        message: "boom".into(),
    };
    let json = serde_json::to_value(&event).unwrap();
    assert_eq!(json["event"], "error");
    assert_eq!(json["data"]["message"], "boom");
}
