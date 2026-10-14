use super::*;
use crate::config::inputs::InputOrigin;

fn lookup(vars: Vec<(&'static str, &'static str)>) -> impl Fn(&str) -> Option<String> {
    move |key| {
        vars.iter()
            .find(|(k, _)| *k == key)
            .map(|(_, v)| (*v).to_string())
    }
}

#[test]
fn xdg_config_home_wins_over_home_with_provenance() {
    let inputs = ProjectInputs::from_lookup(
        "/proj",
        lookup(vec![("XDG_CONFIG_HOME", "/xdg"), ("HOME", "/home/u")]),
    );
    assert_eq!(inputs.config_dir(), Some(Path::new("/xdg/magistr")));
    assert_eq!(
        inputs.sources()["user.config_dir"],
        InputSource::Environment("XDG_CONFIG_HOME")
    );
    assert_eq!(
        inputs.user_config_path(),
        Some(PathBuf::from("/xdg/magistr/config.toml"))
    );
    assert_eq!(
        inputs.user_preset_dir(),
        Some(PathBuf::from("/xdg/magistr/presets"))
    );
    assert_eq!(
        inputs.models_config_path(),
        Some(PathBuf::from("/xdg/magistr/models.toml"))
    );
    assert_eq!(
        inputs.plugins_dir(),
        Some(PathBuf::from("/xdg/magistr/plugins"))
    );
    assert_eq!(
        inputs.org_plugin_config_path(),
        Some(PathBuf::from("/xdg/magistr/org-plugins.json"))
    );
}

#[test]
fn blank_xdg_falls_back_to_home() {
    let inputs = ProjectInputs::from_lookup(
        "/proj",
        lookup(vec![("XDG_CONFIG_HOME", "  "), ("HOME", "/home/u")]),
    );
    assert_eq!(
        inputs.config_dir(),
        Some(Path::new("/home/u/.config/magistr"))
    );
    assert_eq!(
        inputs.sources()["user.config_dir"],
        InputSource::Environment("HOME")
    );
}

#[test]
fn no_home_leaves_user_paths_unlocated() {
    let inputs = ProjectInputs::from_lookup("/proj", lookup(vec![]));
    assert_eq!(inputs.config_dir(), None);
    assert_eq!(inputs.sources()["user.config_dir"], InputSource::Default);
    assert_eq!(inputs.user_config_path(), None);
    assert_eq!(inputs.plugins_dir(), None);
    let cfg = inputs.user_config().unwrap();
    assert_eq!(cfg.origin, InputOrigin::Unlocated);
    assert_eq!(cfg.value, UserConfig::default());
    assert!(
        inputs
            .user_preset_override(Language::Rust)
            .unwrap()
            .is_none()
    );
    assert!(inputs.user_preset_info().is_empty());
    assert!(inputs.preset_layers().user_preset_dir.is_none());
}

#[test]
fn user_config_distinguishes_absent_loaded_and_unreadable() {
    let dir = tempfile::TempDir::new().unwrap();
    let inputs = ProjectInputs::from_lookup("/proj", lookup(vec![])).with_config_dir(dir.path());
    assert_eq!(inputs.sources()["user.config_dir"], InputSource::Explicit);
    let path = dir.path().join("config.toml");

    let absent = inputs.user_config().unwrap();
    assert_eq!(absent.origin, InputOrigin::Absent(path.clone()));
    assert_eq!(absent.value, UserConfig::default());

    std::fs::write(&path, "[general]\nmax_iterations = 7\n").unwrap();
    let loaded = inputs.user_config().unwrap();
    assert_eq!(loaded.origin, InputOrigin::Loaded(path.clone()));
    assert!(loaded.origin.is_loaded());
    assert_eq!(loaded.value.general.max_iterations, Some(7));

    std::fs::write(&path, "not = [valid").unwrap();
    assert!(matches!(
        inputs.user_config(),
        Err(UserConfigError::Parse(p, _)) if p == path
    ));
}

#[test]
fn project_config_distinguishes_absent_loaded_and_invalid() {
    let dir = tempfile::TempDir::new().unwrap();
    let inputs = ProjectInputs::from_lookup(dir.path(), lookup(vec![]));
    let path = inputs.project_json().unwrap();
    assert_eq!(
        path,
        dir.path()
            .canonicalize()
            .unwrap()
            .join(".magistr/project.json")
    );

    let absent = inputs.project_config().unwrap();
    assert_eq!(absent.origin, InputOrigin::Absent(path.clone()));
    assert!(absent.value.is_none());

    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(&path, "{ not json").unwrap();
    assert!(matches!(
        inputs.project_config(),
        Err(MagistrError::Config(msg)) if msg.contains("invalid project config")
    ));
}

#[test]
fn provider_and_plugin_inputs_are_captured_without_secret_values() {
    let inputs = ProjectInputs::from_lookup(
        "/proj",
        lookup(vec![
            ("MAGISTR_AGENT_BACKEND", " acp "),
            ("ANTHROPIC_API_KEY", "sk-ant-secret-value"),
            (
                "MAGISTR_PLUGIN_INDEX_URL",
                "https://plugins.example/index.json",
            ),
        ]),
    );
    assert_eq!(inputs.agent_backend(), Some("acp"));
    assert!(inputs.has_claude_api_key());
    assert_eq!(
        inputs.plugin_index_url(),
        Some("https://plugins.example/index.json")
    );
    assert_eq!(
        inputs.sources()["provider.backend"],
        InputSource::Environment("MAGISTR_AGENT_BACKEND")
    );
    assert_eq!(
        inputs.sources()["provider.claude_api_key"],
        InputSource::Environment("ANTHROPIC_API_KEY")
    );
    assert_eq!(
        inputs.sources()["plugin.index_url"],
        InputSource::Environment("MAGISTR_PLUGIN_INDEX_URL")
    );
    let rendered = format!("{inputs:?}");
    assert!(
        !rendered.contains("sk-ant"),
        "the key value must never be captured: {rendered}"
    );
}

#[test]
fn blank_provider_inputs_are_defaults() {
    let inputs = ProjectInputs::from_lookup(
        "/proj",
        lookup(vec![
            ("MAGISTR_AGENT_BACKEND", ""),
            ("ANTHROPIC_API_KEY", " "),
        ]),
    );
    assert_eq!(inputs.agent_backend(), None);
    assert!(!inputs.has_claude_api_key());
    assert_eq!(inputs.sources()["provider.backend"], InputSource::Default);
    assert_eq!(
        inputs.sources()["provider.claude_api_key"],
        InputSource::Default
    );
}

#[test]
fn explicit_provider_overrides_record_their_provenance() {
    let inputs = ProjectInputs::from_lookup("/proj", lookup(vec![]))
        .with_agent_backend(Some(" auto ".into()))
        .with_claude_api_key(true);
    assert_eq!(inputs.agent_backend(), Some("auto"));
    assert!(inputs.has_claude_api_key());
    assert_eq!(inputs.sources()["provider.backend"], InputSource::Explicit);
    assert_eq!(
        inputs.sources()["provider.claude_api_key"],
        InputSource::Explicit
    );
}

#[test]
fn workspace_env_keeps_only_the_layer_keys_and_drops_blanks() {
    let inputs = ProjectInputs::from_lookup(
        "/proj",
        lookup(vec![
            ("WORKBENCH_USER_CONFIG", "/etc/wb/user.toml"),
            ("WORKBENCH_ORG_POLICY", ""),
            ("HOME", "/home/u"),
            ("WORKBENCH_NETWORK_FS", "1"),
            ("WORKBENCH_SESSION_ID", " session-9 "),
            ("PATH", "/usr/bin"),
        ]),
    );
    let env = inputs.workspace_env();
    assert_eq!(
        env.get("WORKBENCH_USER_CONFIG").unwrap(),
        "/etc/wb/user.toml"
    );
    assert_eq!(env.get("HOME").unwrap(), "/home/u");
    assert!(!env.contains_key("WORKBENCH_ORG_POLICY"));
    assert!(!env.contains_key("PATH"));
    assert_eq!(inputs.workspace_network_override(), Some("1"));
    assert_eq!(inputs.workspace_session_id(), Some("session-9"));
    assert_eq!(
        inputs.sources()["workspace.user_config"],
        InputSource::Environment("WORKBENCH_USER_CONFIG")
    );
    assert_eq!(
        inputs.sources()["workspace.org_policy"],
        InputSource::Default
    );
    assert_eq!(
        inputs.sources()["workspace.network_fs"],
        InputSource::Environment("WORKBENCH_NETWORK_FS")
    );

    let options = inputs.workspace_resolve_options();
    assert_eq!(options.root, PathBuf::from("/proj"));
    let paths = options.layer_paths();
    assert_eq!(paths[1].1, PathBuf::from("/etc/wb/user.toml"));
}

#[test]
fn each_key_is_asked_for_at_most_once() {
    let calls = std::cell::RefCell::new(BTreeMap::<String, usize>::new());
    let _ = ProjectInputs::from_lookup("/proj", |key| {
        *calls.borrow_mut().entry(key.to_string()).or_default() += 1;
        None
    });
    assert!(
        calls.borrow().values().all(|&n| n == 1),
        "keys read more than once: {:?}",
        calls.borrow()
    );
}

#[test]
fn for_project_rebinds_the_root_and_keeps_user_inputs() {
    let inputs = ProjectInputs::from_lookup("/proj", lookup(vec![("HOME", "/home/u")]));
    let other = inputs.for_project("/other");
    assert_eq!(other.project_root(), Path::new("/other"));
    assert!(
        other.project_json().is_err(),
        "unavailable roots cannot select a settings file"
    );
    assert_eq!(
        other.project_preset_dir(),
        PathBuf::from("/other/.magistr/presets")
    );
    assert_eq!(other.config_dir(), inputs.config_dir());
    assert_eq!(other.sources()["project_root"], InputSource::Explicit);
    assert_eq!(
        other.workspace_resolve_options().root,
        PathBuf::from("/other")
    );
}

#[test]
fn preset_layers_apply_the_captured_user_override_directory() {
    let dir = tempfile::TempDir::new().unwrap();
    let presets = dir.path().join("presets");
    std::fs::create_dir_all(&presets).unwrap();
    std::fs::write(
        presets.join("rust.json"),
        r#"{"quality_gate": {"check": "cargo check --all-features"}}"#,
    )
    .unwrap();
    let inputs = ProjectInputs::from_lookup("/proj", lookup(vec![])).with_config_dir(dir.path());
    let layers = inputs.preset_layers();
    assert_eq!(layers.user_preset_dir.as_deref(), Some(presets.as_path()));
    let preset = crate::config::preset::load_preset_layered(
        Language::Rust,
        crate::types::Framework::None,
        crate::types::PackageManager::Cargo,
        &layers,
    )
    .unwrap();
    assert_eq!(preset.quality_gate.check, "cargo check --all-features");
    let info = inputs.user_preset_info();
    assert_eq!(info.len(), 1);
    assert_eq!(info[0].source, "user-global");
}

// ── Remaining ambient reads (config-inputs-remaining-ambient-reads) ─────

#[test]
fn explicit_data_state_and_codex_variables_win_over_home() {
    let inputs = ProjectInputs::from_lookup(
        "/proj",
        lookup(vec![
            ("HOME", "/home/u"),
            ("XDG_DATA_HOME", "/xdg-data"),
            ("MAGISTR_HOME", "/state"),
            ("CODEX_HOME", "/codex"),
        ]),
    );
    assert_eq!(inputs.home_dir(), Some(Path::new("/home/u")));
    assert_eq!(
        inputs.sources()["user.home"],
        InputSource::Environment("HOME")
    );
    assert_eq!(inputs.data_dir(), Some(Path::new("/xdg-data/magistr")));
    assert_eq!(
        inputs.sources()["user.data_dir"],
        InputSource::Environment("XDG_DATA_HOME")
    );
    assert_eq!(inputs.state_dir(), Some(Path::new("/state")));
    assert_eq!(
        inputs.sources()["user.state_dir"],
        InputSource::Environment("MAGISTR_HOME")
    );
    assert_eq!(inputs.codex_home(), Some(Path::new("/codex")));
    assert_eq!(
        inputs.sources()["codex.home"],
        InputSource::Environment("CODEX_HOME")
    );
}

#[test]
fn data_state_and_codex_dirs_derive_from_the_captured_home() {
    let inputs = ProjectInputs::from_lookup("/proj", lookup(vec![("HOME", "/home/u")]));
    assert_eq!(
        inputs.data_dir(),
        Some(Path::new("/home/u/.local/share/magistr"))
    );
    assert_eq!(
        inputs.sources()["user.data_dir"],
        InputSource::Environment("HOME")
    );
    assert_eq!(inputs.state_dir(), Some(Path::new("/home/u/.magistr")));
    assert_eq!(
        inputs.sources()["user.state_dir"],
        InputSource::Environment("HOME")
    );
    assert_eq!(inputs.codex_home(), Some(Path::new("/home/u/.codex")));
    assert_eq!(
        inputs.sources()["codex.home"],
        InputSource::Environment("HOME")
    );

    // USERPROFILE is the home on Windows; it stands in wherever HOME would.
    let windows = ProjectInputs::from_lookup(
        "/proj",
        lookup(vec![("USERPROFILE", "C:/Users/u"), ("HOME", " ")]),
    );
    assert_eq!(windows.home_dir(), Some(Path::new("C:/Users/u")));
    assert_eq!(
        windows.sources()["user.home"],
        InputSource::Environment("USERPROFILE")
    );
    assert_eq!(
        windows.config_dir(),
        Some(Path::new("C:/Users/u/.config/magistr"))
    );
    assert_eq!(
        windows.sources()["user.config_dir"],
        InputSource::Environment("USERPROFILE")
    );
    assert_eq!(windows.state_dir(), Some(Path::new("C:/Users/u/.magistr")));
    assert_eq!(windows.codex_home(), Some(Path::new("C:/Users/u/.codex")));
}

#[test]
fn local_app_data_is_the_last_data_dir_fallback_and_nothing_means_unlocated() {
    let local = ProjectInputs::from_lookup(
        "/proj",
        lookup(vec![("LOCALAPPDATA", "C:/Users/u/AppData/Local")]),
    );
    assert_eq!(
        local.data_dir(),
        Some(Path::new("C:/Users/u/AppData/Local/magistr"))
    );
    assert_eq!(
        local.sources()["user.data_dir"],
        InputSource::Environment("LOCALAPPDATA")
    );
    assert_eq!(local.home_dir(), None);
    assert_eq!(local.state_dir(), None);
    assert_eq!(local.codex_home(), None);

    let nothing = ProjectInputs::from_lookup("/proj", lookup(vec![]));
    assert_eq!(nothing.data_dir(), None);
    assert_eq!(nothing.sources()["user.data_dir"], InputSource::Default);
    assert_eq!(nothing.sources()["user.home"], InputSource::Default);
    assert_eq!(nothing.sources()["user.state_dir"], InputSource::Default);
    assert_eq!(nothing.sources()["codex.home"], InputSource::Default);
}

#[test]
fn explicit_data_and_state_dirs_record_their_provenance() {
    let inputs = ProjectInputs::from_lookup("/proj", lookup(vec![("HOME", "/home/u")]))
        .with_data_dir("/tmp/data")
        .with_state_dir("/tmp/state");
    assert_eq!(inputs.data_dir(), Some(Path::new("/tmp/data")));
    assert_eq!(inputs.sources()["user.data_dir"], InputSource::Explicit);
    assert_eq!(inputs.state_dir(), Some(Path::new("/tmp/state")));
    assert_eq!(inputs.sources()["user.state_dir"], InputSource::Explicit);
    // The override moves only the directory it names.
    assert_eq!(inputs.home_dir(), Some(Path::new("/home/u")));
    assert_eq!(inputs.codex_home(), Some(Path::new("/home/u/.codex")));
}

#[test]
fn identity_inputs_capture_the_override_the_os_user_and_ci() {
    let inputs = ProjectInputs::from_lookup(
        "/proj",
        lookup(vec![
            ("MAGISTR_USER", " Alice <alice@example.dev> "),
            ("USER", "alice"),
            ("USERNAME", "ALICE"),
            ("CI", "true"),
        ]),
    );
    assert_eq!(inputs.user_override(), Some("Alice <alice@example.dev>"));
    assert_eq!(
        inputs.sources()["identity.user"],
        InputSource::Environment("MAGISTR_USER")
    );
    assert_eq!(inputs.os_user(), Some("alice"));
    assert_eq!(
        inputs.sources()["identity.os_user"],
        InputSource::Environment("USER")
    );
    assert!(inputs.is_ci());
    assert_eq!(
        inputs.sources()["identity.ci"],
        InputSource::Environment("CI")
    );

    let windows = ProjectInputs::from_lookup(
        "/proj",
        lookup(vec![("USER", ""), ("USERNAME", "ALICE"), ("CI", "0")]),
    );
    assert_eq!(windows.user_override(), None);
    assert_eq!(windows.sources()["identity.user"], InputSource::Default);
    assert_eq!(windows.os_user(), Some("ALICE"));
    assert_eq!(
        windows.sources()["identity.os_user"],
        InputSource::Environment("USERNAME")
    );
    assert!(!windows.is_ci(), "CI=0 is not a CI runner");
    assert_eq!(windows.sources()["identity.ci"], InputSource::Default);

    for value in ["false", "FALSE", " ", ""] {
        let inputs = ProjectInputs::from_lookup("/proj", lookup(vec![("CI", value)]));
        assert!(!inputs.is_ci(), "CI={value:?} is not a CI runner");
    }
    for value in ["1", "yes", "github"] {
        let inputs = ProjectInputs::from_lookup("/proj", lookup(vec![("CI", value)]));
        assert!(inputs.is_ci(), "CI={value:?} is a CI runner");
    }
}

#[test]
fn session_inputs_capture_the_goal_mode_the_budget_exports_and_the_login_client() {
    let inputs = ProjectInputs::from_lookup(
        "/proj",
        lookup(vec![
            ("MAGISTR_GOAL", " Auto "),
            ("MAGISTR_BUDGET", "5.50"),
            ("MAGISTR_BUDGET_REQUESTED", "12"),
            ("MAGISTR_GITHUB_CLIENT_ID", "Iv1.abc"),
        ]),
    );
    assert_eq!(inputs.goal_mode(), Some("Auto"));
    assert_eq!(
        inputs.sources()["session.goal_mode"],
        InputSource::Environment("MAGISTR_GOAL")
    );
    assert_eq!(inputs.budget_limit(), Some(5.5));
    assert_eq!(
        inputs.sources()["session.budget_limit"],
        InputSource::Environment("MAGISTR_BUDGET")
    );
    assert_eq!(inputs.budget_requested(), Some(12.0));
    assert_eq!(
        inputs.sources()["session.budget_requested"],
        InputSource::Environment("MAGISTR_BUDGET_REQUESTED")
    );
    assert_eq!(inputs.github_client_id(), Some("Iv1.abc"));
    assert_eq!(
        inputs.sources()["login.github_client_id"],
        InputSource::Environment("MAGISTR_GITHUB_CLIENT_ID")
    );
}

#[test]
fn unparseable_or_nonpositive_budget_exports_are_defaults() {
    for (limit, requested) in [("abc", "-1"), ("0", "inf"), ("NaN", " ")] {
        let inputs = ProjectInputs::from_lookup(
            "/proj",
            lookup(vec![
                ("MAGISTR_BUDGET", limit),
                ("MAGISTR_BUDGET_REQUESTED", requested),
            ]),
        );
        assert_eq!(inputs.budget_limit(), None, "MAGISTR_BUDGET={limit:?}");
        assert_eq!(
            inputs.sources()["session.budget_limit"],
            InputSource::Default
        );
        assert_eq!(
            inputs.budget_requested(),
            None,
            "MAGISTR_BUDGET_REQUESTED={requested:?}"
        );
        assert_eq!(
            inputs.sources()["session.budget_requested"],
            InputSource::Default
        );
    }
    let nothing = ProjectInputs::from_lookup("/proj", lookup(vec![]));
    assert_eq!(nothing.goal_mode(), None);
    assert_eq!(nothing.sources()["session.goal_mode"], InputSource::Default);
    assert_eq!(nothing.github_client_id(), None);
    assert_eq!(
        nothing.sources()["login.github_client_id"],
        InputSource::Default
    );
}

#[test]
fn the_governed_budget_binds_explicitly_over_the_exports() {
    let inputs = ProjectInputs::from_lookup(
        "/proj",
        lookup(vec![
            ("MAGISTR_BUDGET", "5.50"),
            ("MAGISTR_BUDGET_REQUESTED", "12"),
        ]),
    )
    .with_budget(Some(2.0), Some(12.0));
    assert_eq!(inputs.budget_limit(), Some(2.0));
    assert_eq!(
        inputs.sources()["session.budget_limit"],
        InputSource::Explicit
    );
    assert_eq!(inputs.budget_requested(), Some(12.0));
    assert_eq!(
        inputs.sources()["session.budget_requested"],
        InputSource::Explicit
    );

    // A decision with no limit clears what the environment exported: the
    // host decided, and nothing capped the session.
    let cleared = inputs.with_budget(None, Some(f64::NAN));
    assert_eq!(cleared.budget_limit(), None);
    assert_eq!(cleared.budget_requested(), None);
    assert_eq!(
        cleared.sources()["session.budget_limit"],
        InputSource::Explicit
    );
}

#[test]
fn for_project_keeps_the_machine_inputs() {
    let inputs = ProjectInputs::from_lookup(
        "/proj",
        lookup(vec![
            ("HOME", "/home/u"),
            ("MAGISTR_USER", "alice"),
            ("CI", "1"),
            ("MAGISTR_GOAL", "off"),
        ]),
    )
    .with_budget(Some(3.0), None);
    let other = inputs.for_project("/other");
    assert_eq!(other.data_dir(), inputs.data_dir());
    assert_eq!(other.state_dir(), inputs.state_dir());
    assert_eq!(other.user_override(), Some("alice"));
    assert!(other.is_ci());
    assert_eq!(other.goal_mode(), Some("off"));
    assert_eq!(other.budget_limit(), Some(3.0));
}
