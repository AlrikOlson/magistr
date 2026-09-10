use crate::in_env as env_test;

use super::*;

#[test]
fn unsupported_continue_is_refused_before_runtime_startup() {
    let dir = tempfile::tempdir().expect("project");
    let result = AgentConfig::new("continue", dir.path())
        .provider("openai")
        .continue_conversation(true)
        .resolve();
    assert!(
        result.is_err(),
        "an unimplemented continuation cannot start a new session"
    );
    let error = result.err().expect("refused continuation").to_string();
    assert!(error.contains("resume"), "{error}");
}

#[test]
fn explicit_auto_bypasses_a_global_api_provider() {
    let path = tempfile::tempdir().expect("empty PATH directory");
    if !env_test::in_env(&[("PATH", Some(path.path().as_os_str()))]) {
        return;
    }
    let mut defaults = provider_defaults();
    defaults.session.default_provider = Some("openai".into());
    let config = AgentConfig::new("first", "/tmp")
        .provider("auto")
        .apply_defaults_from(&defaults);
    assert_eq!(config.provider.as_deref(), Some("auto"));
    assert!(
        config.model.is_none(),
        "auto must not inherit an API provider's model"
    );
}

#[test]
fn invalid_global_provider_is_preserved_for_launch_refusal() {
    let mut defaults = provider_defaults();
    defaults.session.default_provider = Some("cluade".into());
    let config = AgentConfig::new("first", "/tmp").apply_defaults_from(&defaults);
    assert_eq!(config.provider.as_deref(), Some("cluade"));
    assert!(config.resolve_runtime().is_err());
    assert!(config.model.is_none());
}

fn provider_defaults() -> crate::config::user_config::UserConfig {
    let mut defaults = crate::config::user_config::UserConfig::default();
    defaults.session.default_provider = Some("claude".into());
    defaults.models.claude = Some("claude-specific".into());
    defaults.models.copilot = Some("copilot-specific".into());
    defaults
}

#[test]
fn session_providers_resolve_independently_of_global_defaults() {
    let defaults = provider_defaults();
    let first = AgentConfig::new("first", "/tmp/project")
        .provider("openai")
        .model("explicit")
        .resume("runtime-owned-history")
        .apply_defaults_from(&defaults);
    let second = AgentConfig::new("second", "/tmp/project")
        .provider("gemini")
        .apply_defaults_from(&defaults);
    assert_eq!(first.resolve_runtime().unwrap().provider_id(), "openai");
    assert_eq!(second.resolve_runtime().unwrap().provider_id(), "gemini");
    assert_eq!(first.model.as_deref(), Some("explicit"));
    assert_eq!(first.resume.as_deref(), Some("runtime-owned-history"));
    assert!(
        second.model.is_none(),
        "Claude defaults must not cross providers"
    );
    assert_eq!(defaults.session.default_provider.as_deref(), Some("claude"));
}

#[test]
fn model_defaults_follow_the_session_provider_and_remain_pinned() {
    let mut defaults = provider_defaults();
    let inherited = AgentConfig::new("first", "/tmp").apply_defaults_from(&defaults);
    assert_eq!(inherited.provider.as_deref(), Some("claude"));
    assert_eq!(inherited.model.as_deref(), Some("claude-specific"));

    let explicit = AgentConfig::new("second", "/tmp")
        .provider("copilot")
        .apply_defaults_from(&defaults);
    assert_eq!(explicit.provider.as_deref(), Some("copilot"));
    assert_eq!(explicit.model.as_deref(), Some("copilot-specific"));

    defaults.session.default_provider = Some("openai".into());
    defaults.models.claude = Some("changed-model".into());
    let reapplied = inherited.apply_defaults_from(&defaults);
    assert_eq!(reapplied.provider.as_deref(), Some("claude"));
    assert_eq!(reapplied.model.as_deref(), Some("claude-specific"));
}

#[test]
fn resuming_without_a_model_override_preserves_the_runtime_selection() {
    let defaults = provider_defaults();
    let config = AgentConfig::new("continue", "/tmp/project")
        .resume("runtime-thread")
        .apply_defaults_from(&defaults);
    assert_eq!(config.provider.as_deref(), Some("claude"));
    assert_eq!(config.resume.as_deref(), Some("runtime-thread"));
    assert!(
        config.model.is_none(),
        "the global default must not replace the restored model"
    );
}

#[test]
fn resuming_keeps_an_explicit_model_override() {
    let config = AgentConfig::new("continue", "/tmp/project")
        .resume("runtime-thread")
        .model("explicit-resume-model")
        .apply_defaults_from(&provider_defaults());
    assert_eq!(config.model.as_deref(), Some("explicit-resume-model"));
}

#[test]
fn codex_session_provider_survives_defaults_and_uses_its_own_adapter() {
    let config = AgentConfig::new("first", "/tmp")
        .provider("codex")
        .apply_defaults_from(&provider_defaults());
    assert_eq!(config.provider.as_deref(), Some("codex"));
    assert!(config.model.is_none());
    assert_eq!(
        config
            .resolve_runtime()
            .expect("Codex adapter")
            .provider_id(),
        "codex"
    );
}

#[test]
fn agent_config_defaults() {
    let config = AgentConfig::new("hello", "/tmp/project");
    assert_eq!(config.prompt, "hello");
    assert_eq!(config.cwd, PathBuf::from("/tmp/project"));
    assert!(config.system_prompt.is_none());
    assert!(config.max_turns.is_none());
    // Default is AcceptEdits so CLI hooks (quality gates, stop hooks) fire correctly.
    // BypassPermissions / --dangerously-skip-permissions skips the hook pipeline.
    assert_eq!(config.permission_mode, PermissionMode::AcceptEdits);
    assert!(!config.continue_conversation);
    assert!(config.resume.is_none());
}

#[test]
fn agent_config_builder() {
    let config = AgentConfig::new("fix the bug", "/tmp/project")
        .system_prompt("You are a coding assistant")
        .max_turns(10)
        .permission_mode(PermissionMode::AcceptEdits);

    assert_eq!(config.prompt, "fix the bug");
    assert_eq!(
        config.system_prompt.as_deref(),
        Some("You are a coding assistant")
    );
    assert_eq!(config.max_turns, Some(10));
    assert_eq!(config.permission_mode, PermissionMode::AcceptEdits);
}

#[test]
fn agent_config_model_selection() {
    let config = AgentConfig::new("fix it", "/tmp/project").model("sonnet");
    assert_eq!(config.model.as_deref(), Some("sonnet"));
}

#[test]
fn agent_config_model_defaults_to_none() {
    let config = AgentConfig::new("test", "/tmp/project");
    assert!(config.model.is_none());
}

#[test]
fn apply_defaults_preserves_explicit_model() {
    // Explicit .model() call must never be overridden by user defaults
    let config = AgentConfig::new("test", "/tmp/project")
        .model("explicit-model")
        .apply_defaults_from(&provider_defaults());
    assert_eq!(config.model.as_deref(), Some("explicit-model"));
}

#[test]
fn apply_defaults_is_idempotent() {
    let first = AgentConfig::new("test", "/tmp/project").apply_defaults_from(&provider_defaults());
    let first_model = first.model.clone();
    let second = first.apply_defaults_from(&provider_defaults());
    // Calling it twice must not change the outcome.
    assert_eq!(second.model, first_model);
}

#[test]
fn agent_config_default_enables_hooks() {
    // AcceptEdits keeps the agent's hook pipeline running; BypassPermissions
    // would skip it.
    let config = AgentConfig::new("test", "/tmp/project");
    assert_eq!(config.permission_mode, PermissionMode::AcceptEdits);
}

#[test]
fn agent_config_permission_mode_override() {
    let config =
        AgentConfig::new("test", "/tmp/project").permission_mode(PermissionMode::BypassPermissions);
    assert_eq!(config.permission_mode, PermissionMode::BypassPermissions);
}

#[test]
fn agent_config_resume_and_continue() {
    let config = AgentConfig::new("follow up", "/tmp/project")
        .continue_conversation(true)
        .resume("sess-abc123");

    assert!(config.continue_conversation);
    assert_eq!(config.resume.as_deref(), Some("sess-abc123"));
}

// ── with_run_args tests ────────────────────────────────────

#[test]
fn with_run_args_all_set() {
    let config = AgentConfig::new("test", "/tmp/project").with_run_args(
        Some("opus"),
        Some("sess-123"),
        true,
    );

    assert_eq!(config.model.as_deref(), Some("opus"));
    assert_eq!(config.resume.as_deref(), Some("sess-123"));
    assert!(config.continue_conversation);
}

#[test]
fn with_run_args_none() {
    let config = AgentConfig::new("test", "/tmp/project").with_run_args(None, None, false);

    assert!(config.model.is_none());
    assert!(config.resume.is_none());
    assert!(!config.continue_conversation);
}

#[test]
fn with_run_args_partial() {
    let config =
        AgentConfig::new("test", "/tmp/project").with_run_args(Some("sonnet"), None, false);

    assert_eq!(config.model.as_deref(), Some("sonnet"));
    assert!(config.resume.is_none());
    assert!(!config.continue_conversation);
}

// ── Backend tests ──────────────────────────────────────────

#[test]
fn backend_default_is_acp() {
    assert_eq!(Backend::default(), Backend::Acp);
}

#[test]
fn backend_label() {
    assert_eq!(Backend::Acp.label(), "acp");
}

#[test]
fn backend_all_compiled_includes_acp() {
    let all = Backend::all_compiled();
    assert!(all.contains(&Backend::Acp));
}

#[test]
fn backend_capabilities_feature_list_has_eight_items() {
    let caps = Backend::Acp.capabilities();
    let features = caps.feature_list();
    assert_eq!(features.len(), 8);
}

#[test]
fn backend_capabilities_acp_has_cancel_and_state() {
    let caps = Backend::Acp.capabilities();
    assert!(caps.graceful_cancel, "ACP should support graceful cancel");
    assert!(caps.session_state, "ACP should support session state");
    assert!(
        caps.streaming_notifications,
        "ACP should support streaming notifications"
    );
}

#[test]
fn backend_detect_returns_none_without_an_agent_binary() {
    let path = tempfile::tempdir().expect("empty PATH directory");
    if !env_test::in_env(&[("PATH", Some(path.path().as_os_str()))]) {
        return;
    }
    assert_eq!(detect_backend(), None);
}

#[test]
fn backend_is_available_delegates_to_runtime() {
    let path = tempfile::tempdir().expect("empty PATH directory");
    if !env_test::in_env(&[("PATH", Some(path.path().as_os_str()))]) {
        return;
    }
    assert_eq!(
        is_backend_available(Backend::Acp),
        AgentRuntime::is_any_binary_available()
    );
}

#[test]
fn backend_capabilities_acp() {
    let caps = Backend::Acp.capabilities();
    assert_eq!(caps.name, "Claude Code (ACP)");
    assert!(caps.streaming);
    assert!(caps.session_resume);
    assert!(caps.model_selection);
    assert!(caps.mcp_support);
    assert!(caps.permission_profiles);
    assert!(caps.graceful_cancel);
    assert!(caps.session_state);
    assert!(caps.streaming_notifications);
}

#[test]
fn supports_cancel_true_for_acp() {
    assert!(Backend::Acp.supports_cancel());
}

// ── Allowed tools tests ─────────────────────────────────────

#[test]
fn agent_config_has_base_allowed_tools() {
    let config = AgentConfig::new("test", "/tmp/project");
    assert!(
        config.allowed_tools.contains(&"web_search".to_string()),
        "new AgentConfig should include ToolPermissions::base() tools"
    );
}

#[test]
fn allow_tool_deduplicates() {
    let config = AgentConfig::new("test", "/tmp/project")
        .allow_tool("web_search")
        .allow_tool("custom_tool")
        .allow_tool("custom_tool");
    // web_search from defaults + custom_tool (no dupes)
    assert_eq!(
        config
            .allowed_tools
            .iter()
            .filter(|t| *t == "custom_tool")
            .count(),
        1
    );
}

#[test]
fn allowed_tools_replaces_list() {
    let config = AgentConfig::new("test", "/tmp/project").allowed_tools(["only_this"]);
    assert_eq!(config.allowed_tools, vec!["only_this".to_string()]);
}

#[test]
fn permission_context_uses_selected_project_and_preserves_explicit_profile() {
    use crate::permissions::{NoninteractivePolicy, PermissionProfile};
    use magistr_cockpit::agent::{PolicyDecision, SessionPolicy};
    let project = tempfile::tempdir().unwrap();
    std::fs::create_dir(project.path().join(".claude")).unwrap();
    std::fs::write(
        project.path().join(".claude/permissions.toml"),
        "profile = \"cautious\"\nnoninteractive = \"legacy-automatic-approval\"\n",
    )
    .unwrap();
    let config = AgentConfig::new("Review", project.path());
    let (policy, handling) = config.session_permission_policy();
    let mut policy = policy.audit_to(None);
    assert_eq!(handling, NoninteractivePolicy::LegacyAutomaticApproval);
    let (response_tx, _response) = tokio::sync::oneshot::channel();
    let request = magistr_acp::AcpPermissionRequest {
        tool_call_id: "selected-read".into(),
        tool_title: "Read".into(),
        tool_input: serde_json::json!({"file_path":"src/lib.rs"}),
        tool_kind: Some(agent_client_protocol::schema::v1::ToolKind::Read),
        options: vec![],
        response_tx,
    };
    assert!(matches!(policy.decide(&request), PolicyDecision::Ask(_)));
    let (policy, _) = config
        .permission_profile(PermissionProfile::Standard)
        .session_permission_policy();
    assert!(matches!(
        policy.audit_to(None).decide(&request),
        PolicyDecision::Allow
    ));
    assert_eq!(
        crate::commands::run::resolve_permission_profile(project.path(), None).unwrap(),
        PermissionProfile::Cautious
    );
    assert_eq!(
        crate::commands::run::resolve_permission_profile(project.path(), Some("trusted")).unwrap(),
        PermissionProfile::Trusted
    );
}
