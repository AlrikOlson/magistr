use super::*;
use crate::scaffold::manifest::ScaffoldManifest;

/// Write the managed `.gitignore` section for a Rust project, as setup does.
fn merge_gitignore(dir: &Path) {
    let vars = std::collections::HashMap::from([("language".to_owned(), "rust".to_owned())]);
    let change = crate::scaffold::gitignore_change(dir, &vars);
    std::fs::write(dir.join(".gitignore"), &change.new_content).unwrap();
}

const SHARED_FILES: &[&str] = &[
    ".claude/settings.json",
    ".claude/rules/conventions.md",
    ".claude/skills/next/SKILL.md",
    ".cursor/rules/workflow.mdc",
    ".github/copilot-instructions.md",
    ".clinerules",
    ".windsurfrules",
    ".aider.conf.yml",
    "run-autonomous.sh",
    "format-stream.sh",
];

fn write_file(dir: &Path, path: &str, content: &str) {
    let full = dir.join(path);
    std::fs::create_dir_all(full.parent().unwrap()).unwrap();
    std::fs::write(full, content).unwrap();
}

fn record_files(dir: &Path, paths: &[&str]) {
    let mut manifest = ScaffoldManifest::new();
    for path in paths {
        write_file(dir, path, "generated");
        manifest.record(path, "generated");
    }
    manifest.save(dir).unwrap();
}

#[test]
fn removal_preserves_customized_files_at_known_scaffold_paths() {
    let tmp = tempfile::tempdir().unwrap();
    let dir = tmp.path();
    record_files(dir, SHARED_FILES);
    for path in SHARED_FILES {
        write_file(dir, path, "my changes");
    }

    remove_scaffold(dir).unwrap();

    for path in SHARED_FILES {
        assert_eq!(
            std::fs::read_to_string(dir.join(path)).unwrap(),
            "my changes",
            "{path}"
        );
    }
}

#[test]
fn removal_preserves_unowned_files_with_missing_or_corrupt_manifest() {
    for manifest_content in [None, Some("not valid json")] {
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path();
        for path in SHARED_FILES {
            write_file(dir, path, "mine");
        }
        if let Some(content) = manifest_content {
            write_file(dir, ".magistr/scaffold-manifest.json", content);
        }

        remove_scaffold(dir).unwrap();

        for path in SHARED_FILES {
            assert_eq!(
                std::fs::read_to_string(dir.join(path)).unwrap(),
                "mine",
                "{path}"
            );
        }
    }
}

#[test]
fn removal_preserves_unrecorded_files_beside_a_recorded_file() {
    let tmp = tempfile::tempdir().unwrap();
    let dir = tmp.path();
    record_files(dir, &[".claude/rules/tools.md"]);
    for path in SHARED_FILES {
        write_file(dir, path, "generated");
    }

    remove_scaffold(dir).unwrap();

    assert!(!dir.join(".claude/rules/tools.md").exists());
    for path in SHARED_FILES {
        assert_eq!(
            std::fs::read_to_string(dir.join(path)).unwrap(),
            "generated",
            "{path}"
        );
    }
}

#[test]
fn removal_uses_recorded_bytes_for_all_emitters_and_obsolete_claude_files() {
    let tmp = tempfile::tempdir().unwrap();
    let dir = tmp.path();
    let mut paths = SHARED_FILES.to_vec();
    paths.extend([
        ".claude/skills/retired/SKILL.md",
        "CLAUDE.md",
        "AGENTS.md",
        "ROADMAP.md",
        "src/user.rs",
    ]);
    record_files(dir, &paths);

    remove_scaffold(dir).unwrap();

    for path in SHARED_FILES
        .iter()
        .copied()
        .chain([".claude/skills/retired/SKILL.md"])
    {
        assert!(!dir.join(path).exists(), "{path}");
    }
    for path in ["CLAUDE.md", "AGENTS.md", "ROADMAP.md", "src/user.rs"] {
        assert_eq!(
            std::fs::read_to_string(dir.join(path)).unwrap(),
            "generated",
            "{path}"
        );
    }
    assert!(!dir.join(".claude").exists());
    assert!(!dir.join(".cursor").exists());
}

#[test]
fn removal_does_not_prune_directories_without_a_deleted_file() {
    let tmp = tempfile::tempdir().unwrap();
    let dir = tmp.path();
    record_files(dir, &[".claude/skills/next/SKILL.md"]);
    std::fs::create_dir_all(dir.join(".cursor/rules")).unwrap();
    std::fs::create_dir_all(dir.join(".claude/skills/my-empty-skill")).unwrap();

    remove_scaffold(dir).unwrap();

    assert!(dir.join(".cursor/rules").is_dir());
    assert!(dir.join(".claude/skills/my-empty-skill").is_dir());
    assert!(!dir.join(".claude/skills/next").exists());
}

#[test]
fn removal_rejects_parent_traversal_in_manifest_paths() {
    let tmp = tempfile::tempdir().unwrap();
    let dir = tmp.path().join("project");
    std::fs::create_dir_all(dir.join(".claude")).unwrap();
    write_file(tmp.path(), "victim", "generated");
    let mut manifest = ScaffoldManifest::new();
    manifest.record(".claude/../../victim", "generated");
    manifest.save(&dir).unwrap();

    remove_scaffold(&dir).unwrap();

    assert_eq!(
        std::fs::read_to_string(tmp.path().join("victim")).unwrap(),
        "generated"
    );
}

#[test]
fn removal_preserves_non_text_files_and_non_regular_entries() {
    let tmp = tempfile::tempdir().unwrap();
    let dir = tmp.path();
    record_files(dir, &[".clinerules", ".windsurfrules"]);
    std::fs::write(dir.join(".clinerules"), [0xff, 0xfe]).unwrap();
    std::fs::remove_file(dir.join(".windsurfrules")).unwrap();
    std::fs::create_dir(dir.join(".windsurfrules")).unwrap();

    remove_scaffold(dir).unwrap();

    assert_eq!(
        std::fs::read(dir.join(".clinerules")).unwrap(),
        [0xff, 0xfe]
    );
    assert!(dir.join(".windsurfrules").is_dir());
}

#[cfg(unix)]
#[test]
fn removal_never_follows_symlinked_parent_directories() {
    let tmp = tempfile::tempdir().unwrap();
    let dir = tmp.path().join("project");
    let outside = tmp.path().join("outside");
    write_file(&outside, "rules/tools.md", "generated");
    std::fs::create_dir_all(&dir).unwrap();
    std::os::unix::fs::symlink(&outside, dir.join(".claude")).unwrap();
    let mut manifest = ScaffoldManifest::new();
    manifest.record(".claude/rules/tools.md", "generated");
    manifest.save(&dir).unwrap();

    remove_scaffold(&dir).unwrap();

    assert_eq!(
        std::fs::read_to_string(outside.join("rules/tools.md")).unwrap(),
        "generated"
    );
    assert!(
        std::fs::symlink_metadata(dir.join(".claude"))
            .unwrap()
            .is_symlink()
    );
}

#[cfg(unix)]
#[test]
fn removal_preserves_symlink_files_even_when_target_bytes_match() {
    let tmp = tempfile::tempdir().unwrap();
    let dir = tmp.path();
    write_file(dir, "user-script", "generated");
    std::os::unix::fs::symlink("user-script", dir.join("run-autonomous.sh")).unwrap();
    std::os::unix::fs::symlink("missing", dir.join(".clinerules")).unwrap();
    let mut manifest = ScaffoldManifest::new();
    manifest.record("run-autonomous.sh", "generated");
    manifest.record(".clinerules", "generated");
    manifest.save(dir).unwrap();

    remove_scaffold(dir).unwrap();

    assert!(
        std::fs::symlink_metadata(dir.join("run-autonomous.sh"))
            .unwrap()
            .is_symlink()
    );
    assert!(
        std::fs::symlink_metadata(dir.join(".clinerules"))
            .unwrap()
            .is_symlink()
    );
    assert_eq!(
        std::fs::read_to_string(dir.join("user-script")).unwrap(),
        "generated"
    );
}

#[test]
fn remove_magistr_gitignore_entries_preserves_user() {
    let tmp = tempfile::TempDir::new().unwrap();
    let gi_path = tmp.path().join(".gitignore");

    // Generate with user content
    std::fs::write(&gi_path, "# my patterns\nvendor/\n").unwrap();
    merge_gitignore(tmp.path());

    // Remove magistr section
    remove_magistr_gitignore_entries(tmp.path()).unwrap();

    let content = std::fs::read_to_string(&gi_path).unwrap();
    assert!(content.contains("vendor/"), "user pattern preserved");
    assert!(
        !content.contains(GITIGNORE_SECTION_START),
        "magistr section removed"
    );
}

#[test]
fn remove_magistr_gitignore_entries_deletes_empty_file() {
    let tmp = tempfile::TempDir::new().unwrap();
    let gi_path = tmp.path().join(".gitignore");

    // Generate with no user content
    merge_gitignore(tmp.path());
    assert!(gi_path.exists());

    // Remove magistr section — file should be deleted
    remove_magistr_gitignore_entries(tmp.path()).unwrap();
    assert!(!gi_path.exists(), "empty .gitignore should be deleted");
}
