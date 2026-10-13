use super::*;

#[test]
fn gate_labels() {
    assert_eq!(Gate::Check.label(), "check");
    assert_eq!(Gate::Test.label(), "test");
    assert_eq!(Gate::Lint.label(), "lint");
}

#[test]
fn gate_display() {
    assert_eq!(format!("{}", Gate::Check), "check");
}

#[test]
fn gate_all() {
    let all = Gate::all();
    assert_eq!(all.len(), 3);
}

#[test]
fn validation_result_all_passed() {
    let result = ValidationResult {
        results: vec![
            GateResult {
                gate: Gate::Check,
                passed: true,
                duration: Duration::from_secs(1),
                output: String::new(),
                command: "cargo check".into(),
            },
            GateResult {
                gate: Gate::Test,
                passed: true,
                duration: Duration::from_secs(2),
                output: String::new(),
                command: "cargo test".into(),
            },
        ],
    };
    assert!(result.all_passed());
    assert_eq!(result.passed_count(), 2);
    assert_eq!(result.failed_count(), 0);
    assert_eq!(result.total_duration(), Duration::from_secs(3));
}

#[test]
fn validation_result_with_failure() {
    let result = ValidationResult {
        results: vec![
            GateResult {
                gate: Gate::Check,
                passed: true,
                duration: Duration::from_secs(1),
                output: String::new(),
                command: "cargo check".into(),
            },
            GateResult {
                gate: Gate::Test,
                passed: false,
                duration: Duration::from_secs(2),
                output: "test failed".into(),
                command: "cargo test".into(),
            },
        ],
    };
    assert!(!result.all_passed());
    assert_eq!(result.passed_count(), 1);
    assert_eq!(result.failed_count(), 1);
}

#[test]
fn gate_result_status_label() {
    let pass = GateResult {
        gate: Gate::Check,
        passed: true,
        duration: Duration::ZERO,
        output: String::new(),
        command: String::new(),
    };
    let fail = GateResult {
        gate: Gate::Check,
        passed: false,
        duration: Duration::ZERO,
        output: String::new(),
        command: String::new(),
    };
    assert_eq!(pass.status_label(), "PASS");
    assert_eq!(fail.status_label(), "FAIL");
}

#[test]
fn gate_command_maps_correctly() {
    let qg = QualityGate {
        check: "cargo check".into(),
        test: "cargo test".into(),
        lint: "cargo clippy".into(),
    };
    assert_eq!(gate_command(&qg, Gate::Check), "cargo check");
    assert_eq!(gate_command(&qg, Gate::Test), "cargo test");
    assert_eq!(gate_command(&qg, Gate::Lint), "cargo clippy");
}

fn current_settings(dir: &Path) -> String {
    let config = serde_json::json!({
        "project": {"name": "settings-test", "type": "cli"},
        "stack": {"language": "unknown", "framework": "none", "package_manager": "unknown", "test_framework": "none", "linter": "none", "formatter": "none"},
        "quality_gate": {"check": "true", "test": "true", "lint": "true"},
        "format": {"command": "", "extensions": []},
        "testing": {"companion_pattern": "{name}.tests.{ext}", "test_dir": "alongside"},
        "reference_repos": [], "state_prefix": "settings-test", "roadmap_exports": false
    });
    let content = serde_json::to_string_pretty(&config).unwrap();
    std::fs::create_dir_all(dir.join(".magistr")).unwrap();
    std::fs::write(dir.join(".magistr/project.json"), &content).unwrap();
    content
}

#[test]
fn current_settings_loading_does_not_detect_or_write_stack_values() {
    let tmp = tempfile::tempdir().unwrap();
    let original = current_settings(tmp.path());
    std::fs::write(
        tmp.path().join("Cargo.toml"),
        "[package]\nname = \"detected-rust\"\nversion = \"0.1.0\"\n",
    )
    .unwrap();
    let config = load_project_config(tmp.path()).unwrap();
    assert_eq!(config.stack.language, "unknown");
    assert_eq!(config.stack.package_manager, "unknown");
    assert_eq!(
        std::fs::read_to_string(tmp.path().join(".magistr/project.json")).unwrap(),
        original
    );
}

#[test]
fn current_settings_errors_name_the_settings_file() {
    let tmp = tempfile::tempdir().unwrap();
    current_settings(tmp.path());
    let path = crate::paths::project_json(tmp.path()).unwrap();
    std::fs::write(&path, "{invalid").unwrap();
    let error = load_project_config(tmp.path()).unwrap_err().to_string();
    assert!(error.contains(&path.display().to_string()), "{error}");
}
