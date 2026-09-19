use super::*;
use crate::scaffold::{Ownership, is_executable_path};
use tempfile::TempDir;

fn make_preview(path: &str, action: FileAction, content: &str) -> PlannedChange {
    PlannedChange {
        ownership: Ownership::Unclaimed,
        ownership_context: None,
        path: path.to_string(),
        action,
        new_content: content.to_string(),
        existing_content: None,
        diff: None,
    }
}

#[test]
fn apply_rejects_every_stale_target_before_writing_any_selected_file() {
    for (before, after) in [
        (None, Some("another writer created this")),
        (Some("reviewed content"), Some("edited after review")),
        (Some("reviewed content"), None),
    ] {
        let tmp = TempDir::new().unwrap();
        let target = tmp.path().join("changed.md");
        if let Some(content) = after {
            std::fs::write(&target, content).unwrap();
        }
        let first = make_preview("new-directory/first.md", FileAction::Create, "first");
        let stale = PlannedChange {
            ownership: Ownership::Unclaimed,
            ownership_context: None,
            path: "changed.md".to_owned(),
            action: if before.is_some() {
                FileAction::Overwrite
            } else {
                FileAction::Create
            },
            new_content: "replacement".to_owned(),
            existing_content: before.map(str::to_owned),
            diff: None,
        };
        let error = apply_approved(tmp.path(), &[&first, &stale]).unwrap_err();
        assert!(error.to_string().contains("changed.md"), "{error}");
        assert!(error.to_string().contains("preview"), "{error}");
        assert!(!tmp.path().join("new-directory").exists());
        assert!(!tmp.path().join(".magistr").exists());
        assert_eq!(std::fs::read_to_string(&target).ok().as_deref(), after);
    }
}

#[test]
fn apply_does_not_treat_unreadable_text_or_a_directory_as_absent() {
    for directory in [false, true] {
        let tmp = TempDir::new().unwrap();
        let target = tmp.path().join("target");
        if directory {
            std::fs::create_dir(&target).unwrap();
        } else {
            std::fs::write(&target, [0xff, 0xfe]).unwrap();
        }
        let first = make_preview("new-directory/first.md", FileAction::Create, "first");
        let invalid = make_preview("target", FileAction::Create, "replacement");
        assert!(apply_approved(tmp.path(), &[&first, &invalid]).is_err());
        assert!(!tmp.path().join("new-directory").exists());
        if !directory {
            assert_eq!(std::fs::read(&target).unwrap(), [0xff, 0xfe]);
        }
    }
}

#[test]
fn selectively_applied_files_keep_ownership_through_upgrade_and_user_edits() {
    use super::super::manifest::ScaffoldManifest;
    use super::super::pipeline::ScaffoldPipeline;
    use std::collections::HashMap;

    let tmp = TempDir::new().unwrap();
    let mut manifest = ScaffoldManifest::new();
    manifest.record("unrelated.md", "previous generated content");
    manifest.save(tmp.path()).unwrap();
    let pipeline = ScaffoldPipeline::headless(crate::types::Stack::default());
    let preview = pipeline.preview(tmp.path()).unwrap();
    let settings = preview
        .files
        .iter()
        .find(|f| f.path == ".claude/settings.json")
        .unwrap();
    apply_approved(tmp.path(), &[settings]).unwrap();
    let manifest = ScaffoldManifest::load(tmp.path());
    assert!(!manifest.is_customized(&settings.path, &settings.new_content));
    assert!(!manifest.is_customized("unrelated.md", "previous generated content"));
    assert!(!manifest.files.contains_key(".claude/rules/conventions.md"));

    let overrides = HashMap::from([(settings.path.clone(), "{\"upgraded\": true}\n".to_owned())]);
    let upgraded =
        super::super::generate_with_result(tmp.path(), &pipeline.template_vars(), &overrides)
            .unwrap();
    assert!(upgraded.updated.contains(&settings.path));
    assert_eq!(
        std::fs::read_to_string(tmp.path().join(&settings.path)).unwrap(),
        overrides[&settings.path]
    );

    std::fs::write(tmp.path().join(&settings.path), "{\"user\": true}\n").unwrap();
    let next = pipeline.preview(tmp.path()).unwrap();
    assert_eq!(
        next.files
            .iter()
            .find(|f| f.path == settings.path)
            .unwrap()
            .action,
        FileAction::Preserved
    );
    let result = pipeline.generate(tmp.path()).unwrap();
    assert!(!result.contains(&settings.path));
    assert_eq!(
        std::fs::read_to_string(tmp.path().join(&settings.path)).unwrap(),
        "{\"user\": true}\n"
    );
}

#[cfg(unix)]
#[test]
fn apply_rejects_a_dangling_symlink_created_after_preview() {
    let tmp = TempDir::new().unwrap();
    let target = tmp.path().join("not-created.md");
    let first = make_preview("new-directory/first.md", FileAction::Create, "first");
    let linked = make_preview("linked.md", FileAction::Create, "replacement");
    std::os::unix::fs::symlink(&target, tmp.path().join("linked.md")).unwrap();
    let error = apply_approved(tmp.path(), &[&first, &linked]).unwrap_err();
    assert!(error.to_string().contains("linked.md"), "{error}");
    assert!(!target.exists());
    assert!(!tmp.path().join("new-directory").exists());
    assert!(tmp.path().join("linked.md").is_symlink());
}

#[test]
fn apply_shared_merge_files_does_not_claim_whole_file_ownership() {
    let tmp = TempDir::new().unwrap();
    let mcp = make_preview(".mcp.json", FileAction::Create, "{}");
    let gitignore = make_preview(".gitignore", FileAction::Create, "user-pattern\n");
    apply_approved(tmp.path(), &[&mcp, &gitignore]).unwrap();
    assert!(!tmp.path().join(".magistr").exists());
    assert_eq!(
        std::fs::read_to_string(tmp.path().join(".gitignore")).unwrap(),
        "user-pattern\n"
    );
}

#[test]
fn apply_creates_new_files() {
    let tmp = TempDir::new().unwrap();
    let f1 = make_preview(
        ".magistr/project.json",
        FileAction::Create,
        r#"{"test": true}"#,
    );
    let f2 = make_preview(
        ".claude/rules/workflow.md",
        FileAction::Create,
        "# Workflow",
    );

    let written = apply_approved(tmp.path(), &[&f1, &f2])
        .unwrap()
        .applied_paths();

    assert_eq!(written.len(), 2);
    assert!(tmp.path().join(".magistr/project.json").exists());
    assert!(tmp.path().join(".claude/rules/workflow.md").exists());
    assert_eq!(
        std::fs::read_to_string(tmp.path().join(".magistr/project.json")).unwrap(),
        r#"{"test": true}"#
    );
}

#[test]
fn apply_overwrites_existing_files() {
    let tmp = TempDir::new().unwrap();
    let dir = tmp.path().join(".claude");
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("settings.json"), "old").unwrap();

    let f = PlannedChange {
        ownership: Ownership::Unclaimed,
        ownership_context: None,
        path: ".claude/settings.json".to_string(),
        action: FileAction::Overwrite,
        new_content: "new".to_string(),
        existing_content: Some("old".to_string()),
        diff: Some("- old\n+ new".to_string()),
    };

    let written = apply_approved(tmp.path(), &[&f]).unwrap().applied_paths();

    assert_eq!(written, vec![".claude/settings.json"]);
    assert_eq!(
        std::fs::read_to_string(tmp.path().join(".claude/settings.json")).unwrap(),
        "new"
    );
}

#[test]
fn apply_skips_unchanged_files() {
    let tmp = TempDir::new().unwrap();
    let f = make_preview(".claude/settings.json", FileAction::Unchanged, "same");

    let written = apply_approved(tmp.path(), &[&f]).unwrap().applied_paths();

    assert!(written.is_empty());
    assert!(!tmp.path().join(".claude/settings.json").exists());
    assert!(!tmp.path().join(".claude").exists());
    assert!(!tmp.path().join(".magistr").exists());
}

#[test]
fn apply_skips_preserved_files() {
    let tmp = TempDir::new().unwrap();
    std::fs::write(tmp.path().join("CLAUDE.md"), "user content").unwrap();

    let f = PlannedChange {
        ownership: Ownership::Unclaimed,
        ownership_context: None,
        path: "CLAUDE.md".to_string(),
        action: FileAction::Preserved,
        new_content: "generated".to_string(),
        existing_content: Some("user content".to_string()),
        diff: None,
    };

    let written = apply_approved(tmp.path(), &[&f]).unwrap().applied_paths();

    assert!(written.is_empty());
    assert_eq!(
        std::fs::read_to_string(tmp.path().join("CLAUDE.md")).unwrap(),
        "user content"
    );
}

#[test]
fn apply_creates_nested_directories() {
    let tmp = TempDir::new().unwrap();
    let f = make_preview(
        ".claude/skills/validate/SKILL.md",
        FileAction::Create,
        "# Validate",
    );

    let written = apply_approved(tmp.path(), &[&f]).unwrap().applied_paths();

    assert_eq!(written.len(), 1);
    assert!(tmp.path().join(".claude/skills/validate/SKILL.md").exists());
}

#[cfg(unix)]
#[test]
fn apply_sets_executable_on_shell_scripts() {
    use std::os::unix::fs::PermissionsExt;

    let tmp = TempDir::new().unwrap();
    let f = make_preview(
        "scripts/example.sh",
        FileAction::Create,
        "#!/bin/bash\necho ok",
    );

    apply_approved(tmp.path(), &[&f]).unwrap();

    let meta = std::fs::metadata(tmp.path().join("scripts/example.sh")).unwrap();
    let mode = meta.permissions().mode();
    assert!(mode & 0o111 != 0, "Script should be executable");
}

#[test]
fn apply_empty_list_is_noop() {
    let tmp = TempDir::new().unwrap();
    let written = apply_approved(tmp.path(), &[]).unwrap().applied_paths();
    assert!(written.is_empty());
}

#[test]
fn apply_removes_an_approved_removal_and_drops_its_ownership_record() {
    use super::super::manifest::ScaffoldManifest;

    let tmp = TempDir::new().unwrap();
    let dir = tmp.path();
    let retired = ".claude/rules/retired.md";
    std::fs::create_dir_all(dir.join(".claude/rules")).unwrap();
    std::fs::write(dir.join(retired), "retired\n").unwrap();
    let mut manifest = ScaffoldManifest::new();
    manifest.record(retired, "retired\n");
    manifest.record("kept.md", "kept\n");
    manifest.save(dir).unwrap();

    let removal = PlannedChange::removal(retired, "retired\n".to_owned(), Ownership::Generated);
    let created = make_preview(".claude/rules/new.md", FileAction::Create, "new\n");
    let receipt = apply_approved(dir, &[&removal, &created]).unwrap();

    assert_eq!(
        receipt.applied_paths(),
        vec![retired.to_owned(), ".claude/rules/new.md".to_owned()]
    );
    assert!(receipt.failed().next().is_none());
    assert!(!dir.join(retired).exists());
    assert!(dir.join(".claude/rules").is_dir(), "setup keeps the parent");
    assert_eq!(
        std::fs::read_to_string(dir.join(".claude/rules/new.md")).unwrap(),
        "new\n"
    );
    let manifest = ScaffoldManifest::load(dir);
    assert!(!manifest.files.contains_key(retired));
    assert!(manifest.files.contains_key("kept.md"));
}

#[test]
fn apply_rejects_a_stale_removal_before_writing_any_selected_file() {
    let retired = ".claude/rules/retired.md";
    for found in [Some("edited after review\n"), None] {
        let tmp = TempDir::new().unwrap();
        let dir = tmp.path();
        std::fs::create_dir_all(dir.join(".claude/rules")).unwrap();
        if let Some(content) = found {
            std::fs::write(dir.join(retired), content).unwrap();
        }
        let first = make_preview("new-directory/first.md", FileAction::Create, "first");
        let removal = PlannedChange::removal(retired, "retired\n".to_owned(), Ownership::Generated);
        let error = apply_approved(dir, &[&first, &removal]).unwrap_err();
        assert!(error.to_string().contains(retired), "{error}");
        assert!(error.to_string().contains("preview"), "{error}");
        assert!(!dir.join("new-directory").exists());
        assert_eq!(
            std::fs::read_to_string(dir.join(retired)).ok().as_deref(),
            found
        );
    }
}

#[test]
fn apply_leaves_an_unapproved_removal_in_place() {
    let tmp = TempDir::new().unwrap();
    let dir = tmp.path();
    let retired = ".claude/rules/retired.md";
    std::fs::create_dir_all(dir.join(".claude/rules")).unwrap();
    std::fs::write(dir.join(retired), "retired\n").unwrap();
    let created = make_preview(".claude/rules/new.md", FileAction::Create, "new\n");

    let written = apply_approved(dir, &[&created]).unwrap().applied_paths();

    assert_eq!(written, vec![".claude/rules/new.md"]);
    assert_eq!(
        std::fs::read_to_string(dir.join(retired)).unwrap(),
        "retired\n"
    );
}

/// The setup plan's retired rule file, approved on its own from a dry run,
/// is the only thing selective apply touches.
#[test]
fn selective_apply_removes_only_the_approved_retired_rule() {
    use super::super::pipeline::ScaffoldPipeline;

    let tmp = TempDir::new().unwrap();
    let dir = tmp.path();
    let retired = ".claude/rules/ministr-scope.md";
    std::fs::create_dir_all(dir.join(".claude/rules")).unwrap();
    std::fs::write(dir.join(retired), "retired\n").unwrap();
    let pipeline = ScaffoldPipeline::headless(crate::types::Stack::default());
    let plan = pipeline.preview(dir).unwrap();
    let removal = plan.get(retired).unwrap();
    assert_eq!(removal.action, FileAction::Remove);
    assert!(plan.creates() > 0, "the plan has writes the reviewer skips");

    let written = apply_approved(dir, &[removal]).unwrap().applied_paths();

    assert_eq!(written, vec![retired]);
    assert!(!dir.join(retired).exists());
    assert!(!dir.join(".magistr/project.json").exists());
    assert!(!dir.join(".claude/settings.json").exists());
    assert_eq!(
        std::fs::read_dir(dir.join(".claude/rules"))
            .unwrap()
            .count(),
        0,
        "no skipped rule file was written"
    );
}

#[test]
fn codex_apply_rejects_revoked_ownership_before_any_selected_write() {
    use super::super::{manifest::ScaffoldManifest, pipeline::ScaffoldPipeline};
    use std::collections::HashMap;

    let skill = ".agents/skills/next/SKILL.md";
    for selected_path in [".codex/config.toml", "AGENTS.md", skill] {
        let tmp = TempDir::new().unwrap();
        let dir = tmp.path();
        let pipeline = ScaffoldPipeline::headless(crate::types::Stack::default())
            .with_agents(&["codex".to_owned()]);
        pipeline.generate(dir).unwrap();
        let project_path = dir.join(".magistr/project.json");
        let mut project: serde_json::Value =
            serde_json::from_slice(&std::fs::read(&project_path).unwrap()).unwrap();
        project["roadmap_exports"] = false.into();
        std::fs::write(&project_path, serde_json::to_vec(&project).unwrap()).unwrap();
        let mut vars = pipeline.template_vars();
        vars.insert("project_root".into(), "upgraded-root".into());
        let overrides = HashMap::from([(skill.to_owned(), "Upgraded workflow\n".to_owned())]);
        let preview = super::super::preview(dir, &vars, &overrides).unwrap();
        let selected = preview
            .files
            .iter()
            .find(|file| file.path == selected_path)
            .unwrap();
        assert_eq!(selected.action, FileAction::Overwrite);
        let target_before = std::fs::read(dir.join(selected_path)).unwrap();
        let mut manifest = ScaffoldManifest::load(dir);
        manifest.codex = None;
        manifest.files.remove(skill);
        manifest.save(dir).unwrap();
        let manifest_before = std::fs::read(dir.join(".magistr/scaffold-manifest.json")).unwrap();
        let first = make_preview("no-write/first.md", FileAction::Create, "first");
        let error =
            apply_approved(dir, &[&first, selected]).expect_err("ownership changed after review");
        assert!(error.to_string().contains("preview"), "{error}");
        assert!(!dir.join("no-write").exists());
        assert_eq!(
            std::fs::read(dir.join(selected_path)).unwrap(),
            target_before
        );
        assert_eq!(
            std::fs::read(dir.join(".magistr/scaffold-manifest.json")).unwrap(),
            manifest_before
        );
    }
}

#[test]
fn codex_apply_rejects_changed_project_policy_with_unchanged_target_bytes() {
    use super::super::pipeline::ScaffoldPipeline;
    use std::collections::HashMap;

    let skill = ".agents/skills/next/SKILL.md";
    for selected_path in [".codex/config.toml", skill] {
        let tmp = TempDir::new().unwrap();
        let dir = tmp.path();
        let pipeline = ScaffoldPipeline::headless(crate::types::Stack::default())
            .with_agents(&["codex".to_owned()]);
        pipeline.generate(dir).unwrap();
        let mut vars = pipeline.template_vars();
        vars.insert("project_root".into(), "upgraded-root".into());
        let overrides = HashMap::from([(skill.to_owned(), "Upgraded workflow\n".to_owned())]);
        let preview = super::super::preview(dir, &vars, &overrides).unwrap();
        let selected = preview
            .files
            .iter()
            .find(|file| file.path == selected_path)
            .unwrap();
        assert_eq!(selected.action, FileAction::Overwrite);
        let target_before = std::fs::read(dir.join(selected_path)).unwrap();
        let project_path = dir.join(".magistr/project.json");
        let mut project: serde_json::Value =
            serde_json::from_slice(&std::fs::read(&project_path).unwrap()).unwrap();
        project["roadmap_exports"] = false.into();
        let project_before = serde_json::to_vec(&project).unwrap();
        std::fs::write(&project_path, &project_before).unwrap();
        let first = make_preview("no-write/first.md", FileAction::Create, "first");
        let error =
            apply_approved(dir, &[&first, selected]).expect_err("policy changed after review");
        assert!(error.to_string().contains("preview"), "{error}");
        assert!(!dir.join("no-write").exists());
        assert_eq!(
            std::fs::read(dir.join(selected_path)).unwrap(),
            target_before
        );
        assert_eq!(std::fs::read(&project_path).unwrap(), project_before);
    }
}

#[test]
fn codex_apply_validates_context_once_before_its_own_batch_changes_it() {
    use super::super::pipeline::ScaffoldPipeline;

    let tmp = TempDir::new().unwrap();
    let dir = tmp.path();
    let pipeline = ScaffoldPipeline::headless(crate::types::Stack::default())
        .with_agents(&["codex".to_owned()]);
    let preview = pipeline.preview(dir).unwrap();
    let selected: Vec<_> = [
        ".magistr/project.json",
        ".agents/skills/next/SKILL.md",
        ".codex/config.toml",
        "AGENTS.md",
        ".codex/run",
    ]
    .into_iter()
    .map(|path| preview.files.iter().find(|file| file.path == path).unwrap())
    .collect();
    let written = apply_approved(dir, &selected).unwrap().applied_paths();
    assert_eq!(written.len(), selected.len());
    for file in selected {
        assert_eq!(
            std::fs::read_to_string(dir.join(&file.path)).unwrap(),
            file.new_content
        );
    }
}

#[test]
fn is_executable_path_detects_shell_scripts() {
    assert!(is_executable_path("scripts/example.sh"));
    assert!(is_executable_path("scripts/run.sh"));
    assert!(!is_executable_path(".magistr/project.json"));
    assert!(!is_executable_path("CLAUDE.md"));
}
