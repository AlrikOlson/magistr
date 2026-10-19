use super::*;

fn project(dir: &Path, lint_cmd: &str) {
    std::fs::create_dir_all(dir.join(".magistr")).expect("mkdir");
    let config = serde_json::json!({
        "project": {"name": "t", "description": "", "type": "cli"},
        "stack": {"language": "rust", "framework": "none", "package_manager": "cargo",
                  "test_framework": "cargo-nextest", "linter": "clippy", "formatter": "rustfmt"},
        "quality_gate": {"check": "cargo check", "test": "cargo test", "lint": lint_cmd},
        "format": {"command": "cargo fmt", "extensions": ["rs"]},
        "spec": {"file": "ROADMAP.md", "unchecked_pattern": "^- \\\\[ \\\\]"},
        "state_prefix": "t",
        "reference_repos": [],
        "testing": {"companion_pattern": "{name}.tests.{ext}", "test_dir": "alongside"}
    });
    std::fs::write(dir.join(".magistr/project.json"), config.to_string()).expect("write");
}

fn workspace(dir: &Path, lint_cmd: &str) {
    std::fs::create_dir_all(dir.join(".magistr")).unwrap();
    std::fs::write(
        dir.join(".magistr/workspace.toml"),
        format!(
            "[commands]\n\"check\" = \"cargo check\"\n\"test\" = \"cargo test\"\n\"lint\" = \"{lint_cmd}\"\n\"format\" = \"cargo fmt\"\n"
        ),
    )
    .expect("write");
}

#[test]
fn no_workspace_toml_is_a_warning_pointing_at_init() {
    let tmp = tempfile::tempdir().expect("tempdir");
    project(tmp.path(), "cargo clippy");
    assert_eq!(workspace_drift(tmp.path()), None);
    let check = doctor_check(tmp.path());
    assert_eq!(check.status, CheckStatus::Warn);
    assert!(
        check
            .detail
            .as_deref()
            .unwrap_or("")
            .contains("magistr init")
    );
}

#[test]
fn matching_files_pass() {
    let tmp = tempfile::tempdir().expect("tempdir");
    project(tmp.path(), "cargo clippy");
    workspace(tmp.path(), "cargo clippy");
    assert_eq!(workspace_drift(tmp.path()), Some(vec![]));
    let check = doctor_check(tmp.path());
    assert_eq!(check.status, CheckStatus::Pass);
    assert_eq!(check.name, "workspace-commands");
}

#[test]
fn a_changed_gate_is_a_warning_naming_both_sides() {
    let tmp = tempfile::tempdir().expect("tempdir");
    project(tmp.path(), "cargo clippy -- -D warnings");
    workspace(tmp.path(), "cargo clippy");
    let findings = workspace_drift(tmp.path()).expect("comparable");
    assert_eq!(findings.len(), 1, "{findings:?}");
    assert!(findings[0].contains("`lint` drifted"), "{findings:?}");
    let check = doctor_check(tmp.path());
    assert_eq!(check.status, CheckStatus::Warn);
    assert!(
        check
            .detail
            .as_deref()
            .unwrap_or("")
            .contains("cargo clippy -- -D warnings")
    );
}

#[test]
fn workspace_without_project_json_warns_instead_of_guessing() {
    let tmp = tempfile::tempdir().expect("tempdir");
    workspace(tmp.path(), "cargo clippy");
    assert_eq!(workspace_drift(tmp.path()), None);
    let check = doctor_check(tmp.path());
    assert_eq!(check.status, CheckStatus::Warn);
    assert!(
        check
            .detail
            .as_deref()
            .unwrap_or("")
            .contains("project.json")
    );
}
