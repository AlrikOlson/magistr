use super::*;

#[test]
fn sonarqube_config_defaults() {
    let config = SonarQubeConfig::default();
    assert_eq!(config.server_url, "http://localhost:9000");
    assert!(config.token.is_none());
    assert!(config.project_key.is_none());
    assert_eq!(config.scanner_path, "sonar-scanner");
}

#[test]
fn sonarqube_config_from_json() {
    let json = serde_json::json!({
        "server_url": "https://sonar.example.com",
        "token": "my-token",
        "project_key": "my-project",
        "scanner_path": "/usr/local/bin/sonar-scanner"
    });

    let config = SonarQubeConfig::from_json(&json);
    assert_eq!(config.server_url, "https://sonar.example.com");
    assert_eq!(config.token.as_deref(), Some("my-token"));
    assert_eq!(config.project_key.as_deref(), Some("my-project"));
    assert_eq!(config.scanner_path, "/usr/local/bin/sonar-scanner");
}

#[test]
fn sonarqube_config_from_partial_json() {
    let json = serde_json::json!({
        "server_url": "https://sonar.example.com"
    });

    let config = SonarQubeConfig::from_json(&json);
    assert_eq!(config.server_url, "https://sonar.example.com");
    assert!(config.token.is_none());
    assert!(config.project_key.is_none());
    assert_eq!(config.scanner_path, "sonar-scanner");
}

#[test]
fn parse_issue_line_error() {
    let finding = SonarQubeGate::parse_issue_line("ERROR: src/main.rs:42 - Unused variable");
    let finding = finding.expect("should parse");
    assert_eq!(finding.severity, GateSeverity::Error);
    assert_eq!(finding.file.as_deref(), Some("src/main.rs"));
    assert_eq!(finding.line, Some(42));
    assert_eq!(finding.message, "Unused variable");
}

#[test]
fn parse_issue_line_warning() {
    let finding = SonarQubeGate::parse_issue_line("WARN: src/lib.rs:10 - Complex method");
    let finding = finding.expect("should parse");
    assert_eq!(finding.severity, GateSeverity::Warning);
    assert_eq!(finding.file.as_deref(), Some("src/lib.rs"));
    assert_eq!(finding.line, Some(10));
}

#[test]
fn parse_issue_line_no_location() {
    let finding = SonarQubeGate::parse_issue_line("ERROR: Analysis failed");
    let finding = finding.expect("should parse");
    assert_eq!(finding.severity, GateSeverity::Error);
    assert!(finding.file.is_none());
    assert!(finding.line.is_none());
    assert_eq!(finding.message, "Analysis failed");
}

#[test]
fn parse_issue_line_non_issue() {
    assert!(SonarQubeGate::parse_issue_line("INFO: Analysis complete").is_none());
    assert!(SonarQubeGate::parse_issue_line("").is_none());
    assert!(SonarQubeGate::parse_issue_line("  ").is_none());
}

#[test]
fn parse_scanner_output_success() {
    let output = std::process::Output {
        status: std::process::ExitStatus::default(),
        stdout: b"INFO: Analysis complete\n".to_vec(),
        stderr: Vec::new(),
    };

    let (passed, findings, summary) = SonarQubeGate::parse_scanner_output(&output);
    // ExitStatus::default() is success on unix
    assert!(passed);
    assert!(findings.is_empty());
    assert!(summary.contains("passed"));
}

#[test]
fn sonarqube_gate_name() {
    let gate = SonarQubeGate::new();
    assert_eq!(gate.name(), "sonarqube");
}

#[test]
fn sonarqube_supported_gates() {
    let gate = SonarQubeGate::new();
    assert_eq!(gate.supported_gates(), &["sonarqube".to_string()]);
}

#[test]
fn sonarqube_gate_with_config() {
    let config = SonarQubeConfig {
        server_url: "https://sonar.corp.example.com".to_string(),
        token: Some("token123".to_string()),
        project_key: Some("my-app".to_string()),
        scanner_path: "sonar-scanner".to_string(),
    };
    let gate = SonarQubeGate::with_config(config);
    assert_eq!(
        gate.default_config.server_url,
        "https://sonar.corp.example.com"
    );
    assert_eq!(gate.default_config.token.as_deref(), Some("token123"));
}
