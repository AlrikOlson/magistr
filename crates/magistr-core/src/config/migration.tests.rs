use super::*;
use std::path::PathBuf;

#[test]
fn no_claude_dir_no_migration() {
    let dir = tempfile::TempDir::new().unwrap();
    let check = check_migration(dir.path()).unwrap();
    assert!(!check.needs_migration());
    assert!(check.actions.is_empty());
    assert!(check.manual_reasons.is_empty());
}

#[test]
fn missing_project_json_needs_migration() {
    let dir = tempfile::TempDir::new().unwrap();
    let claude_dir = dir.path().join(".claude");
    std::fs::create_dir_all(&claude_dir).unwrap();

    let check = check_migration(dir.path()).unwrap();
    assert!(check.needs_migration());
    assert!(
        check
            .manual_reasons
            .iter()
            .any(|r| r.contains("project.json"))
    );
}

#[test]
fn has_project_json_no_migration() {
    let dir = tempfile::TempDir::new().unwrap();
    let claude_dir = dir.path().join(".claude");
    let magistr_dir = dir.path().join(".magistr");
    std::fs::create_dir_all(&claude_dir).unwrap();
    std::fs::create_dir_all(&magistr_dir).unwrap();
    std::fs::write(
        magistr_dir.join("project.json"),
        r#"{"roadmap_backend":"think-and-ship"}"#,
    )
    .unwrap();

    let check = check_migration(dir.path()).unwrap();
    assert!(!check.needs_migration());
}

#[test]
fn bash_hook_needs_migration() {
    let dir = tempfile::TempDir::new().unwrap();
    let claude_dir = dir.path().join(".claude");
    let magistr_dir = dir.path().join(".magistr");
    let hooks_dir = claude_dir.join("hooks");
    std::fs::create_dir_all(&hooks_dir).unwrap();
    std::fs::create_dir_all(&magistr_dir).unwrap();
    std::fs::write(
        magistr_dir.join("project.json"),
        r#"{"roadmap_backend":"think-and-ship"}"#,
    )
    .unwrap();
    std::fs::write(hooks_dir.join("pre-tool-use.sh"), "#!/bin/bash\necho hello").unwrap();

    let check = check_migration(dir.path()).unwrap();
    assert!(check.needs_migration());
    assert!(
        check
            .manual_reasons
            .iter()
            .any(|r| r.contains("bash shebang"))
    );
}

#[test]
fn env_bash_hook_needs_migration() {
    let dir = tempfile::TempDir::new().unwrap();
    let claude_dir = dir.path().join(".claude");
    let magistr_dir = dir.path().join(".magistr");
    let hooks_dir = claude_dir.join("hooks");
    std::fs::create_dir_all(&hooks_dir).unwrap();
    std::fs::create_dir_all(&magistr_dir).unwrap();
    std::fs::write(
        magistr_dir.join("project.json"),
        r#"{"roadmap_backend":"think-and-ship"}"#,
    )
    .unwrap();
    std::fs::write(
        hooks_dir.join("validate.sh"),
        "#!/usr/bin/env bash\ncargo test",
    )
    .unwrap();

    let check = check_migration(dir.path()).unwrap();
    assert!(check.needs_migration());
}

#[test]
fn non_bash_hook_no_migration() {
    let dir = tempfile::TempDir::new().unwrap();
    let claude_dir = dir.path().join(".claude");
    let magistr_dir = dir.path().join(".magistr");
    let hooks_dir = claude_dir.join("hooks");
    std::fs::create_dir_all(&hooks_dir).unwrap();
    std::fs::create_dir_all(&magistr_dir).unwrap();
    std::fs::write(
        magistr_dir.join("project.json"),
        r#"{"roadmap_backend":"think-and-ship"}"#,
    )
    .unwrap();
    std::fs::write(
        hooks_dir.join("pre-tool-use.sh"),
        "# magistr-generated hook\nset -e\ncargo check",
    )
    .unwrap();

    let check = check_migration(dir.path()).unwrap();
    assert!(!check.needs_migration());
}

#[test]
fn legacy_settings_is_auto_fixable() {
    let dir = tempfile::TempDir::new().unwrap();
    let claude_dir = dir.path().join(".claude");
    let magistr_dir = dir.path().join(".magistr");
    std::fs::create_dir_all(&claude_dir).unwrap();
    std::fs::create_dir_all(&magistr_dir).unwrap();
    std::fs::write(
        magistr_dir.join("project.json"),
        r#"{"roadmap_backend":"think-and-ship"}"#,
    )
    .unwrap();
    std::fs::write(
        claude_dir.join("settings.json"),
        r#"{"jig_version": "0.1.0"}"#,
    )
    .unwrap();

    let check = check_migration(dir.path()).unwrap();
    assert!(check.needs_migration());
    assert!(
        check
            .actions
            .contains(&MigrationAction::RemoveJigVersionKey)
    );
}

#[test]
fn multiple_issues_collected() {
    let dir = tempfile::TempDir::new().unwrap();
    let claude_dir = dir.path().join(".claude");
    let hooks_dir = claude_dir.join("hooks");
    std::fs::create_dir_all(&hooks_dir).unwrap();
    // No project.json (manual) + bash hook (manual) + legacy settings (auto)
    std::fs::write(hooks_dir.join("hook.sh"), "#!/bin/bash\necho test").unwrap();
    std::fs::write(
        claude_dir.join("settings.json"),
        r#"{"jig_version": "0.1.0"}"#,
    )
    .unwrap();

    let check = check_migration(dir.path()).unwrap();
    assert!(check.needs_migration());
    // 1 auto-fixable (jig_version) + 2 manual (missing project.json + bash hook)
    assert_eq!(check.actions.len(), 1);
    assert!(check.manual_reasons.len() >= 2);
}

#[test]
fn legacy_jig_dir_detected_as_auto_fixable() {
    let dir = tempfile::TempDir::new().unwrap();
    std::fs::create_dir_all(dir.path().join(".jig")).unwrap();
    std::fs::write(dir.path().join(".jig").join("project.json"), "{}").unwrap();

    let check = check_migration(dir.path()).unwrap();
    assert!(check.needs_migration());
    assert!(check.actions.contains(&MigrationAction::RenameJigDir));
}

#[test]
fn legacy_jig_dir_with_project_json_no_manual_reason() {
    let dir = tempfile::TempDir::new().unwrap();
    let claude_dir = dir.path().join(".claude");
    std::fs::create_dir_all(&claude_dir).unwrap();
    std::fs::create_dir_all(dir.path().join(".jig")).unwrap();
    std::fs::write(dir.path().join(".jig").join("project.json"), "{}").unwrap();

    let check = check_migration(dir.path()).unwrap();
    // .jig/ has project.json so "missing project.json" should NOT be a manual reason
    assert!(
        !check
            .manual_reasons
            .iter()
            .any(|r| r.contains("project.json"))
    );
}

#[test]
fn roadmap_md_detected_as_auto_fixable() {
    let dir = tempfile::TempDir::new().unwrap();
    std::fs::write(dir.path().join("ROADMAP.md"), "# Roadmap\n- [ ] task").unwrap();

    let check = check_migration(dir.path()).unwrap();
    assert!(check.actions.contains(&MigrationAction::MigrateRoadmapMd));
}

#[test]
fn no_roadmap_action_when_yaml_exists() {
    let dir = tempfile::TempDir::new().unwrap();
    std::fs::write(
        dir.path().join(".magistr-roadmap.yml"),
        "title: test\nphases: []\n",
    )
    .unwrap();
    std::fs::write(dir.path().join("ROADMAP.md"), "# Roadmap\n- [ ] task").unwrap();

    let check = check_migration(dir.path()).unwrap();
    assert!(!check.actions.contains(&MigrationAction::MigrateRoadmapMd));
}

#[test]
fn run_migration_renames_jig_dir() {
    let dir = tempfile::TempDir::new().unwrap();
    std::fs::create_dir_all(dir.path().join(".jig")).unwrap();
    std::fs::write(dir.path().join(".jig").join("project.json"), "{}").unwrap();

    let check = check_migration(dir.path()).unwrap();
    let completed = run_simple_migrations(dir.path(), &check).unwrap();

    assert!(!completed.is_empty());
    assert!(!dir.path().join(".jig").exists());
    assert!(dir.path().join(".magistr").exists());
    assert!(dir.path().join(".magistr").join("project.json").exists());
}

#[test]
fn run_migration_removes_jig_version_key() {
    let dir = tempfile::TempDir::new().unwrap();
    let claude_dir = dir.path().join(".claude");
    let magistr_dir = dir.path().join(".magistr");
    std::fs::create_dir_all(&claude_dir).unwrap();
    std::fs::create_dir_all(&magistr_dir).unwrap();
    std::fs::write(
        magistr_dir.join("project.json"),
        r#"{"roadmap_backend":"think-and-ship"}"#,
    )
    .unwrap();
    std::fs::write(
        claude_dir.join("settings.json"),
        r#"{"jig_version": "0.1.0", "other": true}"#,
    )
    .unwrap();

    let check = check_migration(dir.path()).unwrap();
    run_simple_migrations(dir.path(), &check).unwrap();

    let content = std::fs::read_to_string(claude_dir.join("settings.json")).unwrap();
    assert!(!content.contains("jig_version"));
    assert!(content.contains("other"));
}

#[test]
fn summary_no_migration() {
    let check = MigrationCheck {
        actions: vec![],
        manual_reasons: vec![],
        claude_dir: PathBuf::from(".claude"),
    };
    assert_eq!(check.summary(), "Configuration is up to date.");
}

#[test]
fn summary_auto_fixable_only() {
    let check = MigrationCheck {
        actions: vec![MigrationAction::RenameJigDir],
        manual_reasons: vec![],
        claude_dir: PathBuf::from(".claude"),
    };
    let summary = check.summary();
    assert!(summary.contains("Auto-fixable"));
    assert!(summary.contains(".jig/"));
    assert!(summary.contains("magistr migrate"));
}

#[test]
fn summary_manual_only() {
    let check = MigrationCheck {
        actions: vec![],
        manual_reasons: vec!["Missing project.json".to_string()],
        claude_dir: PathBuf::from(".claude"),
    };
    let summary = check.summary();
    assert!(summary.contains("magistr init"));
    assert!(summary.contains("Missing project.json"));
}

#[test]
fn summary_mixed() {
    let check = MigrationCheck {
        actions: vec![MigrationAction::RenameJigDir],
        manual_reasons: vec!["Missing project.json".to_string()],
        claude_dir: PathBuf::from(".claude"),
    };
    let summary = check.summary();
    assert!(summary.contains("Auto-fixable"));
    assert!(summary.contains("magistr init"));
    assert!(summary.contains("magistr migrate"));
}

// ── roadmap_backend (ts5a) ──────────────────────────────────────────

fn project_with(dir: &std::path::Path, json: &str) {
    let magistr_dir = dir.join(".magistr");
    std::fs::create_dir_all(&magistr_dir).unwrap();
    std::fs::write(magistr_dir.join("project.json"), json).unwrap();
}

#[test]
fn missing_backend_key_is_auto_fixable_with_the_default() {
    use magistr_engine::native::RoadmapBackend;
    let dir = tempfile::TempDir::new().unwrap();
    project_with(dir.path(), "{}");
    let check = check_migration(dir.path()).unwrap();
    assert!(
        check
            .actions
            .contains(&MigrationAction::WriteRoadmapBackend(
                RoadmapBackend::ThinkAndShip
            )),
        "{:?}",
        check.actions
    );
    assert!(
        check.summary().contains("think-and-ship"),
        "{}",
        check.summary()
    );
}

#[test]
fn legacy_yaml_project_gets_yaml_written_and_keeps_its_roadmap() {
    use magistr_engine::native::RoadmapBackend;
    let dir = tempfile::TempDir::new().unwrap();
    project_with(dir.path(), "{}");
    std::fs::write(
        dir.path().join(".magistr-roadmap.yml"),
        "title: test\nphases: []\n",
    )
    .unwrap();
    let check = check_migration(dir.path()).unwrap();
    assert!(
        check
            .actions
            .contains(&MigrationAction::WriteRoadmapBackend(RoadmapBackend::Yaml)),
        "{:?}",
        check.actions
    );

    let done = run_simple_migrations(dir.path(), &check).unwrap();
    assert!(
        done.iter().any(|d| d.contains("roadmap_backend")),
        "{done:?}"
    );
    let written = std::fs::read_to_string(dir.path().join(".magistr/project.json")).unwrap();
    assert!(
        written.contains("\"roadmap_backend\": \"yaml\""),
        "{written}"
    );
    // Idempotent: the key is there now.
    let again = check_migration(dir.path()).unwrap();
    assert!(
        !again
            .actions
            .iter()
            .any(|a| matches!(a, MigrationAction::WriteRoadmapBackend(_))),
        "{:?}",
        again.actions
    );
}

#[test]
fn native_project_does_not_migrate_roadmap_md() {
    let dir = tempfile::TempDir::new().unwrap();
    project_with(dir.path(), r#"{"roadmap_backend":"think-and-ship"}"#);
    std::fs::write(dir.path().join("ROADMAP.md"), "# Roadmap\n- [ ] task").unwrap();
    let check = check_migration(dir.path()).unwrap();
    assert!(!check.actions.contains(&MigrationAction::MigrateRoadmapMd));
    assert!(!check.needs_migration(), "{}", check.summary());
}
