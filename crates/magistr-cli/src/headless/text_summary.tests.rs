use super::*;

#[test]
fn emit_gate_pass_shows_checkmark() {
    let renderer = TextSummaryRenderer::new();
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
    assert!(output.contains("[PASS]"));
    assert!(output.contains("check"));
    assert!(output.contains("1.2s"));
}

#[test]
fn emit_gate_fail_shows_cross() {
    let renderer = TextSummaryRenderer::new();
    let mut buf = Vec::new();
    renderer
        .emit(
            &mut buf,
            HeadlessEvent::GateFail {
                name: "test".into(),
                duration_ms: 5678,
                output: Some("3 tests failed".into()),
            },
        )
        .unwrap();
    let output = String::from_utf8(buf).unwrap();
    assert!(output.contains("[FAIL]"));
    assert!(output.contains("test"));
    assert!(output.contains("5.7s"));
    assert!(output.contains("3 tests failed"));
}

#[test]
fn emit_error_shows_error_prefix() {
    let renderer = TextSummaryRenderer::new();
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
    assert!(output.contains("ERROR:"));
    assert!(output.contains("something broke"));
}

#[test]
fn emit_session_end_shows_summary() {
    let renderer = TextSummaryRenderer::new();
    let mut buf = Vec::new();
    renderer
        .emit(
            &mut buf,
            HeadlessEvent::SessionEnd {
                duration_ms: 60_000,
                cost_usd: Some(0.42),
                exit_code: 0,
            },
        )
        .unwrap();
    let output = String::from_utf8(buf).unwrap();
    assert!(output.contains("60.0s"));
    assert!(output.contains("$0.42"));
}

#[test]
fn no_ansi_escape_codes_in_output() {
    let renderer = TextSummaryRenderer::new();
    let mut buf = Vec::new();
    renderer
        .emit(
            &mut buf,
            HeadlessEvent::GatePass {
                name: "lint".into(),
                duration_ms: 100,
            },
        )
        .unwrap();
    let output = String::from_utf8(buf).unwrap();
    // No ANSI escape sequences
    assert!(!output.contains("\x1b["));
}
