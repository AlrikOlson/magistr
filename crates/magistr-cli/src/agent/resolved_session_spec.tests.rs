use super::*;
use crate::agent::permission_request::permission_request as policy_request;
use crate::agent::{AuthMethod, PermissionMode};
use crate::in_env as env_test;
use magistr_application::sessions::{PolicyDecision, SessionPolicy};
use magistr_engine::policy::GovernancePolicy;

fn runtime(provider: &str) -> AgentRuntime {
    AgentRuntime {
        path: "never-spawned".into(),
        args: Vec::new(),
        auth: AuthMethod::Implicit,
        provider: provider.into(),
    }
}

#[test]
fn snapshots_keep_provider_model_mcp_and_policy_after_configuration_changes() {
    let dir = tempfile::tempdir().expect("project");
    std::fs::create_dir(dir.path().join(".claude")).expect("permissions directory");
    let permission_path = dir.path().join(".claude/permissions.toml");
    std::fs::write(&permission_path, "profile = 'yolo'\n").expect("initial policy");
    let mcp_path = dir.path().join(".mcp.json");
    std::fs::write(
        &mcp_path,
        r#"{"mcpServers":{"original":{"command":"original-server"}}}"#,
    )
    .expect("MCP");
    let mut defaults = UserConfig::default();
    defaults.session.default_provider = Some("claude".into());
    defaults.models.claude = Some("first-model".into());
    let spec = ResolvedSessionSpec::resolve_with_runtime(
        AgentConfig::new("original prompt", dir.path()),
        &defaults,
        runtime("claude"),
    )
    .expect("resolve");
    defaults.session.default_provider = Some("copilot".into());
    defaults.models.claude = Some("changed-model".into());
    std::fs::write(&permission_path, "profile = 'cautious'\n").expect("changed policy");
    std::fs::write(
        &mcp_path,
        r#"{"mcpServers":{"changed":{"command":"changed-server"}}}"#,
    )
    .expect("changed MCP");
    assert_eq!(spec.runtime().provider_id(), "claude");
    assert_eq!(spec.model(), Some("first-model"));
    assert_eq!(spec.prompt(), "original prompt");
    assert_eq!(
        spec.cwd(),
        dir.path().canonicalize().expect("canonical root")
    );
    assert_eq!(spec.permission_profile(), PermissionProfile::Yolo);
    assert_eq!(spec.provenance()["model"], SessionConfigSource::UserDefault);
    assert!(
        matches!(&spec.mcp_servers()[0], McpServer::Stdio(server) if server.name == "original")
    );
    let (mut policy, handling) = spec.into_permission_policy();
    assert_eq!(handling, NoninteractivePolicy::Refuse);
    let (response_tx, _) = tokio::sync::oneshot::channel();
    let request = magistr_acp::AcpPermissionRequest {
        tool_call_id: "write".into(),
        tool_title: "Write".into(),
        tool_input: serde_json::json!({"file_path": dir.path().join("ordinary.txt")}),
        tool_kind: None,
        options: Vec::new(),
        response_tx,
    };
    assert_eq!(
        policy.decide(&policy_request(&request)),
        PolicyDecision::Allow,
        "running policy retains its resolved profile"
    );
    let next = ResolvedSessionSpec::resolve_with_runtime(
        AgentConfig::new("next", dir.path()),
        &defaults,
        runtime("copilot"),
    )
    .expect("next session");
    assert_eq!(next.permission_profile(), PermissionProfile::Cautious);
    assert!(
        next.model().is_none(),
        "Claude model defaults never cross providers"
    );
}

#[test]
fn inherited_models_pass_through_project_governance_before_launch() {
    let dir = tempfile::tempdir().expect("project");
    let identity = crate::team::Identity::resolve(dir.path());
    let mut team = crate::team::TeamConfig::new_with_owner(&identity);
    team.policy = GovernancePolicy {
        approved_models: vec!["approved".into()],
        ..GovernancePolicy::default()
    };
    team.save(dir.path()).expect("governance");
    let mut defaults = UserConfig::default();
    defaults.models.claude = Some("unapproved-default".into());
    let result = ResolvedSessionSpec::resolve_with_runtime(
        AgentConfig::new("hello", dir.path()),
        &defaults,
        runtime("claude"),
    );
    assert!(
        result.is_err(),
        "defaulted models must be governed like explicit models"
    );
    assert!(
        result
            .err()
            .expect("refusal")
            .to_string()
            .contains("unapproved-default")
    );
}

#[test]
fn explicit_choices_win_and_resume_keeps_runtime_model_without_an_override() {
    let dir = tempfile::tempdir().expect("project");
    let mut defaults = UserConfig::default();
    defaults.models.claude = Some("default-model".into());
    let explicit = ResolvedSessionSpec::resolve_with_runtime(
        AgentConfig::new("hello", dir.path())
            .provider("claude")
            .model("explicit")
            .effort("high"),
        &defaults,
        runtime("claude"),
    )
    .expect("explicit spec");
    assert_eq!(explicit.model(), Some("explicit"));
    assert_eq!(explicit.effort(), Some("high"));
    assert_eq!(
        explicit.provenance()["model"],
        SessionConfigSource::Explicit
    );
    let resumed = ResolvedSessionSpec::resolve_with_runtime(
        AgentConfig::new("continue", dir.path())
            .resume("retained-history")
            .continue_conversation(true),
        &defaults,
        runtime("claude"),
    )
    .expect("resume");
    assert!(resumed.model().is_none());
    assert_eq!(
        resumed.provenance()["model"],
        SessionConfigSource::RuntimeResume
    );
    assert!(
        matches!(resumed.session_mode(), AcpSessionMode::Load { session_id } if session_id == "retained-history")
    );
}

#[test]
fn malformed_or_unsupported_mcp_configuration_is_not_silently_discarded() {
    let dir = tempfile::tempdir().expect("project");
    for invalid in [
        "{broken json",
        r#"{"mcpServers":{"remote":{"type":"http","url":"https://example.test/mcp"}}}"#,
        r#"{"mcpServers":{"invalid":{"command":"server","args":[3]}}}"#,
    ] {
        std::fs::write(dir.path().join(".mcp.json"), invalid).expect("configuration");
        let result = ResolvedSessionSpec::resolve_with_runtime(
            AgentConfig::new("hello", dir.path()),
            &UserConfig::default(),
            runtime("claude"),
        );
        assert!(
            result.is_err(),
            "invalid MCP configuration must refuse startup: {invalid}"
        );
        assert!(
            result
                .err()
                .expect("refused MCP")
                .to_string()
                .contains("MCP configuration")
        );
    }
}

#[test]
fn unsupported_provider_and_continuation_are_refused_before_runtime_discovery() {
    let dir = tempfile::tempdir().expect("project");
    let result = ResolvedSessionSpec::resolve_with_defaults(
        AgentConfig::new("hello", dir.path()).provider("unknown-provider"),
        &UserConfig::default(),
    );
    assert!(
        result
            .err()
            .expect("unknown provider")
            .to_string()
            .contains("Unsupported agent provider")
    );
    let result = ResolvedSessionSpec::resolve_with_defaults(
        AgentConfig::new("hello", dir.path())
            .provider("unknown-provider")
            .continue_conversation(true),
        &UserConfig::default(),
    );
    assert!(
        result
            .err()
            .expect("unknown continuation")
            .to_string()
            .contains("--resume")
    );
}

#[test]
fn unknown_runtime_and_resumed_models_cannot_bypass_project_model_governance() {
    let dir = tempfile::tempdir().expect("project");
    let identity = crate::team::Identity::resolve(dir.path());
    let mut team = crate::team::TeamConfig::new_with_owner(&identity);
    team.policy = GovernancePolicy {
        approved_models: vec!["approved".into()],
        ..GovernancePolicy::default()
    };
    team.save(dir.path()).expect("governance");
    for config in [
        AgentConfig::new("hello", dir.path()),
        AgentConfig::new("hello", dir.path()).resume("retained-history"),
        AgentConfig::new("hello", dir.path()).model("default"),
    ] {
        let result = ResolvedSessionSpec::resolve_with_runtime(
            config,
            &UserConfig::default(),
            runtime("claude"),
        );
        assert!(
            result
                .err()
                .expect("unverifiable model refused")
                .to_string()
                .contains("explicit approved model")
        );
    }
}

#[test]
fn relative_root_is_frozen_as_an_absolute_canonical_project() {
    let dir = tempfile::tempdir_in(".").expect("relative project");
    let spec = ResolvedSessionSpec::resolve_with_runtime(
        AgentConfig::new("hello", dir.path()),
        &UserConfig::default(),
        runtime("claude"),
    )
    .expect("resolve relative root");
    assert!(
        spec.cwd().is_absolute(),
        "startup never depends on a later process working directory"
    );
    assert_eq!(
        spec.cwd(),
        dir.path().canonicalize().expect("canonical project")
    );
}

#[test]
fn malformed_user_defaults_are_refused_instead_of_selecting_another_provider() {
    let dir = tempfile::tempdir().expect("config root");
    std::fs::create_dir(dir.path().join("magistr")).expect("config directory");
    std::fs::write(
        dir.path().join("magistr/config.toml"),
        "[session\nbroken = true\n",
    )
    .expect("malformed config");
    if !env_test::in_env(&[("XDG_CONFIG_HOME", Some(dir.path().as_os_str()))]) {
        return;
    }
    let result = AgentConfig::new("hello", dir.path())
        .provider("openai")
        .resolve();
    assert!(
        result
            .err()
            .expect("invalid defaults")
            .to_string()
            .contains("Cannot resolve session user defaults")
    );
}

#[test]
fn malformed_project_governance_and_charter_refuse_session_resolution() {
    for file in ["team.json", "charter.json"] {
        let dir = tempfile::tempdir().expect("project");
        std::fs::create_dir(dir.path().join(".magistr")).expect("project configuration directory");
        std::fs::write(dir.path().join(".magistr").join(file), "{broken json")
            .expect("malformed policy");
        let result = ResolvedSessionSpec::resolve_with_runtime(
            AgentConfig::new("hello", dir.path()).permission_profile(PermissionProfile::Yolo),
            &UserConfig::default(),
            runtime("claude"),
        );
        assert!(
            result.is_err(),
            "invalid {file} must not discard existing restrictions"
        );
        assert!(
            result
                .err()
                .expect("refused policy")
                .to_string()
                .contains(file)
        );
    }
}

#[test]
fn legacy_provider_options_are_refused_before_runtime_start() {
    let dir = tempfile::tempdir().expect("project");

    let zero_turns = match ResolvedSessionSpec::resolve_with_runtime(
        AgentConfig::new("hello", dir.path())
            .provider("claude")
            .max_turns(0),
        &UserConfig::default(),
        runtime("claude"),
    ) {
        Ok(_) => panic!("zero-turn sessions cannot start"),
        Err(error) => error,
    };
    assert!(zero_turns.to_string().contains("max_turns"));

    let legacy_permissions = match ResolvedSessionSpec::resolve_with_runtime(
        AgentConfig::new("hello", dir.path())
            .provider("claude")
            .permission_mode(PermissionMode::AcceptEdits),
        &UserConfig::default(),
        runtime("claude"),
    ) {
        Ok(_) => panic!("legacy permission mode cannot override host policy"),
        Err(error) => error,
    };
    assert!(legacy_permissions.to_string().contains("permission_mode"));
}
