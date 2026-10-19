use super::*;
use crate::scaffold::Refusal;
use crate::scaffold::manifest::content_hash;
use crate::scaffold::{ChangePlan, Ownership};
use tempfile::TempDir;

fn write(dir: &Path, path: &str, content: &str) {
    let full = dir.join(path);
    std::fs::create_dir_all(full.parent().unwrap()).unwrap();
    std::fs::write(full, content).unwrap();
}

fn read(dir: &Path, path: &str) -> String {
    std::fs::read_to_string(dir.join(path)).unwrap()
}

fn apply(dir: &Path, plan: &ChangePlan, owned: &[&str]) -> ApplyReceipt {
    let mut service = AdminService::open(dir, plan.operation, owned.iter().copied());
    for change in &plan.files {
        service.execute(change);
    }
    service.finish()
}

/// A setup plan over a project with one file of every kind.
fn plan(dir: &Path) -> ChangePlan {
    write(dir, ".claude/rules/old.md", "old\n");
    write(dir, ".claude/rules/same.md", "same\n");
    write(dir, ".claude/rules/mine.md", "mine\n");
    write(dir, ".claude/rules/retired.md", "retired\n");
    write(dir, "unrelated.md", "untouched\n");
    let mut plan = ChangePlan::new(AdminOperation::Setup);
    plan.files.push(PlannedChange::merged(
        ".claude/rules/new.md",
        None,
        "new\n".to_owned(),
    ));
    plan.files.push(PlannedChange::merged(
        ".claude/rules/old.md",
        Some("old\n".to_owned()),
        "newer\n".to_owned(),
    ));
    plan.files.push(PlannedChange::merged(
        ".claude/rules/same.md",
        Some("same\n".to_owned()),
        "same\n".to_owned(),
    ));
    plan.files.push(PlannedChange::preserved(
        ".claude/rules/mine.md",
        Some("mine\n".to_owned()),
        Ownership::Customized,
    ));
    plan.files.push(PlannedChange::removal(
        ".claude/rules/retired.md",
        "retired\n".to_owned(),
        Ownership::Internal,
    ));
    plan.files.push(PlannedChange::refused(
        ".claude/rules/blocked.md",
        Refusal::NotRegularFile,
        None,
        Ownership::Internal,
    ));
    plan.files.push(PlannedChange::merged(
        "scripts/run.sh",
        None,
        "#!/bin/sh\n".to_owned(),
    ));
    plan
}

#[test]
fn an_unchanged_project_applies_the_whole_plan_and_records_what_it_owns() {
    let tmp = TempDir::new().unwrap();
    let dir = tmp.path();
    let plan = plan(dir);
    let receipt = apply(
        dir,
        &plan,
        &[
            ".claude/rules/new.md",
            ".claude/rules/old.md",
            ".claude/rules/same.md",
        ],
    );

    assert!(!receipt.is_partial(), "{}", receipt.summary());
    assert_eq!(
        receipt.applied_paths(),
        [
            ".claude/rules/new.md",
            ".claude/rules/old.md",
            ".claude/rules/retired.md",
            "scripts/run.sh"
        ]
    );
    assert_eq!(read(dir, ".claude/rules/new.md"), "new\n");
    assert_eq!(read(dir, ".claude/rules/old.md"), "newer\n");
    assert_eq!(read(dir, ".claude/rules/mine.md"), "mine\n");
    assert!(!dir.join(".claude/rules/retired.md").exists());
    assert_eq!(read(dir, "unrelated.md"), "untouched\n");
    assert_eq!(
        receipt.get(".claude/rules/same.md").unwrap().outcome,
        ApplyOutcome::Skipped {
            reason: "already up to date".to_owned()
        }
    );
    assert_eq!(
        receipt.get(".claude/rules/mine.md").unwrap().outcome,
        ApplyOutcome::Skipped {
            reason: "customized after magistr wrote it".to_owned()
        }
    );
    assert_eq!(
        receipt.get(".claude/rules/blocked.md").unwrap().outcome,
        ApplyOutcome::Refused {
            reason: "target is not a regular file".to_owned()
        }
    );
    assert_eq!(receipt.manifest, ManifestOutcome::Saved);
    let manifest = ScaffoldManifest::load(dir);
    assert_eq!(
        manifest.files.get(".claude/rules/new.md"),
        Some(&content_hash("new\n"))
    );
    assert_eq!(
        manifest.files.get(".claude/rules/old.md"),
        Some(&content_hash("newer\n"))
    );
    assert_eq!(
        manifest.files.get(".claude/rules/same.md"),
        Some(&content_hash("same\n"))
    );
    assert!(!manifest.files.contains_key("scripts/run.sh"));
    assert!(
        dir.join(".claude/rules").is_dir(),
        "setup keeps directories"
    );
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mode = std::fs::metadata(dir.join("scripts/run.sh"))
            .unwrap()
            .permissions()
            .mode();
        assert_eq!(mode & 0o777, 0o755);
    }
}

#[test]
fn a_target_that_changed_after_planning_fails_and_is_left_as_found() {
    let tmp = TempDir::new().unwrap();
    let dir = tmp.path();
    let plan = plan(dir);
    // Concurrent mutation between planning and applying.
    write(dir, ".claude/rules/new.md", "someone else's\n");
    write(dir, ".claude/rules/old.md", "edited meanwhile\n");
    std::fs::remove_file(dir.join(".claude/rules/retired.md")).unwrap();
    std::fs::remove_file(dir.join(".claude/rules/same.md")).unwrap();
    std::fs::create_dir(dir.join(".claude/rules/same.md")).unwrap();

    let receipt = apply(
        dir,
        &plan,
        &[".claude/rules/new.md", ".claude/rules/old.md"],
    );

    assert!(receipt.is_partial());
    assert_eq!(receipt.applied_paths(), ["scripts/run.sh"]);
    assert_eq!(
        receipt.get(".claude/rules/new.md").unwrap().outcome,
        ApplyOutcome::Failed(ApplyFailure::Stale {
            expected: Expected::Absent,
            found: Expected::of(Some("someone else's\n")),
        })
    );
    assert_eq!(
        receipt.get(".claude/rules/old.md").unwrap().outcome,
        ApplyOutcome::Failed(ApplyFailure::Stale {
            expected: Expected::of(Some("old\n")),
            found: Expected::of(Some("edited meanwhile\n")),
        })
    );
    assert_eq!(
        receipt.get(".claude/rules/retired.md").unwrap().outcome,
        ApplyOutcome::Failed(ApplyFailure::Stale {
            expected: Expected::of(Some("retired\n")),
            found: Expected::Absent,
        })
    );
    assert_eq!(
        receipt.get(".claude/rules/same.md").unwrap().outcome,
        ApplyOutcome::Failed(ApplyFailure::Blocked(Refusal::NotRegularFile))
    );
    assert_eq!(read(dir, ".claude/rules/new.md"), "someone else's\n");
    assert_eq!(read(dir, ".claude/rules/old.md"), "edited meanwhile\n");
    assert_eq!(read(dir, "unrelated.md"), "untouched\n");
    assert!(
        !ScaffoldManifest::load(dir)
            .files
            .contains_key(".claude/rules/old.md")
    );
    let error = receipt.partial_error().unwrap().to_string();
    assert!(error.contains("setup did not complete"), "{error}");
    assert!(error.contains(".claude/rules/old.md: failed"), "{error}");
    assert!(
        error.contains("applied before that: scripts/run.sh"),
        "{error}"
    );
}

#[test]
fn a_write_that_fails_leaves_the_target_intact_and_the_rest_still_applies() {
    let tmp = TempDir::new().unwrap();
    let dir = tmp.path();
    let plan = plan(dir);
    // The create's parent is now a file; the overwrite's temporary file
    // name is taken, so its write cannot even start.
    write(dir, "scripts", "not a directory\n");
    std::fs::write(temp_path(&dir.join(".claude/rules/old.md")), "occupied").unwrap();

    let receipt = apply(dir, &plan, &[".claude/rules/old.md"]);

    assert!(receipt.is_partial());
    assert_eq!(
        receipt.applied_paths(),
        [".claude/rules/new.md", ".claude/rules/retired.md"]
    );
    // A parent that is a file is found by revalidation, before any write.
    assert!(matches!(
        receipt.get("scripts/run.sh").unwrap().outcome,
        ApplyOutcome::Failed(ApplyFailure::Blocked(Refusal::NotRegularFile))
    ));
    assert_eq!(read(dir, "scripts"), "not a directory\n");
    assert!(matches!(
        receipt.get(".claude/rules/old.md").unwrap().outcome,
        ApplyOutcome::Failed(ApplyFailure::Io { .. })
    ));
    assert_eq!(read(dir, ".claude/rules/old.md"), "old\n");
    assert_eq!(
        std::fs::read_to_string(temp_path(&dir.join(".claude/rules/old.md"))).unwrap(),
        "occupied",
        "a file the service did not create is not removed"
    );
    assert!(
        !ScaffoldManifest::load(dir)
            .files
            .contains_key(".claude/rules/old.md")
    );
    assert_eq!(receipt.manifest, ManifestOutcome::Untouched);
}

#[test]
fn a_manifest_that_cannot_be_saved_is_reported_beside_the_applied_files() {
    let tmp = TempDir::new().unwrap();
    let dir = tmp.path();
    let plan = plan(dir);
    std::fs::write(dir.join(".magistr"), "not a directory").unwrap();

    let receipt = apply(dir, &plan, &[".claude/rules/new.md"]);

    assert!(receipt.is_partial());
    assert_eq!(read(dir, ".claude/rules/new.md"), "new\n");
    assert_eq!(
        receipt.get(".claude/rules/new.md").unwrap().outcome,
        ApplyOutcome::Applied
    );
    assert!(receipt.manifest.is_failed(), "{}", receipt.manifest);
    let error = receipt.partial_error().unwrap().to_string();
    assert!(error.contains("manifest not saved"), "{error}");
    assert!(
        error.contains("applied before that: .claude/rules/new.md"),
        "{error}"
    );
}

#[cfg(unix)]
#[test]
fn an_overwrite_keeps_the_targets_mode_and_an_unchanged_launcher_regains_execute_bits() {
    use std::os::unix::fs::PermissionsExt;
    let tmp = TempDir::new().unwrap();
    let dir = tmp.path();
    write(dir, ".claude/settings.json", "{}\n");
    std::fs::set_permissions(
        dir.join(".claude/settings.json"),
        std::fs::Permissions::from_mode(0o600),
    )
    .unwrap();
    write(dir, ".codex/run", "#!/bin/sh\n");
    std::fs::set_permissions(
        dir.join(".codex/run"),
        std::fs::Permissions::from_mode(0o644),
    )
    .unwrap();
    let mut plan = ChangePlan::new(AdminOperation::Repair);
    plan.files.push(PlannedChange::merged(
        ".claude/settings.json",
        Some("{}\n".to_owned()),
        "{\"a\":1}\n".to_owned(),
    ));
    plan.files.push(PlannedChange::merged(
        ".codex/run",
        Some("#!/bin/sh\n".to_owned()),
        "#!/bin/sh\n".to_owned(),
    ));

    let receipt = apply(dir, &plan, &[]);

    assert_eq!(
        receipt.applied_paths(),
        [".claude/settings.json", ".codex/run"]
    );
    let mode = |path: &str| {
        std::fs::metadata(dir.join(path))
            .unwrap()
            .permissions()
            .mode()
            & 0o777
    };
    assert_eq!(mode(".claude/settings.json"), 0o600);
    assert_eq!(mode(".codex/run"), 0o755);
    assert_eq!(read(dir, ".claude/settings.json"), "{\"a\":1}\n");

    // Nothing left to restore: the launcher is skipped like any unchanged file.
    let receipt = apply(dir, &plan, &[]);
    assert_eq!(receipt.applied_paths(), Vec::<String>::new());
    assert!(matches!(
        receipt.get(".codex/run").unwrap().outcome,
        ApplyOutcome::Skipped { .. }
    ));
}

#[test]
fn a_removal_takes_its_records_and_empty_parents_with_it() {
    let tmp = TempDir::new().unwrap();
    let dir = tmp.path();
    write(dir, ".agents/skills/validate/SKILL.md", "skill\n");
    write(dir, ".agents/skills/next/SKILL.md", "mine\n");
    let mut manifest = ScaffoldManifest::new();
    manifest.record(".agents/skills/validate/SKILL.md", "skill\n");
    manifest.record(".agents/skills/next/SKILL.md", "next\n");
    manifest.save(dir).unwrap();
    let mut plan = ChangePlan::new(AdminOperation::Removal);
    plan.files.push(PlannedChange::removal(
        ".agents/skills/validate/SKILL.md",
        "skill\n".to_owned(),
        Ownership::Generated,
    ));
    plan.files.push(PlannedChange::preserved(
        ".agents/skills/next/SKILL.md",
        Some("mine\n".to_owned()),
        Ownership::Customized,
    ));

    let receipt = apply(dir, &plan, &[]);

    assert_eq!(
        receipt.applied_paths(),
        [".agents/skills/validate/SKILL.md"]
    );
    assert!(!dir.join(".agents/skills/validate").exists());
    assert!(dir.join(".agents/skills/next/SKILL.md").is_file());
    let manifest = ScaffoldManifest::load(dir);
    assert!(
        !manifest
            .files
            .contains_key(".agents/skills/validate/SKILL.md")
    );
    assert!(manifest.files.contains_key(".agents/skills/next/SKILL.md"));
    assert_eq!(receipt.manifest, ManifestOutcome::Saved);
}
