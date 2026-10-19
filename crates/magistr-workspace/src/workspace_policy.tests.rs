use super::*;

fn options(root: &Path) -> ResolveOptions {
    let mut options = ResolveOptions::new(root);
    options.env.insert(
        "MAGISTR_WORKSPACE_ORG_POLICY".into(),
        root.join("policy.toml").display().to_string(),
    );
    options.env.insert(
        "MAGISTR_WORKSPACE_USER_CONFIG".into(),
        root.join("user.toml").display().to_string(),
    );
    options
}

#[test]
fn loaded_layers_and_additive_paths_are_automatically_protected() {
    let temp = tempfile::tempdir().unwrap();
    std::fs::write(
        temp.path().join("policy.toml"),
        "[security]\nprotected_paths = ['secrets/**']\n",
    )
    .unwrap();
    std::fs::write(
        temp.path().join("user.toml"),
        "[security]\nprotected_paths = ['deployment']\n",
    )
    .unwrap();
    let policy = WorkspacePolicy::resolve(&options(temp.path())).unwrap();
    for path in [
        "policy.toml",
        "user.toml",
        ".magistr/workspace.toml",
        "secrets/token",
        "deployment/new",
    ] {
        assert_eq!(
            policy.check_write(path).unwrap_err().code(),
            "PROTECTED_PATH",
            "{path}"
        );
        assert!(policy.check_read(path).is_ok(), "{path}");
    }
    assert!(policy.check_write("src/policy.rs").is_ok());
}

#[test]
fn unsupported_protection_patterns_are_explicit_errors() {
    let temp = tempfile::tempdir().unwrap();
    std::fs::write(
        temp.path().join("policy.toml"),
        "[security]\nprotected_paths = ['secrets/*.pem']\n",
    )
    .unwrap();
    let error = WorkspacePolicy::resolve(&options(temp.path()))
        .err()
        .unwrap();
    assert_eq!(error.code(), "CONFIG_POLICY_DENIED");
    assert!(error.message.contains("protected_paths"));
}

#[test]
fn declared_commands_remain_pinned_while_workspace_files_are_mutable() {
    let temp = tempfile::tempdir().unwrap();
    std::fs::write(
        temp.path().join("policy.toml"),
        "[commands]\ntest = 'cargo test'\n",
    )
    .unwrap();
    std::fs::create_dir_all(temp.path().join(".magistr")).unwrap();
    std::fs::write(
        temp.path().join(".magistr/workspace.toml"),
        "[commands]\ntest = 'sh -c dangerous'\n",
    )
    .unwrap();
    let policy = WorkspacePolicy::resolve(&options(temp.path())).unwrap();
    assert_eq!(
        policy.admit_command("test", &["--lib".into()]).unwrap(),
        ["cargo", "test", "--lib"]
    );
    assert_eq!(
        policy.admit_command("unknown", &[]).unwrap_err().code(),
        "COMMAND_NOT_DECLARED"
    );
    assert_eq!(policy.config().violations.len(), 1);
    assert!(policy.check_write("src/lib.rs").is_ok());
    assert_eq!(
        policy
            .check_write(".magistr/workspace.toml")
            .unwrap_err()
            .code(),
        "PROTECTED_PATH"
    );
}

#[cfg(unix)]
#[test]
fn configuration_alias_and_absent_layer_cannot_reopen_the_allowlist() {
    let temp = tempfile::tempdir().unwrap();
    std::fs::create_dir(temp.path().join("admin")).unwrap();
    std::fs::write(temp.path().join("admin/settings.toml"), "[commands]\n").unwrap();
    std::os::unix::fs::symlink("admin/settings.toml", temp.path().join("policy.toml")).unwrap();
    let policy = WorkspacePolicy::resolve(&options(temp.path())).unwrap();
    for path in ["policy.toml", "admin/settings.toml", "user.toml"] {
        assert_eq!(
            policy.check_write(path).unwrap_err().code(),
            "PROTECTED_PATH",
            "{path}"
        );
    }
    assert_eq!(
        policy.admit_command("test", &[]).unwrap_err().code(),
        "COMMAND_NOT_DECLARED"
    );
}
