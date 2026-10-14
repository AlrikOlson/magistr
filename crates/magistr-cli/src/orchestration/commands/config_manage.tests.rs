use super::*;

// A capture with no home: the user config is unlocated, so these checks
// never read a developer's real ~/.config/magistr.
fn inputs(root: &std::path::Path) -> ProjectInputs {
    ProjectInputs::from_lookup(root, |_| None)
}

#[test]
fn config_check_no_project_json_warns() {
    let tmp = tempfile::TempDir::new().unwrap();
    // Should succeed (warnings only, not errors)
    let result = cmd_config_check(tmp.path(), &inputs(tmp.path()));
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
    let result = cmd_config_check(tmp.path(), &inputs(tmp.path()));
    assert!(result.is_ok(), "check should pass: {result:?}");
}

#[test]
fn config_check_reports_an_unparseable_user_config_as_an_error() {
    let tmp = tempfile::TempDir::new().unwrap();
    let config_dir = tmp.path().join("config");
    std::fs::create_dir_all(&config_dir).unwrap();
    std::fs::write(config_dir.join("config.toml"), "not = [toml").unwrap();
    let inputs = inputs(tmp.path()).with_config_dir(&config_dir);
    let result = cmd_config_check(tmp.path(), &inputs);
    assert!(
        result.is_err(),
        "an unreadable user config is an error, not a default"
    );
}

#[test]
fn config_check_invalid_json_fails() {
    let tmp = tempfile::TempDir::new().unwrap();
    let magistr_dir = tmp.path().join(".magistr");
    std::fs::create_dir_all(&magistr_dir).unwrap();
    std::fs::write(magistr_dir.join("project.json"), "not valid json{{{").unwrap();
    let result = cmd_config_check(tmp.path(), &inputs(tmp.path()));
    assert!(result.is_err(), "invalid JSON should fail");
}

#[test]
fn config_show_no_project_json_succeeds() {
    let tmp = tempfile::TempDir::new().unwrap();
    let result = cmd_config_show(tmp.path(), &inputs(tmp.path()));
    assert!(
        result.is_ok(),
        "show should succeed without project.json: {result:?}"
    );
}

#[test]
fn current_settings_show_refuses_invalid_project_settings() {
    let root = tempfile::tempdir().unwrap();
    std::fs::create_dir(root.path().join(".magistr")).unwrap();
    std::fs::write(root.path().join(".magistr/project.json"), "{invalid").unwrap();
    assert!(cmd_config_show(root.path(), &inputs(root.path())).is_err());
}

#[test]
fn current_settings_show_refuses_invalid_personal_settings() {
    let root = tempfile::tempdir().unwrap();
    let personal = root.path().join("personal");
    std::fs::create_dir(&personal).unwrap();
    std::fs::write(personal.join("config.toml"), "not = [toml").unwrap();
    let inputs = inputs(root.path()).with_config_dir(personal);
    assert!(cmd_config_show(root.path(), &inputs).is_err());
}
