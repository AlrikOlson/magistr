use super::*;
use crate::plugin::types::{PluginRequest, PluginResponse};
use std::time::Duration;

#[test]
fn check_severity_from_str_lossy() {
    assert_eq!(CheckSeverity::from_str_lossy("error"), CheckSeverity::Error);
    assert_eq!(CheckSeverity::from_str_lossy("err"), CheckSeverity::Error);
    assert_eq!(
        CheckSeverity::from_str_lossy("critical"),
        CheckSeverity::Error
    );
    assert_eq!(
        CheckSeverity::from_str_lossy("blocker"),
        CheckSeverity::Error
    );
    assert_eq!(
        CheckSeverity::from_str_lossy("warning"),
        CheckSeverity::Warning
    );
    assert_eq!(
        CheckSeverity::from_str_lossy("warn"),
        CheckSeverity::Warning
    );
    assert_eq!(
        CheckSeverity::from_str_lossy("major"),
        CheckSeverity::Warning
    );
    assert_eq!(
        CheckSeverity::from_str_lossy("minor"),
        CheckSeverity::Warning
    );
    assert_eq!(CheckSeverity::from_str_lossy("info"), CheckSeverity::Info);
    assert_eq!(
        CheckSeverity::from_str_lossy("something_else"),
        CheckSeverity::Info
    );
}

#[test]
fn check_severity_label() {
    assert_eq!(CheckSeverity::Error.label(), "error");
    assert_eq!(CheckSeverity::Warning.label(), "warning");
    assert_eq!(CheckSeverity::Info.label(), "info");
}

#[test]
fn check_severity_display() {
    assert_eq!(format!("{}", CheckSeverity::Error), "error");
    assert_eq!(format!("{}", CheckSeverity::Warning), "warning");
    assert_eq!(format!("{}", CheckSeverity::Info), "info");
}

#[test]
fn check_outcome_count_by_severity() {
    let outcome = PluginCheckOutcome {
        passed: false,
        check_name: "test".to_string(),
        findings: vec![
            CheckFinding {
                file: Some("src/main.rs".to_string()),
                line: Some(10),
                message: "error 1".to_string(),
                severity: CheckSeverity::Error,
                rule_id: Some("E001".to_string()),
            },
            CheckFinding {
                file: Some("src/main.rs".to_string()),
                line: Some(20),
                message: "error 2".to_string(),
                severity: CheckSeverity::Error,
                rule_id: None,
            },
            CheckFinding {
                file: None,
                line: None,
                message: "warning 1".to_string(),
                severity: CheckSeverity::Warning,
                rule_id: None,
            },
            CheckFinding {
                file: None,
                line: None,
                message: "info 1".to_string(),
                severity: CheckSeverity::Info,
                rule_id: None,
            },
        ],
        summary: "2 errors, 1 warning, 1 info".to_string(),
        duration: Duration::from_secs(1),
    };

    assert_eq!(outcome.error_count(), 2);
    assert_eq!(outcome.warning_count(), 1);
    assert_eq!(outcome.count_by_severity(CheckSeverity::Info), 1);
}

#[test]
fn check_outcome_empty_findings() {
    let outcome = PluginCheckOutcome {
        passed: true,
        check_name: "clean".to_string(),
        findings: vec![],
        summary: "All clear".to_string(),
        duration: Duration::from_millis(100),
    };

    assert!(outcome.passed);
    assert_eq!(outcome.error_count(), 0);
    assert_eq!(outcome.warning_count(), 0);
}

#[test]
fn check_config_fields() {
    let config = CheckPluginConfig {
        check_name: "sonarqube".to_string(),
        project_dir: "/tmp/project".to_string(),
        config: Some(serde_json::json!({"server_url": "http://localhost:9000"})),
    };

    assert_eq!(config.check_name, "sonarqube");
    assert!(config.config.is_some());
}

#[test]
fn protocol_check_run_request_serializes() {
    let request = PluginRequest::CheckRun {
        id: 1,
        check_name: "sonarqube".to_string(),
        project_dir: "/tmp/project".to_string(),
        config: Some(serde_json::json!({"server_url": "http://localhost:9000"})),
    };

    let json = serde_json::to_string(&request).expect("serialize");
    assert!(json.contains("check.run"));
    assert!(json.contains("sonarqube"));
    assert_eq!(request.id(), 1);
}

#[test]
fn protocol_check_run_request_without_config() {
    let request = PluginRequest::CheckRun {
        id: 2,
        check_name: "license".to_string(),
        project_dir: "/tmp/project".to_string(),
        config: None,
    };

    let json = serde_json::to_string(&request).expect("serialize");
    assert!(!json.contains("config"));
}

#[test]
fn protocol_check_finding_response_roundtrip() {
    let response = PluginResponse::CheckFinding {
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
fn protocol_check_done_response_roundtrip() {
    let response = PluginResponse::CheckDone {
        id: 1,
        passed: false,
        summary: "3 issues found".to_string(),
    };

    let json = serde_json::to_string(&response).expect("serialize");
    let parsed: PluginResponse = serde_json::from_str(&json).expect("deserialize");
    assert_eq!(parsed, response);
}

#[test]
fn protocol_check_finding_minimal_fields() {
    let response = PluginResponse::CheckFinding {
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
