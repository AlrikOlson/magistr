use super::*;

use crate::{cli_test_support, in_env};

#[tokio::test]
async fn an_unknown_provider_is_refused_naming_the_known_ones() {
    let err = cmd_acp_agent("gpt-9", &[])
        .await
        .expect_err("no such provider");
    let message = err.to_string();
    assert!(
        message.contains("unknown ACP provider `gpt-9`"),
        "{message}"
    );
    for known in ["claude", "codex", "openai", "gemini", "ollama", "scripted"] {
        assert!(
            message.contains(known),
            "lists the known provider {known}: {message}"
        );
    }
}

/// The API providers read their credentials at construction, so a missing
/// key is the subcommand's own refusal — before anything touches stdio.
#[tokio::test]
async fn openai_without_a_key_is_refused_before_serving() {
    if !in_env::in_env(&[("OPENAI_API_KEY", None)]) {
        return;
    }
    let err = cmd_acp_agent("openai", &[]).await.expect_err("no key");
    assert!(
        err.to_string().contains("OPENAI_API_KEY"),
        "names the variable: {err}"
    );
}

/// The scripted provider is served, not refused. Pinned in the source
/// because serving takes stdio and never returns — the real check is the
/// end-to-end test through `AcpTransport` in tests/acp_agent_stdio.rs.
/// The runtime launches every agent with `--allow-tool=<name>` per
/// pre-approved tool (connect_transport); the subcommand must take them.
#[test]
fn allow_tool_flags_parse_on_the_subcommand() {
    let cli = cli_test_support::parse_cli([
        "magistr",
        "acp-agent",
        "--provider",
        "claude",
        "--allow-tool=Bash",
        "--allow-tool=Edit",
    ])
    .expect("parse explicit subcommand arguments");
    match cli.command {
        Some(crate::cli::Command::AcpAgent {
            provider,
            allow_tool,
        }) => {
            assert_eq!(provider, "claude");
            assert_eq!(allow_tool, vec!["Bash", "Edit"]);
        }
        other => panic!("expected acp-agent, got {other:?}"),
    }
}

#[test]
fn the_scripted_provider_is_served_on_stdio() {
    const SOURCE: &str = include_str!("cmd_acp_agent.rs");
    assert!(
        SOURCE
            .contains("ScriptedProvider::NAME => magistr_acp_agent::serve_stdio(ScriptedProvider)")
    );
}
