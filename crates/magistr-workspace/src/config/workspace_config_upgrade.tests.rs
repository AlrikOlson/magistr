use super::*;

#[test]
fn preview_and_apply_preserve_comments_commands_and_archive_original_once() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    let original =
        "# My commands\n[commands]\ntest = 'npm test'\n[edit]\nconfirm_mode = 'elicit'\n";
    std::fs::write(root.join("workbench.toml"), original).unwrap();
    assert_eq!(
        WorkspaceConfigUpgrade::preview(root).unwrap().as_deref(),
        Some(original)
    );
    assert!(!root.join(".magistr").exists());
    WorkspaceConfigUpgrade::apply(root).unwrap();
    assert_eq!(
        std::fs::read_to_string(root.join(".magistr/workspace.toml")).unwrap(),
        original
    );
    assert!(!root.join("workbench.toml").exists());
    let files: Vec<_> = std::fs::read_dir(root.join(".magistr/upgrades/workspace-config"))
        .unwrap()
        .map(|e| e.unwrap().path())
        .collect();
    assert_eq!(files.len(), 1);
    assert_eq!(std::fs::read_to_string(&files[0]).unwrap(), original);
    // Retry the crash window after publishing current settings but before removing old.
    std::fs::write(root.join("workbench.toml"), original).unwrap();
    WorkspaceConfigUpgrade::apply(root).unwrap();
    WorkspaceConfigUpgrade::apply(root).unwrap();
    assert_eq!(
        std::fs::read_dir(root.join(".magistr/upgrades/workspace-config"))
            .unwrap()
            .count(),
        1
    );
}

#[test]
fn coexistence_merges_missing_keys_but_never_guesses_between_conflicting_settings() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    std::fs::create_dir(root.join(".magistr")).unwrap();
    let old = "# Old\n[commands]\ntest = 'npm test'\n";
    let current =
        "# Current\n[commands]\ncheck = 'npm run check'\n[edit]\nconfirm_mode = 'elicit'\n";
    std::fs::write(root.join("workbench.toml"), old).unwrap();
    std::fs::write(root.join(".magistr/workspace.toml"), current).unwrap();
    let merged = WorkspaceConfigUpgrade::preview(root).unwrap().unwrap();
    assert!(merged.contains("# Current"));
    assert!(merged.contains("npm test") && merged.contains("npm run check"));
    assert!(merged.contains("elicit"));
    std::fs::write(
        root.join("workbench.toml"),
        "[commands]\ncheck = 'different'\n",
    )
    .unwrap();
    let error = WorkspaceConfigUpgrade::apply(root).unwrap_err().to_string();
    assert!(error.contains("commands.check"), "{error}");
    assert_eq!(
        std::fs::read_to_string(root.join(".magistr/workspace.toml")).unwrap(),
        current
    );
    assert!(!root.join(".magistr/upgrades").exists());
}

#[test]
fn malformed_or_newer_settings_are_never_replaced_with_defaults() {
    for content in ["[broken", "config_version = 9\n"] {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("workbench.toml"), content).unwrap();
        assert!(WorkspaceConfigUpgrade::apply(dir.path()).is_err());
        assert_eq!(
            std::fs::read_to_string(dir.path().join("workbench.toml")).unwrap(),
            content
        );
        assert!(!dir.path().join(".magistr").exists());
    }
}

#[cfg(unix)]
#[test]
fn configuration_upgrade_does_not_follow_links() {
    let dir = tempfile::tempdir().unwrap();
    let external = tempfile::NamedTempFile::new().unwrap();
    std::fs::write(external.path(), "[commands]\ntest='true'\n").unwrap();
    std::os::unix::fs::symlink(external.path(), dir.path().join("workbench.toml")).unwrap();
    assert!(WorkspaceConfigUpgrade::apply(dir.path()).is_err());
    assert!(!dir.path().join(".magistr").exists());
}
