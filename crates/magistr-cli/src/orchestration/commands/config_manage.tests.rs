use super::*;

#[test]
fn config_check_no_project_json_warns() {
    let tmp = tempfile::TempDir::new().unwrap();
    // Should succeed (warnings only, not errors)
    let result = cmd_config_check(tmp.path());
    assert!(
        result.is_ok(),
        "check should pass with warnings: {result:?}"
    );
}

#[test]
fn config_check_valid_project_json_passes() {
    let tmp = tempfile::TempDir::new().unwrap();
    let magistr_dir = tmp.path().join(".magistr");
    std::fs::create_dir_all(&magistr_dir).unwrap();
    std::fs::write(
        magistr_dir.join("project.json"),
        serde_json::json!({
            "project": { "name": "test-project", "version": "1.0.0" },
            "stack": {
                "language": "rust",
                "framework": "none",
                "package_manager": "cargo",
                "test_framework": "cargo-test",
                "linter": "clippy",
                "formatter": "rustfmt"
            },
            "quality_gate": {
                "check": "cargo check",
                "test": "cargo test",
                "lint": "cargo clippy"
            },
            "format": {
                "command": "rustfmt {file}",
                "extensions": ["rs"]
            },
            "spec": {
                "file": "ROADMAP.md",
                "unchecked_pattern": "- [ ]"
            },
            "testing": {
                "companion_pattern": "{name}_test",
                "test_dir": "tests"
            },
            "reference_repos": [],
            "state_prefix": "test"
        })
        .to_string(),
    )
    .unwrap();
    let result = cmd_config_check(tmp.path());
    assert!(result.is_ok(), "check should pass: {result:?}");
}

#[test]
fn config_check_invalid_json_fails() {
    let tmp = tempfile::TempDir::new().unwrap();
    let magistr_dir = tmp.path().join(".magistr");
    std::fs::create_dir_all(&magistr_dir).unwrap();
    std::fs::write(magistr_dir.join("project.json"), "not valid json{{{").unwrap();
    let result = cmd_config_check(tmp.path());
    assert!(result.is_err(), "invalid JSON should fail");
}

#[test]
fn config_show_no_project_json_succeeds() {
    let tmp = tempfile::TempDir::new().unwrap();
    let result = cmd_config_show(tmp.path());
    assert!(
        result.is_ok(),
        "show should succeed without project.json: {result:?}"
    );
}
