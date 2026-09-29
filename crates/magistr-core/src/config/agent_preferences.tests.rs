use super::*;

#[test]
fn missing_settings_use_defaults_but_invalid_settings_refuse() {
    let dir = tempfile::tempdir().unwrap();
    assert_eq!(
        AgentPreferences::load(dir.path()).unwrap(),
        AgentPreferences::default()
    );
    std::fs::create_dir(dir.path().join(".magistr")).unwrap();
    for text in ["agent = 42", "agent = ' '", "agnt = 'codex'"] {
        std::fs::write(dir.path().join(".magistr/agents.toml"), text).unwrap();
        assert!(AgentPreferences::load(dir.path()).is_err(), "{text}");
    }
}

#[test]
fn project_models_and_efforts_remain_scoped_to_the_agent() {
    let prefs: AgentPreferences = toml::from_str("agent = 'codex'\n[models]\ncodex = 'coding-model'\nclaude = 'other-model'\n[efforts]\ncodex = 'high'").unwrap();
    assert_eq!(prefs.models["codex"], "coding-model");
    assert_eq!(prefs.models["claude"], "other-model");
    assert_eq!(prefs.efforts["codex"], "high");
}
