use super::*;
use std::collections::HashSet;

#[test]
fn license_config_defaults() {
    let config = LicenseConfig::default();
    assert!(config.allowed.is_empty());
    assert!(config.denied.contains("GPL-3.0"));
    assert!(config.denied.contains("AGPL-3.0"));
    assert!(config.deny_unlicensed);
}

#[test]
fn license_config_from_json() {
    let json = serde_json::json!({
        "allowed": ["MIT", "Apache-2.0"],
        "denied": ["GPL-3.0"],
        "deny_unlicensed": false
    });

    let config = LicenseConfig::from_json(&json);
    assert_eq!(config.allowed.len(), 2);
    assert!(config.allowed.contains("MIT"));
    assert!(config.allowed.contains("Apache-2.0"));
    assert_eq!(config.denied.len(), 1);
    assert!(config.denied.contains("GPL-3.0"));
    assert!(!config.deny_unlicensed);
}

#[test]
fn license_config_from_partial_json() {
    let json = serde_json::json!({
        "denied": ["SSPL"]
    });

    let config = LicenseConfig::from_json(&json);
    assert!(config.allowed.is_empty()); // keeps default
    assert_eq!(config.denied.len(), 1);
    assert!(config.denied.contains("SSPL"));
    assert!(config.deny_unlicensed); // keeps default
}

#[test]
fn check_dependency_denied_license() {
    let config = LicenseConfig::default();
    let dep = DependencyLicense {
        name: "some-gpl-lib".to_string(),
        license: Some("GPL-3.0".to_string()),
        source_file: "Cargo.toml".to_string(),
    };

    let finding = LicenseGate::check_dependency(&dep, &config);
    let finding = finding.expect("should produce a finding");
    assert_eq!(finding.severity, GateSeverity::Error);
    assert!(finding.message.contains("denied license"));
    assert!(finding.rule_id.unwrap().contains("license-denied"));
}

#[test]
fn check_dependency_allowed_license() {
    let config = LicenseConfig {
        allowed: HashSet::from(["MIT".to_string(), "Apache-2.0".to_string()]),
        denied: HashSet::new(),
        deny_unlicensed: true,
    };
    let dep = DependencyLicense {
        name: "safe-lib".to_string(),
        license: Some("MIT".to_string()),
        source_file: "Cargo.toml".to_string(),
    };

    let finding = LicenseGate::check_dependency(&dep, &config);
    assert!(finding.is_none(), "MIT should be allowed");
}

#[test]
fn check_dependency_not_allowed_license() {
    let config = LicenseConfig {
        allowed: HashSet::from(["MIT".to_string()]),
        denied: HashSet::new(),
        deny_unlicensed: true,
    };
    let dep = DependencyLicense {
        name: "bsd-lib".to_string(),
        license: Some("BSD-3-Clause".to_string()),
        source_file: "Cargo.toml".to_string(),
    };

    let finding = LicenseGate::check_dependency(&dep, &config);
    let finding = finding.expect("should produce a finding");
    assert_eq!(finding.severity, GateSeverity::Error);
    assert!(finding.message.contains("non-allowed"));
}

#[test]
fn check_dependency_missing_license_denied() {
    let config = LicenseConfig {
        denied: HashSet::new(),
        allowed: HashSet::new(),
        deny_unlicensed: true,
    };
    let dep = DependencyLicense {
        name: "mystery-lib".to_string(),
        license: None,
        source_file: "Cargo.toml".to_string(),
    };

    let finding = LicenseGate::check_dependency(&dep, &config);
    let finding = finding.expect("should produce a finding");
    assert_eq!(finding.severity, GateSeverity::Warning);
    assert!(finding.message.contains("no license"));
}

#[test]
fn check_dependency_missing_license_allowed() {
    let config = LicenseConfig {
        denied: HashSet::new(),
        allowed: HashSet::new(),
        deny_unlicensed: false,
    };
    let dep = DependencyLicense {
        name: "mystery-lib".to_string(),
        license: None,
        source_file: "Cargo.toml".to_string(),
    };

    let finding = LicenseGate::check_dependency(&dep, &config);
    assert!(finding.is_none());
}

#[test]
fn scan_cargo_toml_with_tempdir() {
    let dir = tempfile::tempdir().expect("create tempdir");
    let cargo_toml = dir.path().join("Cargo.toml");
    std::fs::write(
        &cargo_toml,
        r#"
[package]
name = "test-project"
version = "0.1.0"

[dependencies]
serde = "1.0"
tokio = { version = "1", features = ["full"] }

[dev-dependencies]
tempfile = "3"
"#,
    )
    .expect("write Cargo.toml");

    let deps = LicenseGate::scan_cargo_toml(&cargo_toml);
    let names: Vec<&str> = deps.iter().map(|d| d.name.as_str()).collect();
    assert!(names.contains(&"serde"));
    assert!(names.contains(&"tokio"));
    assert!(names.contains(&"tempfile"));
}

#[test]
fn scan_package_json_with_tempdir() {
    let dir = tempfile::tempdir().expect("create tempdir");
    let package_json = dir.path().join("package.json");
    std::fs::write(
        &package_json,
        r#"{
  "name": "test-app",
  "license": "MIT",
  "dependencies": {
    "react": "^18.0.0",
    "next": "^14.0.0"
  },
  "devDependencies": {
    "typescript": "^5.0.0"
  }
}"#,
    )
    .expect("write package.json");

    let deps = LicenseGate::scan_package_json(&package_json);
    let names: Vec<&str> = deps.iter().map(|d| d.name.as_str()).collect();
    assert!(names.contains(&"test-app"));
    assert!(names.contains(&"react"));
    assert!(names.contains(&"next"));
    assert!(names.contains(&"typescript"));

    // The project itself should have the license.
    let project = deps.iter().find(|d| d.name == "test-app").unwrap();
    assert_eq!(project.license.as_deref(), Some("MIT"));
}

#[test]
fn scan_licenses_full_project() {
    let dir = tempfile::tempdir().expect("create tempdir");
    std::fs::write(
        dir.path().join("Cargo.toml"),
        "[dependencies]\nserde = \"1\"\n",
    )
    .expect("write");

    let deps = LicenseGate::scan_licenses(dir.path());
    assert!(!deps.is_empty());
}

#[test]
fn license_gate_name() {
    let gate = LicenseGate::new();
    assert_eq!(gate.name(), "license");
}

#[test]
fn license_gate_supported_gates() {
    let gate = LicenseGate::new();
    assert_eq!(gate.supported_gates(), &["license".to_string()]);
}

#[tokio::test]
async fn license_gate_run_empty_project() {
    let dir = tempfile::tempdir().expect("create tempdir");
    let mut gate = LicenseGate::new();

    let config = GateConfig {
        gate_name: "license".to_string(),
        project_dir: dir.path().display().to_string(),
        config: None,
    };

    let outcome = gate.run_gate(config).await.expect("should succeed");
    assert!(outcome.passed);
    assert!(outcome.findings.is_empty());
    assert!(outcome.summary.contains("0 dependencies"));
}

#[tokio::test]
async fn license_gate_run_with_denied_license() {
    let dir = tempfile::tempdir().expect("create tempdir");
    std::fs::write(
        dir.path().join("package.json"),
        r#"{"name": "gpl-app", "license": "GPL-3.0", "dependencies": {}}"#,
    )
    .expect("write");

    let mut gate = LicenseGate::new();
    let config = GateConfig {
        gate_name: "license".to_string(),
        project_dir: dir.path().display().to_string(),
        config: None,
    };

    let outcome = gate.run_gate(config).await.expect("should succeed");
    assert!(!outcome.passed);
    assert_eq!(outcome.error_count(), 1);
    assert!(outcome.findings[0].message.contains("GPL-3.0"));
}
