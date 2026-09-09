use super::*;
use crate::output::SilentRenderer;

#[test]
fn cmd_export_errors_on_missing_project_json() {
    // Error path: no .magistr/project.json in the directory.
    let tmp = tempfile::TempDir::new().unwrap();
    let renderer = SilentRenderer::new();
    let result = cmd_export(tmp.path(), None, &renderer);
    assert!(result.is_err(), "export should fail without project.json");
}

/// Create a valid project.json for testing.
fn sample_project_json(name: &str) -> serde_json::Value {
    serde_json::json!({
        "project": {
            "name": name,
            "description": "test project",
            "type": "cli"
        },
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
            "command": "cargo fmt",
            "extensions": [".rs"]
        },
        "spec": {
            "file": "ROADMAP.md",
            "unchecked_pattern": "- [ ]"
        },
        "testing": {
            "companion_pattern": "tests/{{name}}_test.rs",
            "test_dir": "tests"
        },
        "reference_repos": [],
        "state_prefix": name
    })
}

/// Write a valid .magistr/project.json into a temp directory.
fn write_project_json(dir: &std::path::Path, name: &str) {
    let magistr_dir = dir.join(".magistr");
    std::fs::create_dir_all(&magistr_dir).unwrap();
    let json = sample_project_json(name);
    std::fs::write(magistr_dir.join("project.json"), json.to_string()).unwrap();
}

#[test]
fn cmd_export_creates_preset_file() {
    // Happy path: export from a directory with valid project.json.
    let tmp = tempfile::TempDir::new().unwrap();
    write_project_json(tmp.path(), "test-export");

    let renderer = SilentRenderer::new();
    let result = cmd_export(tmp.path(), None, &renderer);
    assert!(result.is_ok(), "export should succeed: {result:?}");

    // The preset file should be created at the default location.
    let preset_path = tmp.path().join(".magistr-preset.toml");
    assert!(
        preset_path.exists(),
        "preset file should exist at {}",
        preset_path.display()
    );
}

#[test]
fn cmd_export_with_custom_output_path() {
    // Edge case: export to a custom output path.
    let tmp = tempfile::TempDir::new().unwrap();
    write_project_json(tmp.path(), "custom-out");

    let custom_path = tmp.path().join("my-preset.toml");
    let renderer = SilentRenderer::new();
    let result = cmd_export(tmp.path(), Some(custom_path.as_path()), &renderer);
    assert!(
        result.is_ok(),
        "export to custom path should succeed: {result:?}"
    );
    assert!(custom_path.exists(), "custom preset file should exist");
}

#[test]
fn cmd_lint_errors_on_missing_claude_md() {
    // Error path: no CLAUDE.md in the directory.
    let tmp = tempfile::TempDir::new().unwrap();
    let renderer = SilentRenderer::new();
    let result = cmd_lint(tmp.path(), false, &renderer);
    assert!(result.is_err());
    let msg = result.unwrap_err().to_string();
    assert!(
        msg.contains("No CLAUDE.md found"),
        "unexpected error: {msg}"
    );
}

#[test]
fn cmd_init_ci_generates_workflow_for_rust() {
    // Happy path: generate CI workflow for a Rust project.
    let tmp = tempfile::TempDir::new().unwrap();
    std::fs::write(
        tmp.path().join("Cargo.toml"),
        r#"[package]
name = "ci-test"
version = "0.1.0"
edition = "2024"
"#,
    )
    .unwrap();

    let renderer = SilentRenderer::new();
    let result = cmd_init_ci(tmp.path(), false, &renderer);
    assert!(result.is_ok(), "init --ci should succeed: {result:?}");

    // A workflow file should be created under .github/workflows/
    let workflow = tmp.path().join(".github/workflows/quality-gates.yml");
    assert!(
        workflow.exists(),
        "workflow file should exist at {}",
        workflow.display()
    );

    let content = std::fs::read_to_string(&workflow).unwrap();
    assert!(content.contains("cargo"), "workflow should reference cargo");
}
