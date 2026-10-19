use super::*;

#[test]
fn clean_launch_has_no_implied_project_or_provider() {
    let cli = Desktop::try_parse_from(["desktop"]).unwrap();
    assert!(cli.dir.is_none());
    assert!(cli.provider.is_none());
}

#[path = "args_history.tests.rs"]
mod history;

#[test]
fn worker_role_is_exact_and_never_opens_a_window() {
    let argv = |args: &[&str]| args.iter().map(OsString::from).collect::<Vec<_>>();
    assert!(is_worker(&argv(&["desktop", "--workspace-worker"])));
    for args in [
        vec!["desktop"],
        vec!["desktop", "--workspace-worker", "extra"],
    ] {
        assert!(!is_worker(&argv(&args)));
    }
    assert!(Desktop::try_parse_from(["desktop", "--workspace-worker", "extra"]).is_err());
}

#[test]
fn child_roles_keep_paths_and_tool_arguments_and_reject_session_options() {
    let cli = Desktop::try_parse_from([
        "desktop",
        "acp-agent",
        "--provider",
        "codex",
        "--allow-tool=Read",
    ])
    .unwrap();
    assert!(
        matches!(cli.child, Some(Child::AcpAgent { provider, allow_tool }) if provider == "codex" && allow_tool == ["Read"])
    );
    let cli = Desktop::try_parse_from(["desktop", "mcp", "--dir", "/tmp/a project"]).unwrap();
    assert!(
        matches!(cli.child, Some(Child::Mcp { dir, access: Access::Full, .. }) if dir == std::path::Path::new("/tmp/a project"))
    );
    for args in [
        vec!["desktop", "acp-agent"],
        vec!["desktop", "mcp-launch", "unknown", "."],
        vec![
            "desktop",
            "--provider",
            "ollama",
            "acp-agent",
            "--provider",
            "codex",
        ],
        vec!["desktop", "--resume", "saved", "--continue"],
    ] {
        assert!(Desktop::try_parse_from(args).is_err());
    }
}

#[test]
fn selected_project_and_session_choices_reach_the_shared_config() {
    let root = tempfile::tempdir().unwrap();
    let cli = Desktop::try_parse_from([
        "desktop",
        "--dir",
        root.path().to_str().unwrap(),
        "--provider",
        "codex",
        "--model",
        "chosen-model",
        "--trust",
        "cautious",
        "hello",
    ])
    .unwrap();
    let config = cli
        .config_for(
            root.path(),
            &ProjectInputs::from_lookup(root.path(), |_| None),
            true,
        )
        .unwrap();
    assert_eq!(config.cwd, root.path().canonicalize().unwrap());
    assert_eq!(config.inputs().project_root(), config.cwd);
    assert_eq!(config.provider.as_deref(), Some("codex"));
    assert_eq!(config.model.as_deref(), Some("chosen-model"));
    assert_eq!(config.resume, None);
    assert_eq!(config.prompt, "hello");
    assert!(!config.continue_conversation);
    assert_eq!(
        config.permission_profile,
        Some(magistr::permissions::PermissionProfile::Cautious)
    );
}

/// A project the window switches to keeps the launch's provider, model and
/// permission choice, and takes none of the first session's message or resume.
#[test]
fn a_switched_project_keeps_the_choices_and_drops_the_first_sessions_opening() {
    let first = tempfile::tempdir().unwrap();
    let other = tempfile::tempdir().unwrap();
    let cli = Desktop::try_parse_from([
        "desktop",
        "--dir",
        first.path().to_str().unwrap(),
        "--provider",
        "codex",
        "--model",
        "chosen-model",
        "--trust",
        "cautious",
        "hello",
    ])
    .unwrap();
    let config = cli
        .config_for(
            other.path(),
            &ProjectInputs::from_lookup(first.path(), |_| None),
            false,
        )
        .unwrap();
    assert_eq!(config.cwd, other.path().canonicalize().unwrap());
    assert_eq!(config.inputs().project_root(), config.cwd);
    assert_eq!(config.provider.as_deref(), Some("codex"));
    assert_eq!(config.model.as_deref(), Some("chosen-model"));
    assert_eq!(config.resume, None);
    assert_eq!(config.prompt, "");
    assert!(!config.continue_conversation);
    assert_eq!(
        config.permission_profile,
        Some(magistr::permissions::PermissionProfile::Cautious)
    );
    assert!(
        cli.config_for(
            &other.path().join("missing"),
            &ProjectInputs::from_lookup(first.path(), |_| None),
            false
        )
        .is_err()
    );
}

#[test]
fn invalid_project_or_permission_choice_fails_before_admission() {
    let root = tempfile::tempdir().unwrap();
    let file = root.path().join("file");
    std::fs::write(&file, "data").unwrap();
    let cli = Desktop::try_parse_from(["desktop", "--dir", file.to_str().unwrap()]).unwrap();
    assert!(
        cli.config_for(
            &file,
            &ProjectInputs::from_lookup(root.path(), |_| None),
            true
        )
        .err()
        .unwrap()
        .to_string()
        .contains("not a directory")
    );
    let cli = Desktop::try_parse_from([
        "desktop",
        "--dir",
        root.path().to_str().unwrap(),
        "--trust",
        "typo",
    ])
    .unwrap();
    assert!(
        cli.config_for(
            root.path(),
            &ProjectInputs::from_lookup(root.path(), |_| None),
            true
        )
        .is_err()
    );
}
