use super::*;

#[tokio::test]
async fn personal_agent_writer_invalidates_desktop_revision_and_reload_preserves_both() {
    use crate::application::{DesktopPreferences, DesktopPreferencesService};
    use crate::config::user_config::ThemePreference;
    let dir = tempfile::tempdir().unwrap();
    let inputs = ProjectInputs::from_lookup(dir.path(), |_| None)
        .with_config_dir(dir.path().join("personal"));
    let service = DesktopPreferencesService::new(inputs.clone());
    let displayed = service.load().await.unwrap();
    config_agent_set("model", "chosen", false, Some("codex"), dir.path(), &inputs).unwrap();
    let draft = DesktopPreferences {
        theme: ThemePreference::Dark,
        reduce_motion: true,
    };
    assert!(service.save(displayed.revision, draft).await.is_err());
    let refreshed = service.load().await.unwrap();
    service.save(refreshed.revision, draft).await.unwrap();
    let config = inputs.user_config().unwrap().value;
    assert_eq!(config.models.for_provider("codex"), Some("chosen"));
    assert_eq!(config.theme.default, ThemePreference::Dark);
    assert_eq!(config.accessibility.reduce_motion, Some(true));
}

#[test]
fn personal_agent_writer_respects_the_shared_preferences_lock() {
    let dir = tempfile::tempdir().unwrap();
    let inputs = ProjectInputs::from_lookup(dir.path(), |_| None).with_config_dir(dir.path());
    let held = std::fs::File::options()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(dir.path().join("config.toml.lock"))
        .unwrap();
    held.try_lock().unwrap();
    assert!(config_agent_set("agent", "codex", false, None, dir.path(), &inputs).is_err());
    assert!(!inputs.user_config_path().unwrap().exists());
    held.unlock().unwrap();
    drop(held);
    config_agent_set("agent", "codex", false, None, dir.path(), &inputs).unwrap();
}

#[test]
fn personal_settings_keep_comments_other_preferences_and_per_agent_models() {
    let dir = tempfile::tempdir().unwrap();
    let inputs = ProjectInputs::from_lookup(dir.path(), |_| None)
        .with_config_dir(dir.path().join("personal"));
    let path = inputs.config_dir().unwrap().join("config.toml");
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(
        &path,
        "# Keep this comment\n[theme]\ndefault = 'dark'\n[models]\nclaude = 'claude-model'\n",
    )
    .unwrap();
    config_agent_set("agent", "codex", false, None, dir.path(), &inputs).unwrap();
    config_agent_set("model", "codex-model", false, None, dir.path(), &inputs).unwrap();
    config_agent_set("effort", "high", false, None, dir.path(), &inputs).unwrap();
    let text = std::fs::read_to_string(&path).unwrap();
    assert!(text.contains("# Keep this comment"));
    let user = inputs.user_config().unwrap().value;
    assert_eq!(
        user.theme.default,
        crate::config::user_config::ThemePreference::Dark
    );
    assert_eq!(user.models.for_provider("claude"), Some("claude-model"));
    assert_eq!(user.models.for_provider("codex"), Some("codex-model"));
    assert_eq!(user.efforts["codex"], "high");
}

#[test]
fn project_defaults_and_explicit_agent_are_saved_without_changing_personal_settings() {
    let dir = tempfile::tempdir().unwrap();
    let inputs = ProjectInputs::from_lookup(dir.path(), |_| None)
        .with_config_dir(dir.path().join("personal"));
    config_agent_set("agent", "codex", true, None, dir.path(), &inputs).unwrap();
    config_agent_set("model", "project-model", true, None, dir.path(), &inputs).unwrap();
    config_agent_set(
        "model",
        "claude-model",
        true,
        Some("claude"),
        dir.path(),
        &inputs,
    )
    .unwrap();
    config_agent_set("effort", "default", true, None, dir.path(), &inputs).unwrap();
    let project = AgentPreferences::load(dir.path()).unwrap();
    assert_eq!(project.agent.as_deref(), Some("codex"));
    assert_eq!(project.models["codex"], "project-model");
    assert_eq!(project.models["claude"], "claude-model");
    assert_eq!(project.efforts["codex"], "default");
    assert!(!inputs.config_dir().unwrap().join("config.toml").exists());
}

#[test]
fn invalid_setting_does_not_write_a_file() {
    let dir = tempfile::tempdir().unwrap();
    let inputs = ProjectInputs::from_lookup(dir.path(), |_| None)
        .with_config_dir(dir.path().join("personal"));
    for (key, setting, agent) in [
        ("model", "chosen", None),
        ("agent", "unknown", None),
        ("effort", " ", Some("codex")),
        ("unknown", "value", None),
    ] {
        assert!(config_agent_set(key, setting, false, agent, dir.path(), &inputs).is_err());
    }
    assert!(!inputs.config_dir().unwrap().join("config.toml").exists());
}

#[test]
fn malformed_existing_settings_are_refused_without_overwriting_them() {
    let dir = tempfile::tempdir().unwrap();
    let inputs = ProjectInputs::from_lookup(dir.path(), |_| None).with_config_dir(dir.path());
    let path = dir.path().join("config.toml");
    let text = "models = 'wrong type'\n";
    std::fs::write(&path, text).unwrap();
    assert!(
        config_agent_set("model", "chosen", false, Some("codex"), dir.path(), &inputs).is_err()
    );
    assert_eq!(std::fs::read_to_string(path).unwrap(), text);
}
