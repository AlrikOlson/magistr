use super::*;

#[test]
fn emit_gate_fail_produces_error_annotation() {
    let renderer = GhaRenderer::new();
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
    assert!(output.contains("::error::"));
    assert!(output.contains("test"));
}

#[test]
fn emit_gate_pass_produces_notice() {
    let renderer = GhaRenderer::new();
    let mut buf = Vec::new();
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
    assert!(output.contains("::notice::"));
    assert!(output.contains("check"));
}

#[test]
fn emit_error_produces_error_annotation() {
    let renderer = GhaRenderer::new();
    let mut buf = Vec::new();
    renderer
        .emit(
            &mut buf,
            HeadlessEvent::Error {
                message: "runtime failure".into(),
            },
        )
        .unwrap();
    let output = String::from_utf8(buf).unwrap();
    assert!(output.contains("::error::"));
    assert!(output.contains("runtime failure"));
}

#[test]
fn emit_gate_start_uses_group() {
    let renderer = GhaRenderer::new();
    let mut buf = Vec::new();
    renderer
        .emit(
            &mut buf,
            HeadlessEvent::GateStart {
                name: "lint".into(),
            },
        )
        .unwrap();
    let output = String::from_utf8(buf).unwrap();
    assert!(output.contains("::group::"));
    assert!(output.contains("lint"));
}

#[test]
fn emit_budget_exceeded_produces_warning() {
    let renderer = GhaRenderer::new();
    let mut buf = Vec::new();
    renderer
        .emit(
            &mut buf,
            HeadlessEvent::BudgetExceeded {
                limit_usd: 5.0,
                spent_usd: 5.12,
            },
        )
        .unwrap();
    let output = String::from_utf8(buf).unwrap();
    assert!(output.contains("::warning::"));
    assert!(output.contains("Budget"));
}
