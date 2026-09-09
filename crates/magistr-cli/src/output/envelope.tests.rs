use super::*;

#[test]
fn success_envelope_serializes() {
    let data = serde_json::json!({"language": "rust"});
    let envelope = success(&data);
    let json = serde_json::to_string(&envelope).unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
    assert_eq!(parsed["status"], "ok");
    assert_eq!(parsed["data"]["language"], "rust");
}

#[test]
fn error_envelope_serializes() {
    let envelope = error("something went wrong");
    let json = serde_json::to_string(&envelope).unwrap();
    let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
    assert_eq!(parsed["status"], "error");
    assert_eq!(parsed["error"], "something went wrong");
}

#[test]
fn validation_output_from_result() {
    use crate::validate::{Gate, GateResult, ValidationResult};
    use std::time::Duration;

    let result = ValidationResult {
        results: vec![
            GateResult {
                gate: Gate::Check,
                passed: true,
                duration: Duration::from_millis(150),
                output: String::new(),
                command: "cargo check".into(),
            },
            GateResult {
                gate: Gate::Test,
                passed: false,
                duration: Duration::from_millis(300),
                output: "test failed".into(),
                command: "cargo test".into(),
            },
        ],
    };

    let output = ValidationOutput::from(&result);
    assert!(!output.all_passed);
    assert_eq!(output.gates.len(), 2);
    assert!(output.gates[0].passed);
    assert!(!output.gates[1].passed);
    assert_eq!(output.gates[0].name, "check");
    assert_eq!(output.gates[1].name, "test");
}
