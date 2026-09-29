use super::*;
use crate::agent::AuthMethod;
use crate::config::inputs::ProjectInputs;
use crate::config::session_selection::SessionSelection;

fn runtime(agent: &str) -> AgentRuntime {
    AgentRuntime {
        path: "never-launched".into(),
        args: vec![],
        auth: AuthMethod::Implicit,
        provider: agent.into(),
    }
}

#[test]
fn flags_project_personal_and_agent_defaults_have_one_precedence() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::create_dir(dir.path().join(".magistr")).unwrap();
    std::fs::write(
        dir.path().join(".magistr/agents.toml"),
        "agent = 'codex'\n[models]\ncodex = 'project-model'\n[efforts]\ncodex = 'medium'",
    )
    .unwrap();
    let mut personal = UserConfig::default();
    personal
        .models
        .set_for_provider("codex", Some("personal-model".into()));
    personal.models.claude = Some("claude-only".into());
    personal.efforts.insert("codex".into(), "low".into());
    let mut config = AgentConfig::new("", dir.path());
    let sources = session_defaults(&mut config, &personal, &runtime("codex")).unwrap();
    assert_eq!(config.model.as_deref(), Some("project-model"));
    assert_eq!(config.effort.as_deref(), Some("medium"));
    assert_eq!(sources["model"], SessionConfigSource::ProjectFile);
    let mut explicit = AgentConfig::new("", dir.path())
        .model("flag-model")
        .effort("high");
    session_defaults(&mut explicit, &personal, &runtime("codex")).unwrap();
    assert_eq!(explicit.model.as_deref(), Some("flag-model"));
    assert_eq!(explicit.effort.as_deref(), Some("high"));
    let mut other = AgentConfig::new("", dir.path());
    session_defaults(&mut other, &personal, &runtime("copilot")).unwrap();
    assert!(other.model.is_none());
    assert!(other.effort.is_none());
}

#[test]
fn frozen_selection_survives_changed_defaults_and_preserves_unset_values() {
    let dir = tempfile::tempdir().unwrap();
    let saved = SessionSelection {
        agent: "codex".into(),
        model: Some("saved-model".into()),
        effort: None,
        sources: BTreeMap::new(),
    };
    let inputs = ProjectInputs::from_lookup(dir.path(), |_| None).with_session_selection(saved);
    let mut personal = UserConfig::default();
    personal
        .models
        .set_for_provider("codex", Some("changed".into()));
    personal.efforts.insert("codex".into(), "high".into());
    let mut config = AgentConfig::new("", dir.path()).with_inputs(&inputs);
    session_defaults(&mut config, &personal, &runtime("codex")).unwrap();
    assert_eq!(config.model.as_deref(), Some("saved-model"));
    assert!(config.effort.is_none());
}

#[test]
fn resume_preserves_runtime_settings_and_default_can_override_saved_defaults() {
    let dir = tempfile::tempdir().unwrap();
    let mut personal = UserConfig::default();
    personal
        .models
        .set_for_provider("codex", Some("personal".into()));
    personal.efforts.insert("codex".into(), "high".into());
    let mut config = AgentConfig::new("", dir.path()).resume("thread");
    let sources = session_defaults(&mut config, &personal, &runtime("codex")).unwrap();
    assert!(config.model.is_none());
    assert!(config.effort.is_none());
    assert_eq!(sources["model"], SessionConfigSource::RuntimeResume);
    let mut config = AgentConfig::new("", dir.path())
        .model("default")
        .effort("default");
    session_defaults(&mut config, &personal, &runtime("codex")).unwrap();
    assert!(config.model.is_none());
    assert!(config.effort.is_none());
}

#[test]
fn switching_agents_discards_the_previous_agents_frozen_preferences() {
    let dir = tempfile::tempdir().unwrap();
    let snapshot = SessionSelection {
        agent: "codex".into(),
        model: Some("codex-model".into()),
        effort: Some("high".into()),
        sources: Default::default(),
    };
    let inputs = ProjectInputs::from_lookup(dir.path(), |_| None).with_session_selection(snapshot);
    let mut personal = UserConfig::default();
    personal.models.claude = Some("claude-model".into());
    let mut config = AgentConfig::new("", dir.path())
        .with_inputs(&inputs)
        .provider("claude");
    session_defaults(&mut config, &personal, &runtime("claude")).unwrap();
    assert_eq!(config.model.as_deref(), Some("claude-model"));
    assert!(config.effort.is_none());
}

#[test]
fn a_frozen_selection_does_not_read_changed_project_preferences() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::create_dir(dir.path().join(".magistr")).unwrap();
    std::fs::write(dir.path().join(".magistr/agents.toml"), "not valid TOML").unwrap();
    let snapshot = SessionSelection {
        agent: "codex".into(),
        model: Some("saved-model".into()),
        effort: Some("high".into()),
        sources: Default::default(),
    };
    let inputs = ProjectInputs::from_lookup(dir.path(), |_| None).with_session_selection(snapshot);
    let mut config = AgentConfig::new("", dir.path()).with_inputs(&inputs);
    session_defaults(&mut config, &UserConfig::default(), &runtime("codex")).unwrap();
    assert_eq!(config.model.as_deref(), Some("saved-model"));
    assert_eq!(config.effort.as_deref(), Some("high"));
}

#[test]
fn explicit_default_effort_reports_the_command_as_its_source() {
    let dir = tempfile::tempdir().unwrap();
    let mut config = AgentConfig::new("", dir.path()).effort("default");
    let sources = session_defaults(&mut config, &UserConfig::default(), &runtime("codex")).unwrap();
    assert!(config.effort.is_none());
    assert_eq!(sources["effort"], SessionConfigSource::Explicit);
}
