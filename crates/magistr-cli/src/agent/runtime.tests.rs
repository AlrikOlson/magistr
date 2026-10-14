use super::*;

// A capture with nothing set: no API key, no home — detection sees only PATH.
fn inputs() -> ProjectInputs {
    ProjectInputs::from_lookup("/proj", |_| None)
}

#[test]
fn an_unsupported_provider_never_falls_back_to_another_runtime() {
    for provider in ["cluade", "", "OPENAI"] {
        let result = AgentRuntime::detect_for(&inputs(), Some(provider));
        assert!(
            result
                .as_ref()
                .is_err_and(|error| error.to_string().contains("Unsupported agent provider")),
            "{provider:?} must be rejected before probing or launching another provider: {result:?}"
        );
    }
}

#[test]
fn explicit_codex_resolves_to_the_coding_adapter_without_auth_probing() {
    let runtime = AgentRuntime::detect_for(&inputs(), Some("codex")).expect("Codex adapter");
    assert_eq!(
        runtime.path,
        std::env::current_exe().expect("current executable")
    );
    assert_eq!(runtime.args, vec!["acp-agent", "--provider", "codex"]);
    assert_eq!(runtime.provider_id(), "codex");
    assert_eq!(runtime.provider_label(), "Codex");
    assert_eq!(runtime.auth, AuthMethod::Implicit);
    assert_ne!(runtime.provider_id(), magistr_acp_agent::OpenAi::NAME);
}

#[test]
fn installed_codex_is_an_available_choice_without_creating_a_provider() {
    let choices = AgentRuntime::detect_all_with(
        |name| (name == "codex").then(|| PathBuf::from("/tools/codex")),
        |_| panic!("Codex login belongs to startup"),
    );
    assert_eq!(choices.len(), 1);
    assert_eq!(choices[0].provider_id(), "codex");
}

// ── AuthMethod ───────────────────────────────────────────────

#[test]
fn auth_method_labels_are_descriptive() {
    assert!(AuthMethod::ApiKey.label().contains("API key"));
    assert!(AuthMethod::CliLogin.label().contains("subscription"));
    assert!(AuthMethod::Copilot.label().contains("Copilot"));
    assert!(AuthMethod::Implicit.label().contains("implicit"));
}

// ── AgentRuntime::detect ────────────────────────────────────

fn binaries(claude: bool, copilot: bool) -> impl Fn(&str) -> Option<PathBuf> {
    move |name| match name {
        CLAUDE_CLI if claude => Some(PathBuf::from("/fixture/claude")),
        COPILOT_BINARY if copilot => Some(PathBuf::from("/fixture/copilot")),
        _ => None,
    }
}

#[test]
fn automatic_detection_prefers_direct_claude_auth_then_copilot_then_implicit_claude() {
    for auth in [AuthMethod::ApiKey, AuthMethod::CliLogin] {
        let runtime = AgentRuntime::detect_with(None, binaries(true, true), |_| auth.clone())
            .expect("authenticated Claude");
        assert_eq!(runtime.provider_id(), "claude");
        assert_eq!(runtime.auth, auth);
    }
    let runtime = AgentRuntime::detect_with(None, binaries(true, true), |_| AuthMethod::Implicit)
        .expect("Copilot before implicit Claude");
    assert_eq!(runtime.provider_id(), "copilot");
    assert_eq!(runtime.args, vec!["--acp"]);
    let runtime = AgentRuntime::detect_with(None, binaries(true, false), |_| AuthMethod::Implicit)
        .expect("implicit Claude fallback");
    assert_eq!(runtime.provider_id(), "claude");
    assert_eq!(runtime.auth, AuthMethod::Implicit);
    assert!(
        AgentRuntime::detect_with(None, binaries(false, false), |_| panic!(
            "no binary to authenticate"
        ))
        .is_err()
    );
}

#[test]
fn an_explicit_provider_never_falls_back_when_its_binary_is_missing() {
    for provider in ["claude", "copilot"] {
        let found = binaries(provider != "claude", provider != "copilot");
        let err = AgentRuntime::detect_with(Some(provider), found, |_| {
            panic!("no auth probe for missing preference")
        })
        .expect_err("preferred binary missing");
        assert!(err.to_string().contains("not found on PATH"), "{err}");
    }
}

#[test]
fn preferring_claude_lands_on_the_in_binary_agent() {
    let rt =
        AgentRuntime::detect_with(Some("claude"), binaries(true, true), |_| AuthMethod::ApiKey)
            .expect("fixture Claude");
    assert_eq!(rt.path, std::env::current_exe().expect("current_exe"));
    assert_eq!(rt.args, vec!["acp-agent", "--provider", "claude"]);
    assert_eq!(rt.provider_id(), "claude");
    assert_eq!(rt.provider_label(), "Claude Code");
}

#[test]
fn availability_requires_either_supported_binary() {
    for claude in [false, true] {
        for copilot in [false, true] {
            assert_eq!(
                AgentRuntime::is_any_binary_available_with(
                    |name| binaries(claude, copilot)(name).is_some()
                ),
                claude || copilot
            );
        }
    }
    assert_eq!(AgentRuntime::AGENT_BINARY, "claude");
}

// ── Error message ────────────────────────────────────────────

#[test]
fn no_runtime_message_mentions_install_options() {
    let msg = AgentRuntime::no_runtime_message_with(false);
    assert!(msg.contains("Option 1"));
    assert!(msg.contains("Option 2"));
    assert!(msg.contains("Option 3"));
}

#[test]
fn no_runtime_message_names_the_real_installers() {
    let msg = AgentRuntime::no_runtime_message_with(false);
    assert!(msg.contains(COPILOT_INSTALL), "{msg}");
    assert!(
        msg.contains(CLAUDE_INSTALL) || msg.contains("claude auth login"),
        "{msg}"
    );
    assert!(
        !msg.contains("npm install"),
        "no npm shim is ever hinted: {msg}"
    );
}

// ── Provider ID / label ──────────────────────────────────────

#[test]
fn provider_id_and_label_come_from_the_provider_not_the_auth() {
    let rt = |provider: &str, auth: AuthMethod| AgentRuntime {
        path: std::path::PathBuf::from("/usr/bin/test"),
        args: vec![],
        auth,
        provider: provider.to_owned(),
    };
    assert_eq!(rt("claude", AuthMethod::ApiKey).provider_id(), "claude");
    assert_eq!(
        rt("claude", AuthMethod::Implicit).provider_label(),
        "Claude Code"
    );
    assert_eq!(rt("copilot", AuthMethod::Copilot).provider_id(), "copilot");
    assert_eq!(
        rt("copilot", AuthMethod::Copilot).provider_label(),
        "GitHub Copilot"
    );
    assert_eq!(
        rt("scripted", AuthMethod::Implicit).provider_id(),
        "scripted"
    );
    assert_eq!(
        rt("scripted", AuthMethod::Implicit).provider_label(),
        "scripted"
    );
}

// ── detect_all ───────────────────────────────────────────────

#[test]
fn detect_all_lists_each_available_provider_once() {
    for claude in [false, true] {
        for copilot in [false, true] {
            let runtimes =
                AgentRuntime::detect_all_with(binaries(claude, copilot), |_| AuthMethod::CliLogin);
            let providers: Vec<_> = runtimes.iter().map(|r| r.provider_id()).collect();
            let expected: Vec<_> = [("claude", claude), ("copilot", copilot)]
                .into_iter()
                .filter_map(|(name, exists)| exists.then_some(name))
                .collect();
            assert_eq!(providers, expected);
            if claude {
                assert_eq!(runtimes[0].args, vec!["acp-agent", "--provider", "claude"]);
                assert_eq!(runtimes[0].auth, AuthMethod::CliLogin);
            }
        }
    }
}

// ── API providers ───────────────────────────────────────────

#[test]
fn preferring_an_api_provider_lands_on_the_in_binary_agent_without_any_binary_on_path() {
    for (name, label) in [
        ("openai", "OpenAI"),
        ("gemini", "Gemini"),
        ("ollama", "Ollama"),
    ] {
        let rt = AgentRuntime::detect_for(&inputs(), Some(name)).expect("served in-binary");
        assert_eq!(rt.path, std::env::current_exe().expect("current_exe"));
        assert_eq!(rt.args, vec!["acp-agent", "--provider", name]);
        assert_eq!(rt.auth, AuthMethod::Implicit);
        assert_eq!(rt.provider_id(), name);
        assert_eq!(rt.provider_label(), label);
    }
}

// ── AgentRuntime::in_binary ─────────────────────────────────

#[test]
fn in_binary_spawns_this_executable_as_the_acp_agent() {
    let runtime = AgentRuntime::in_binary("scripted").expect("current_exe");
    assert_eq!(
        runtime.path,
        std::env::current_exe().expect("current_exe"),
        "the agent is this very binary"
    );
    assert_eq!(runtime.args, vec!["acp-agent", "--provider", "scripted"]);
    assert_eq!(runtime.auth, AuthMethod::Implicit);
    assert_eq!(runtime.provider_id(), "scripted");
}
