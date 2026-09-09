use color_eyre::eyre::{self, bail};
use magistr_acp_agent::{
    ApiProvider, ClaudeProvider, CodexProvider, Gemini, Ollama, OpenAi, ScriptedProvider,
};

/// The providers `--provider` can name, for the error that lists them.
const PROVIDERS: &[&str] = &[
    ClaudeProvider::NAME,
    CodexProvider::NAME,
    OpenAi::NAME,
    Gemini::NAME,
    Ollama::NAME,
    ScriptedProvider::NAME,
];

/// Serve magistr's own ACP agent on stdio for the provider named — the
/// agent side of the bridge the cockpit speaks as a client
/// (`crates/magistr-acp-agent`), the way `mcp-serve` serves MCP.
///
/// The runtime spawns this binary with `acp-agent --provider <p>`
/// ([`AgentRuntime::in_binary`](crate::agent::AgentRuntime::in_binary)),
/// so no agent lands on PATH. `claude` drives the Claude Code CLI found on
/// PATH; `codex` drives Codex App Server with its existing ChatGPT login.
/// `openai`, `gemini` and `ollama` drive those HTTP APIs directly
/// (chat only — no tools, so never a permission ask), with the key from
/// `OPENAI_API_KEY` / `GEMINI_API_KEY` and the daemon from `OLLAMA_HOST`;
/// `scripted` is the model-less smoke provider. `allowed_tools` are the
/// `--allow-tool` flags every runtime is launched with: the Claude provider
/// hands them to the CLI as its allowed tools, so they run without a
/// permission ask; the API providers run no tools and ignore them. Nothing
/// may print to stdout once the agent runs: stdout is the JSON-RPC
/// transport.
pub async fn cmd_acp_agent(provider: &str, allowed_tools: &[String]) -> eyre::Result<()> {
    match provider {
        ClaudeProvider::NAME => {
            let claude = which::which(crate::agent::AgentRuntime::AGENT_BINARY).map_err(|_| {
                eyre::eyre!(
                    "`{}` is not on PATH; install Claude Code: curl -fsSL https://claude.ai/install.sh | bash",
                    crate::agent::AgentRuntime::AGENT_BINARY
                )
            })?;
            let provider = ClaudeProvider::discover(claude)
                .await
                .map_err(|e| eyre::eyre!("claude did not complete the handshake: {e}"))?
                .allowed_tools(allowed_tools.to_vec());
            serve(provider).await
        }
        CodexProvider::NAME => {
            let codex = which::which("codex").map_err(|_| {
                eyre::eyre!(
                    "`codex` is not on PATH; install Codex CLI and run `codex login` with ChatGPT"
                )
            })?;
            let provider = CodexProvider::discover(codex).await.map_err(|error| {
                eyre::eyre!("Codex App Server did not complete discovery: {error}")
            })?;
            serve(provider).await
        }
        OpenAi::NAME => serve(ApiProvider::new(OpenAi::from_env()?)).await,
        Gemini::NAME => serve(ApiProvider::new(Gemini::from_env()?)).await,
        Ollama::NAME => serve(ApiProvider::new(Ollama::discover().await?)).await,
        ScriptedProvider::NAME => magistr_acp_agent::serve_stdio(ScriptedProvider)
            .await
            .map_err(|e| eyre::eyre!("ACP agent ended with an error: {e}")),
        other => bail!(
            "unknown ACP provider `{other}`; known providers: {}",
            PROVIDERS.join(", ")
        ),
    }
}

async fn serve(provider: impl magistr_acp_agent::Provider) -> eyre::Result<()> {
    magistr_acp_agent::serve_stdio(provider)
        .await
        .map_err(|e| eyre::eyre!("ACP agent ended with an error: {e}"))
}

#[cfg(test)]
#[path = "cmd_acp_agent.tests.rs"]
mod tests;
