use super::*;

fn lookup(vars: Vec<(&'static str, &'static str)>) -> impl Fn(&str) -> Option<String> {
    move |key| {
        vars.iter()
            .find(|(k, _)| *k == key)
            .map(|(_, v)| (*v).to_string())
    }
}

#[test]
fn injected_temporary_store_uses_only_injected_tmpdir() {
    let cfg = NativeConfig::from_lookup("/proj", lookup(vec![("TMPDIR", "/injected/tmp")]));
    assert_eq!(
        cfg.data_dir,
        PathBuf::from("/injected/tmp/magistr/workflow")
    );
}

#[test]
fn persistence_is_on_by_default() {
    let cfg = NativeConfig::from_lookup("/proj", lookup(vec![]));
    assert!(cfg.persist);
    assert_eq!(cfg.project_root, Path::new("/proj"));
    assert!(cfg.broadcast_path.is_none());
}

#[test]
fn explicit_false_or_zero_disables_persistence() {
    for v in ["false", "0", "FALSE", " false "] {
        let cfg = NativeConfig::from_lookup("/p", lookup(vec![("MAGISTR_WORKFLOW_PERSIST", v)]));
        assert!(!cfg.persist, "{v:?} should disable");
    }
    for v in ["true", "1", "yes"] {
        let cfg = NativeConfig::from_lookup("/p", lookup(vec![("MAGISTR_WORKFLOW_PERSIST", v)]));
        assert!(cfg.persist, "{v:?} should keep persistence on");
    }
}

#[test]
fn data_dir_and_broadcast_come_from_env() {
    let cfg = NativeConfig::from_lookup(
        "/p",
        lookup(vec![
            ("MAGISTR_WORKFLOW_DATA_DIR", "/data/tas"),
            ("MAGISTR_WORKFLOW_BROADCAST_PATH", " /tmp/tas.sock "),
        ]),
    );
    assert_eq!(cfg.data_dir, PathBuf::from("/data/tas"));
    assert_eq!(cfg.broadcast_path, Some(PathBuf::from("/tmp/tas.sock")));
}

#[test]
fn blank_data_dir_falls_through_to_the_library_chain() {
    let cfg = NativeConfig::from_lookup(
        "/p",
        lookup(vec![
            ("MAGISTR_WORKFLOW_DATA_DIR", "  "),
            ("XDG_DATA_HOME", "/injected/xdg"),
            ("HOME", "/injected/home"),
        ]),
    );
    assert_eq!(
        cfg.data_dir,
        PathBuf::from("/injected/xdg/magistr/workflow")
    );
}

#[test]
fn data_dir_fallbacks_use_only_the_injected_home_variables() {
    for (vars, expected) in [
        (
            vec![("HOME", "/injected/home"), ("APPDATA", "/injected/appdata")],
            "/injected/home/.local/share/magistr/workflow",
        ),
        (
            vec![("HOME", " "), ("APPDATA", "/injected/appdata")],
            "/injected/appdata/magistr/workflow",
        ),
        (
            vec![("USERPROFILE", "/injected/profile")],
            "/injected/profile/AppData/Roaming/magistr/workflow",
        ),
    ] {
        let cfg = NativeConfig::from_lookup("/p", lookup(vars));
        assert_eq!(cfg.data_dir, PathBuf::from(expected));
    }
}

#[test]
fn persistence_config_mirrors_the_fields() {
    let cfg = NativeConfig::from_lookup("/p", lookup(vec![]))
        .with_data_dir("/x")
        .persist(false);
    let pc = cfg.persistence_config();
    assert!(!pc.enabled);
    assert_eq!(pc.data_dir, PathBuf::from("/x"));
}

#[test]
fn project_socket_default_fills_only_when_env_named_none() {
    let cfg = NativeConfig::from_lookup("/p", |_| None).with_project_socket_default();
    assert_eq!(
        cfg.broadcast_path,
        Some(PathBuf::from("/p/.magistr/workflow.sock"))
    );
    assert_eq!(
        cfg.project_socket_path(),
        PathBuf::from("/p/.magistr/workflow.sock")
    );

    let explicit = NativeConfig::from_lookup(
        "/p",
        lookup(vec![("MAGISTR_WORKFLOW_BROADCAST_PATH", "/tmp/tas.sock")]),
    )
    .with_project_socket_default();
    assert_eq!(
        explicit.broadcast_path,
        Some(PathBuf::from("/tmp/tas.sock"))
    );
}

// ── socket path rule: local when it fits, short per-project otherwise ──

fn long_root(tmp: &tempfile::TempDir) -> PathBuf {
    // Deep enough that `<root>/.magistr/workflow.sock` exceeds every
    // kernel's sun_path (macOS 104, Linux 108 bytes).
    let mut root = tmp.path().to_path_buf();
    for i in 0..6 {
        root.push(format!("a-rather-long-directory-name-{i}"));
    }
    std::fs::create_dir_all(&root).expect("mkdir long root");
    assert!(root.as_os_str().len() > 110, "{}", root.display());
    root
}

#[test]
fn short_root_keeps_the_project_local_socket() {
    let cfg = NativeConfig::from_lookup("/p", |_| None);
    assert_eq!(
        cfg.project_socket_path(),
        PathBuf::from("/p/.magistr/workflow.sock")
    );
    assert_eq!(cfg.feed_socket_path(), cfg.project_socket_path());
}

#[test]
fn long_root_falls_back_to_a_short_socket_under_the_runtime_dir() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let root = long_root(&tmp);
    let cfg = NativeConfig::from_lookup(&root, lookup(vec![("XDG_RUNTIME_DIR", "/run/user/1000")]));

    let path = cfg.project_socket_path();
    assert!(
        path.starts_with("/run/user/1000"),
        "should move under the runtime dir: {}",
        path.display()
    );
    let name = path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or_default();
    assert!(
        name.starts_with("magistr-") && name.ends_with(".sock"),
        "per-project name: {name}"
    );
    assert!(
        path.as_os_str().len() <= 103,
        "must fit sun_path on macOS: {} bytes",
        path.as_os_str().len()
    );
    // Stable: a second resolution (another process) lands on the same file.
    let again =
        NativeConfig::from_lookup(&root, lookup(vec![("XDG_RUNTIME_DIR", "/run/user/1000")]));
    assert_eq!(again.project_socket_path(), path);
    // And the default is what with_project_socket_default installs.
    assert_eq!(
        cfg.clone().with_project_socket_default().broadcast_path,
        Some(path)
    );
}

#[test]
fn runtime_dir_prefers_xdg_then_tmpdir_then_tmp() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let root = long_root(&tmp);
    let xdg = NativeConfig::from_lookup(
        &root,
        lookup(vec![
            ("XDG_RUNTIME_DIR", "/run/user/7"),
            ("TMPDIR", "/var/tmpx"),
        ]),
    );
    assert!(xdg.project_socket_path().starts_with("/run/user/7"));
    let tmpdir = NativeConfig::from_lookup(&root, lookup(vec![("TMPDIR", "/var/tmpx/")]));
    assert!(tmpdir.project_socket_path().starts_with("/var/tmpx"));
    let plain = NativeConfig::from_lookup(&root, |_| None);
    assert!(plain.project_socket_path().starts_with("/tmp"));
}

#[test]
fn explicit_broadcast_path_wins_for_the_feed() {
    let cfg = NativeConfig::from_lookup(
        "/p",
        lookup(vec![("MAGISTR_WORKFLOW_BROADCAST_PATH", "/tmp/tas.sock")]),
    );
    assert_eq!(cfg.feed_socket_path(), PathBuf::from("/tmp/tas.sock"));
    // The project default is unchanged by the override.
    assert_eq!(
        cfg.project_socket_path(),
        PathBuf::from("/p/.magistr/workflow.sock")
    );
}

#[cfg(unix)]
#[test]
fn the_derived_socket_binds_for_a_very_long_project_root() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let root = long_root(&tmp);
    // Own the runtime directory as well as the project root: a developer's
    // XDG_RUNTIME_DIR may be absent, inaccessible, or meant for another host.
    let runtime = tempfile::Builder::new()
        .prefix("tas-")
        .tempdir_in("/tmp")
        .expect("short runtime directory");
    let cfg = NativeConfig::from_lookup(&root, |key| {
        (key == "XDG_RUNTIME_DIR").then(|| runtime.path().to_string_lossy().into_owned())
    });
    let path = cfg.project_socket_path();
    assert!(path.starts_with(runtime.path()), "{}", path.display());
    let listener = std::os::unix::net::UnixListener::bind(&path)
        .unwrap_or_else(|e| panic!("bind {}: {e}", path.display()));
    drop(listener);
    std::fs::remove_file(&path).expect("remove the owned socket");
}
#[test]
fn captured_settings_do_not_consult_the_lookup_again() {
    use std::cell::RefCell;
    use std::collections::BTreeMap;
    let vars = RefCell::new(BTreeMap::from([
        ("MAGISTR_WORKFLOW_ENABLE_SESSIONS", "true"),
        ("MAGISTR_WORKFLOW_STRICT_MODE", "true"),
        ("MAGISTR_WORKFLOW_RECENT_STEPS_LIMIT", "17"),
        ("TMPDIR", "/captured/tmp"),
    ]));
    let reads = RefCell::new(BTreeMap::<String, usize>::new());
    let cfg = NativeConfig::from_lookup("/project", |key| {
        *reads.borrow_mut().entry(key.to_string()).or_default() += 1;
        vars.borrow().get(key).map(|v| (*v).to_owned())
    });
    let count = reads.borrow().clone();
    vars.borrow_mut().clear();
    for project in ["first-project", "second-project"] {
        let think = cfg.think_config(project);
        assert!(think.validation.strict_mode);
        assert_eq!(think.system.recent_steps_limit, 17);
        assert_eq!(think.features.default_session_id.as_deref(), Some(project));
        assert!(think.persistence.enabled);
        assert_eq!(
            think.persistence.data_dir,
            PathBuf::from("/captured/tmp/magistr/workflow")
        );
        assert!(think.broadcast.path.is_none());
    }
    assert_eq!(*reads.borrow(), count);
    assert!(count.values().all(|&calls| calls == 1));
    assert!(!count.contains_key("API_KEY"));
}

#[test]
fn effective_sources_track_native_precedence_and_builder_overrides() {
    let cfg = NativeConfig::from_lookup(
        "/project",
        lookup(vec![
            ("MAGISTR_WORKFLOW_DATA_DIR", "/explicit/store"),
            ("XDG_DATA_HOME", "/ignored"),
            ("MAGISTR_WORKFLOW_PERSIST", " false "),
            ("MAGISTR_WORKFLOW_STRICT_MODE", "true"),
            ("MAGISTR_WORKFLOW_BROADCAST_PATH", "/secret-looking/socket"),
            ("MAGISTR_WORKFLOW_DEFAULT_SESSION_ID", "sensitive/session"),
        ]),
    );
    let sources = cfg.configuration_sources();
    assert_eq!(
        sources["persistence.data_dir"],
        ConfigSource::Environment("MAGISTR_WORKFLOW_DATA_DIR")
    );
    assert_eq!(
        sources["persistence.enabled"],
        ConfigSource::Environment("MAGISTR_WORKFLOW_PERSIST")
    );
    assert_eq!(
        sources["validation.strict_mode"],
        ConfigSource::Environment("MAGISTR_WORKFLOW_STRICT_MODE")
    );
    assert_eq!(
        sources["features.default_session_id"],
        ConfigSource::Default
    );
    let diagnostic = format!("{sources:?}");
    assert!(!diagnostic.contains("/secret-looking/socket"));
    assert!(!diagnostic.contains("sensitive/session"));
    let updated = cfg
        .with_data_dir("/builder/store")
        .persist(true)
        .with_broadcast_path(None)
        .with_runtime_dir("/builder/runtime");
    for field in [
        "persistence.data_dir",
        "persistence.enabled",
        "broadcast.path",
        "runtime_dir",
    ] {
        assert_eq!(
            updated.configuration_sources()[field],
            ConfigSource::Explicit
        );
    }
    let think = updated.think_config("project");
    assert!(think.persistence.enabled);
    assert_eq!(think.persistence.data_dir, PathBuf::from("/builder/store"));
    assert!(think.broadcast.path.is_none());
}

#[test]
fn explicit_session_is_frozen_while_automatic_sessions_follow_admission() {
    let cfg = NativeConfig::from_lookup(
        "/project",
        lookup(vec![
            ("MAGISTR_WORKFLOW_AUTO_SESSION", "true"),
            ("MAGISTR_WORKFLOW_DEFAULT_SESSION_ID", "chosen-session"),
        ]),
    );
    assert_eq!(
        cfg.think_config("admitted-project")
            .features
            .default_session_id
            .as_deref(),
        Some("chosen-session")
    );
    let automatic = NativeConfig::from_lookup(
        "/project",
        lookup(vec![("MAGISTR_WORKFLOW_ENABLE_SESSIONS", "true")]),
    );
    assert_eq!(
        automatic.configuration_sources()["features.default_session_id"],
        ConfigSource::ProjectIdentity
    );
    assert_eq!(
        automatic
            .think_config("admitted-project")
            .features
            .default_session_id
            .as_deref(),
        Some("admitted-project")
    );
}

#[test]
fn injected_empty_lookup_has_a_deterministic_fallback_and_no_ambient_think_settings() {
    let cfg = NativeConfig::from_lookup("/project", |_| None);
    assert_eq!(cfg.data_dir, PathBuf::from("/tmp/magistr/workflow"));
    let think = cfg.think_config("project");
    assert!(!think.validation.strict_mode);
    assert!(!think.features.enable_sessions);
    assert_eq!(think.system.recent_steps_limit, 3);
    assert_eq!(
        cfg.configuration_sources()["persistence.data_dir"],
        ConfigSource::Default
    );
}

#[test]
fn configuration_equality_includes_captured_behavior() {
    let strict = NativeConfig::from_lookup(
        "/project",
        lookup(vec![("MAGISTR_WORKFLOW_STRICT_MODE", "true")]),
    );
    let defaults = NativeConfig::from_lookup("/project", |_| None);
    assert_ne!(strict, defaults);
}
