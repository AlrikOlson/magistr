use super::*;
use crate::output::SilentRenderer;
use crate::types;

#[test]
fn cmd_detect_succeeds_for_rust_project() {
    // Happy path: detect stack in a directory with Cargo.toml.
    let tmp = tempfile::TempDir::new().unwrap();
    std::fs::write(
        tmp.path().join("Cargo.toml"),
        r#"[package]
name = "detect-test"
version = "0.1.0"
edition = "2024"
"#,
    )
    .unwrap();

    let renderer = SilentRenderer::new();
    let result = cmd_detect(tmp.path(), &renderer);
    assert!(
        result.is_ok(),
        "detect should succeed for Rust project: {result:?}"
    );
}

#[test]
fn cmd_detect_succeeds_with_unknown_on_empty_dir() {
    // Edge case: empty dir succeeds but detects Language::Unknown.
    // Detection is best-effort — it doesn't fail, just returns unknown values.
    let tmp = tempfile::TempDir::new().unwrap();
    let renderer = SilentRenderer::new();
    let result = cmd_detect(tmp.path(), &renderer);
    assert!(result.is_ok(), "detect should succeed even on empty dir");
}

#[test]
fn cmd_plans_empty_returns_ok() {
    // A missing native execution is empty, not an error.
    let tmp = tempfile::TempDir::new().unwrap();
    let renderer = SilentRenderer::new();
    let inputs = crate::config::inputs::ProjectInputs::from_lookup(tmp.path(), |_| None);
    let result = cmd_plans(tmp.path(), None, 10, &renderer, &inputs);
    assert!(
        result.is_ok(),
        "plans should succeed on empty dir: {result:?}"
    );
}

#[test]
fn ensure_no_existing_scaffold_allows_clean_dir() {
    // Happy path: no .claude/ dir exists.
    let tmp = tempfile::TempDir::new().unwrap();
    let result = ensure_no_existing_scaffold(tmp.path(), false);
    assert!(result.is_ok());
}

#[test]
fn ensure_no_existing_scaffold_allows_existing_without_force() {
    // Idempotent: .claude/ exists without --force — should succeed (no-op).
    let tmp = tempfile::TempDir::new().unwrap();
    std::fs::create_dir(tmp.path().join(".claude")).unwrap();
    let result = ensure_no_existing_scaffold(tmp.path(), false);
    assert!(result.is_ok(), "init should be idempotent without --force");
    // .claude/ should still exist (not removed).
    assert!(tmp.path().join(".claude").exists());
}

#[test]
fn ensure_no_existing_scaffold_preserves_with_force() {
    // Preflight never removes files or the ownership evidence generation needs.
    let tmp = tempfile::TempDir::new().unwrap();
    let claude_dir = tmp.path().join(".claude");
    std::fs::create_dir_all(claude_dir.join("rules")).unwrap();
    std::fs::write(claude_dir.join("rules/workflow.md"), "magistr's").unwrap();
    std::fs::write(claude_dir.join("dummy.json"), "{}").unwrap();
    std::fs::create_dir(tmp.path().join(".magistr")).unwrap();

    let mut manifest = crate::scaffold::manifest::ScaffoldManifest::new();
    manifest.record(".claude/rules/workflow.md", "magistr's");
    manifest.save(tmp.path()).unwrap();

    let result = ensure_no_existing_scaffold(tmp.path(), true);
    assert!(result.is_ok(), "force should succeed: {result:?}");
    assert!(
        claude_dir.join("rules/workflow.md").is_file(),
        "generation decides whether a recorded file needs updating"
    );
    assert!(
        claude_dir.join("dummy.json").is_file(),
        "a file magistr never wrote is not magistr's to delete"
    );
    assert!(tmp.path().join(".magistr/scaffold-manifest.json").is_file());
}

#[test]
fn build_template_vars_includes_project_name() {
    // Verify template vars contain detected project name.
    let stack = types::Stack {
        project_name: "my-project".into(),
        language: types::Language::Rust,
        package_manager: types::PackageManager::Cargo,
        framework: types::Framework::None,
        test_framework: types::TestFramework::CargoTest,
        linter: types::Linter::Clippy,
        formatter: types::Formatter::Rustfmt,
        workspace: types::Workspace::None,
        ci_provider: types::CiProvider::None,
        extra_detections: Vec::new(),
    };
    let preset = crate::config::preset::fallback_preset();
    let selections = crate::scaffold::ScaffoldSelections::default();
    let vars = build_template_vars(&stack, &preset, &selections);

    let name_var = vars.iter().find(|(k, _)| *k == "project_name");
    assert!(
        name_var.is_some(),
        "template vars should include project_name"
    );
    assert_eq!(name_var.unwrap().1, "my-project");
}
