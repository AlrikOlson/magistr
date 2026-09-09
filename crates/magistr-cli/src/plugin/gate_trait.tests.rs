use super::*;
use crate::plugin::types::{PluginRequest, PluginResponse};
use std::time::Duration;

#[test]
fn gate_severity_from_str_lossy() {
    assert_eq!(GateSeverity::from_str_lossy("error"), GateSeverity::Error);
    assert_eq!(GateSeverity::from_str_lossy("err"), GateSeverity::Error);
    assert_eq!(
        GateSeverity::from_str_lossy("critical"),
        GateSeverity::Error
    );
    assert_eq!(GateSeverity::from_str_lossy("blocker"), GateSeverity::Error);
    assert_eq!(
        GateSeverity::from_str_lossy("warning"),
        GateSeverity::Warning
    );
    assert_eq!(GateSeverity::from_str_lossy("warn"), GateSeverity::Warning);
    assert_eq!(GateSeverity::from_str_lossy("major"), GateSeverity::Warning);
    assert_eq!(GateSeverity::from_str_lossy("minor"), GateSeverity::Warning);
    assert_eq!(GateSeverity::from_str_lossy("info"), GateSeverity::Info);
    assert_eq!(
        GateSeverity::from_str_lossy("something_else"),
        GateSeverity::Info
    );
}

#[test]
fn gate_severity_label() {
    assert_eq!(GateSeverity::Error.label(), "error");
    assert_eq!(GateSeverity::Warning.label(), "warning");
    assert_eq!(GateSeverity::Info.label(), "info");
}

#[test]
fn gate_severity_display() {
    assert_eq!(format!("{}", GateSeverity::Error), "error");
    assert_eq!(format!("{}", GateSeverity::Warning), "warning");
    assert_eq!(format!("{}", GateSeverity::Info), "info");
}

#[test]
fn gate_outcome_count_by_severity() {
    let outcome = GateOutcome {
        passed: false,
        gate_name: "test".to_string(),
        findings: vec![
            GateFinding {
                file: Some("src/main.rs".to_string()),
                line: Some(10),
                message: "error 1".to_string(),
                severity: GateSeverity::Error,
                rule_id: Some("E001".to_string()),
            },
            GateFinding {
                file: Some("src/main.rs".to_string()),
                line: Some(20),
                message: "error 2".to_string(),
                severity: GateSeverity::Error,
                rule_id: None,
            },
            GateFinding {
                file: None,
                line: None,
                message: "warning 1".to_string(),
                severity: GateSeverity::Warning,
                rule_id: None,
            },
            GateFinding {
                file: None,
                line: None,
                message: "info 1".to_string(),
                severity: GateSeverity::Info,
                rule_id: None,
            },
        ],
        summary: "2 errors, 1 warning, 1 info".to_string(),
        duration: Duration::from_secs(1),
    };

    assert_eq!(outcome.error_count(), 2);
    assert_eq!(outcome.warning_count(), 1);
    assert_eq!(outcome.count_by_severity(GateSeverity::Info), 1);
}

#[test]
fn gate_outcome_empty_findings() {
    let outcome = GateOutcome {
        passed: true,
        gate_name: "clean".to_string(),
        findings: vec![],
        summary: "All clear".to_string(),
        duration: Duration::from_millis(100),
    };

    assert!(outcome.passed);
    assert_eq!(outcome.error_count(), 0);
    assert_eq!(outcome.warning_count(), 0);
}

#[test]
fn gate_config_fields() {
    let config = GateConfig {
        gate_name: "sonarqube".to_string(),
        project_dir: "/tmp/project".to_string(),
        config: Some(serde_json::json!({"server_url": "http://localhost:9000"})),
    };

    assert_eq!(config.gate_name, "sonarqube");
    assert!(config.config.is_some());
}

#[test]
fn protocol_gate_run_request_serializes() {
    let request = PluginRequest::GateRun {
        id: 1,
        gate_name: "sonarqube".to_string(),
        project_dir: "/tmp/project".to_string(),
        config: Some(serde_json::json!({"server_url": "http://localhost:9000"})),
    };

    let json = serde_json::to_string(&request).expect("serialize");
    assert!(json.contains("gate.run"));
    assert!(json.contains("sonarqube"));
    assert_eq!(request.id(), 1);
}

#[test]
fn protocol_gate_run_request_without_config() {
    let request = PluginRequest::GateRun {
        id: 2,
        gate_name: "license".to_string(),
        project_dir: "/tmp/project".to_string(),
        config: None,
    };

    let json = serde_json::to_string(&request).expect("serialize");
    assert!(!json.contains("config"));
}

#[test]
fn protocol_gate_finding_response_roundtrip() {
    let response = PluginResponse::GateFinding {
        id: 1,
        file: Some("src/main.rs".to_string()),
        line: Some(42),
        message: "Unused variable".to_string(),
        severity: "warning".to_string(),
        rule_id: Some("S1234".to_string()),
    };

    let json = serde_json::to_string(&response).expect("serialize");
    let parsed: PluginResponse = serde_json::from_str(&json).expect("deserialize");
    assert_eq!(parsed, response);
    assert_eq!(parsed.id(), 1);
}

#[test]
fn protocol_gate_done_response_roundtrip() {
    let response = PluginResponse::GateDone {
        id: 1,
        passed: false,
        summary: "3 issues found".to_string(),
    };

    let json = serde_json::to_string(&response).expect("serialize");
    let parsed: PluginResponse = serde_json::from_str(&json).expect("deserialize");
    assert_eq!(parsed, response);
}

#[test]
fn protocol_gate_finding_minimal_fields() {
    let response = PluginResponse::GateFinding {
        id: 5,
        file: None,
        line: None,
        message: "Global issue".to_string(),
        severity: "info".to_string(),
        rule_id: None,
    };

    let json = serde_json::to_string(&response).expect("serialize");
    // Optional fields should be absent in serialized form
    assert!(!json.contains("file"));
    assert!(!json.contains("line"));
    assert!(!json.contains("rule_id"));

    let parsed: PluginResponse = serde_json::from_str(&json).expect("deserialize");
    assert_eq!(parsed, response);
}
