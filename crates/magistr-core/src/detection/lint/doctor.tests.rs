use super::*;
use tempfile::TempDir;

fn setup_project(dir: &Path) {
    std::fs::create_dir_all(dir.join(".claude/rules")).unwrap();
    std::fs::create_dir_all(dir.join(".magistr")).unwrap();
    std::fs::write(dir.join("CLAUDE.md"), "# Project\n\nRules here.\n").unwrap();
    std::fs::write(dir.join(".claude/settings.json"), r#"{"allowedTools": []}"#).unwrap();
    std::fs::write(
        dir.join(".magistr/project.json"),
        r#"{"project": {"name": "test"}}"#,
    )
    .unwrap();
    std::fs::write(dir.join(".claude/rules/conventions.md"), "# Conventions\n").unwrap();
}

#[test]
fn healthy_project_all_pass() {
    let dir = TempDir::new().unwrap();
    setup_project(dir.path());
    let checks = run_all(dir.path(), true);
    let fails: Vec<_> = checks
        .iter()
        .filter(|c| c.status == CheckStatus::Fail)
        .collect();
    assert!(fails.is_empty(), "Expected no failures, got: {fails:?}");
}

#[test]
fn missing_claude_md_fails() {
    let dir = TempDir::new().unwrap();
    let check = check_claude_md_exists(dir.path());
    assert_eq!(check.status, CheckStatus::Fail);
}

#[test]
fn existing_claude_md_passes() {
    let dir = TempDir::new().unwrap();
    std::fs::write(dir.path().join("CLAUDE.md"), "# Test\n").unwrap();
    let check = check_claude_md_exists(dir.path());
    assert_eq!(check.status, CheckStatus::Pass);
}

#[test]
fn large_claude_md_fails() {
    let dir = TempDir::new().unwrap();
    let content = "line\n".repeat(600);
    std::fs::write(dir.path().join("CLAUDE.md"), content).unwrap();
    let check = check_claude_md_size(dir.path());
    assert_eq!(check.status, CheckStatus::Fail);
}

#[test]
fn medium_claude_md_warns() {
    let dir = TempDir::new().unwrap();
    let content = "line\n".repeat(350);
    std::fs::write(dir.path().join("CLAUDE.md"), content).unwrap();
    let check = check_claude_md_size(dir.path());
    assert_eq!(check.status, CheckStatus::Warn);
}

#[test]
fn small_claude_md_passes() {
    let dir = TempDir::new().unwrap();
    std::fs::write(dir.path().join("CLAUDE.md"), "Short.\n").unwrap();
    let check = check_claude_md_size(dir.path());
    assert_eq!(check.status, CheckStatus::Pass);
}

#[test]
fn missing_rules_dir_warns() {
    let dir = TempDir::new().unwrap();
    let check = check_rules_directory(dir.path());
    assert_eq!(check.status, CheckStatus::Warn);
}

#[test]
fn empty_rules_dir_warns() {
    let dir = TempDir::new().unwrap();
    std::fs::create_dir_all(dir.path().join(".claude/rules")).unwrap();
    let check = check_rules_directory(dir.path());
    assert_eq!(check.status, CheckStatus::Warn);
}

#[test]
fn rules_dir_with_files_passes() {
    let dir = TempDir::new().unwrap();
    std::fs::create_dir_all(dir.path().join(".claude/rules")).unwrap();
    std::fs::write(dir.path().join(".claude/rules/conventions.md"), "# Rules\n").unwrap();
    let check = check_rules_directory(dir.path());
    assert_eq!(check.status, CheckStatus::Pass);
}

#[test]
fn invalid_settings_json_fails() {
    let dir = TempDir::new().unwrap();
    std::fs::create_dir_all(dir.path().join(".claude")).unwrap();
    std::fs::write(dir.path().join(".claude/settings.json"), "not json {{{").unwrap();
    let check = check_settings_valid(dir.path());
    assert_eq!(check.status, CheckStatus::Fail);
}

#[test]
fn valid_settings_json_passes() {
    let dir = TempDir::new().unwrap();
    std::fs::create_dir_all(dir.path().join(".claude")).unwrap();
    std::fs::write(dir.path().join(".claude/settings.json"), "{}").unwrap();
    let check = check_settings_valid(dir.path());
    assert_eq!(check.status, CheckStatus::Pass);
}

#[test]
fn missing_claude_dir_fails() {
    let dir = TempDir::new().unwrap();
    let check = check_claude_dir_exists(dir.path());
    assert_eq!(check.status, CheckStatus::Fail);
}

#[test]
fn existing_claude_dir_passes() {
    let dir = TempDir::new().unwrap();
    std::fs::create_dir_all(dir.path().join(".claude")).unwrap();
    let check = check_claude_dir_exists(dir.path());
    assert_eq!(check.status, CheckStatus::Pass);
}

#[test]
fn check_display_format() {
    let check = DoctorCheck {
        name: "test",
        description: "something checked".into(),
        status: CheckStatus::Pass,
        detail: Some("all good".into()),
    };
    let s = check.to_string();
    assert!(s.contains("✓"));
    assert!(s.contains("test"));
    assert!(s.contains("→ all good"));
}
