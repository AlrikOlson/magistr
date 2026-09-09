use super::*;

#[test]
fn emit_produces_valid_ndjson_line() {
    let renderer = JsonStreamRenderer::new();
    let mut buf = Vec::new();
    renderer
        .emit(
            &mut buf,
            HeadlessEvent::GatePass {
                name: "check".into(),
                duration_ms: 1234,
            },
        )
        .unwrap();
    let output = String::from_utf8(buf).unwrap();

    // Must end with exactly one newline (NDJSON)
    assert!(output.ends_with('\n'));
    assert!(!output.ends_with("\n\n"));

    // Must be valid JSON
    let parsed: serde_json::Value = serde_json::from_str(output.trim()).unwrap();
    assert_eq!(parsed["event"], "gate_pass");
    assert_eq!(parsed["data"]["name"], "check");
    assert_eq!(parsed["data"]["duration_ms"], 1234);
    assert!(parsed["timestamp"].is_string());
}

#[test]
fn emit_gate_fail_includes_output() {
    let renderer = JsonStreamRenderer::new();
    let mut buf = Vec::new();
    renderer
        .emit(
            &mut buf,
            HeadlessEvent::GateFail {
                name: "test".into(),
                duration_ms: 500,
                output: Some("assertion failed".into()),
            },
        )
        .unwrap();
    let output = String::from_utf8(buf).unwrap();
    let parsed: serde_json::Value = serde_json::from_str(output.trim()).unwrap();
    assert_eq!(parsed["event"], "gate_fail");
    assert_eq!(parsed["data"]["output"], "assertion failed");
}

#[test]
fn emit_error_event() {
    let renderer = JsonStreamRenderer::new();
    let mut buf = Vec::new();
    renderer
        .emit(
            &mut buf,
            HeadlessEvent::Error {
                message: "something broke".into(),
            },
        )
        .unwrap();
    let output = String::from_utf8(buf).unwrap();
    let parsed: serde_json::Value = serde_json::from_str(output.trim()).unwrap();
    assert_eq!(parsed["event"], "error");
    assert_eq!(parsed["data"]["message"], "something broke");
}

#[test]
fn multiple_events_produce_multiple_lines() {
    let renderer = JsonStreamRenderer::new();
    let mut buf = Vec::new();
    renderer
        .emit(
            &mut buf,
            HeadlessEvent::GateStart {
                name: "check".into(),
            },
        )
        .unwrap();
    renderer
        .emit(
            &mut buf,
            HeadlessEvent::GatePass {
                name: "check".into(),
                duration_ms: 100,
            },
        )
        .unwrap();
    let output = String::from_utf8(buf).unwrap();
    let lines: Vec<&str> = output.trim().lines().collect();
    assert_eq!(lines.len(), 2);
    // Each line is valid JSON
    for line in &lines {
        serde_json::from_str::<serde_json::Value>(line).unwrap();
    }
}
