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
    let path = inputs.project_json();
    assert_eq!(path, dir.path().join(".magistr/project.json"));

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
    assert_eq!(
        other.project_json(),
        PathBuf::from("/other/.magistr/project.json")
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
