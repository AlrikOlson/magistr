//! Unified agent runtime resolution.
//!
//! `AgentRuntime` is the single source of truth for "which binary do we launch,
//! with what args, using which authentication, as which provider?" It
//! replaces the old scattered approach where auth detection, binary search
//! and preflight checks lived in separate functions that could (and did)
//! disagree.
//!
//! ## Priority order
//!
//! [`AgentRuntime::detect`] resolves the best available runtime in one pass:
//!
//! 0. User's `[session] default_provider` preference (from configuration or config.toml)
//! 1. `claude` on PATH with direct auth (API key or CLI login) — served by
//!    magistr's own ACP agent (`magistr acp-agent --provider claude`,
//!    `crates/magistr-acp-agent`), which drives that `claude` — preferred
//! 2. `codex` on PATH — authentication is checked by Codex during startup
//! 3. `copilot --acp` — authenticated via GitHub session
//! 4. `claude` on PATH without explicit auth — the CLI may hold its own credentials
//! 5. Error naming the missing agent commands

use std::path::PathBuf;

use super::auth_method::AuthMethod;
use crate::config::inputs::ProjectInputs;

/// A resolved agent runtime — binary path, arguments, authentication and the
/// provider it stands for.
///
/// Constructed via [`detect`](Self::detect), which ensures the binary exists
/// and auth is available, or [`in_binary`](Self::in_binary) for the agent
/// this executable serves itself. Pass it to `AcpTransport::new_with_args`
/// to create a transport.
#[derive(Debug, Clone)]
pub struct AgentRuntime {
    /// Absolute path to the agent binary.
    pub path: PathBuf,
    /// Extra CLI arguments (e.g. `["--acp"]` for the Copilot binary).
    pub args: Vec<String>,
    /// How this runtime authenticates.
    pub auth: AuthMethod,
    /// The provider this runtime stands for: `claude`, `copilot`, or the
    /// name an in-binary provider was started with.
    pub provider: String,
}

impl AgentRuntime {
    /// Stable provider identifier for config keys and display grouping —
    /// `"claude"` for Claude Code, `"copilot"` for GitHub Copilot, or the
    /// in-binary provider's own name.
    #[must_use]
    pub fn provider_id(&self) -> &str {
        &self.provider
    }

    /// Human-readable provider name for display in the provider settings.
    #[must_use]
    pub fn provider_label(&self) -> &str {
        Self::provider_label_for(&self.provider)
    }

    #[must_use]
    pub fn provider_label_for(provider: &str) -> &str {
        match provider {
            CLAUDE_PROVIDER => "Claude Code",
            CODEX_PROVIDER => "Codex",
            COPILOT_PROVIDER => "GitHub Copilot",
            OPENAI_PROVIDER => "OpenAI",
            GEMINI_PROVIDER => "Gemini",
            OLLAMA_PROVIDER => "Ollama",
            other => other,
        }
    }
}

// ── Binary and provider names ────────────────────────────────────────────

/// The Claude Code CLI — the one Claude dependency, kept current by the user.
const CLAUDE_CLI: &str = "claude";
const COPILOT_BINARY: &str = "copilot";
const CLAUDE_PROVIDER: &str = "claude";
const CODEX_PROVIDER: &str = magistr_acp_agent::CodexProvider::NAME;
const COPILOT_PROVIDER: &str = "copilot";
/// The API providers the in-binary agent serves directly (chat only): a
/// `[session] default_provider` naming one is honoured as-is, since their
/// credentials are the agent's own affair and checked when it starts.
const OPENAI_PROVIDER: &str = magistr_acp_agent::OpenAi::NAME;
const GEMINI_PROVIDER: &str = magistr_acp_agent::Gemini::NAME;
const OLLAMA_PROVIDER: &str = magistr_acp_agent::Ollama::NAME;
const API_PROVIDERS: [&str; 3] = [OPENAI_PROVIDER, GEMINI_PROVIDER, OLLAMA_PROVIDER];

// ── Detection helpers ────────────────────────────────────────────────────

/// Check whether the Claude CLI at `claude` reports `loggedIn: true`.
///
/// Runs `claude auth status` and parses the JSON output. Returns `false`
/// if the command times out or returns unexpected output.
fn has_cli_login(claude: &std::path::Path) -> bool {
    // Try without --output flag first (works on all versions),
    // fall back to --output json for older CLIs that need it.
    let result = std::process::Command::new(claude)
        .args(["auth", "status"])
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::null())
        .output();

    match result {
        Ok(output) if output.status.success() => {
            let stdout = String::from_utf8_lossy(&output.stdout);
            if let Some(logged_in) = serde_json::from_str::<serde_json::Value>(&stdout)
                .ok()
                .and_then(|v| v.get("loggedIn")?.as_bool())
            {
                return logged_in;
            }
            // Output wasn't JSON — try with --output json flag
            let result_json = std::process::Command::new(claude)
                .args(["auth", "status", "--output", "json"])
                .stdout(std::process::Stdio::piped())
                .stderr(std::process::Stdio::null())
                .output();
            match result_json {
                Ok(out) if out.status.success() => {
                    let stdout = String::from_utf8_lossy(&out.stdout);
                    serde_json::from_str::<serde_json::Value>(&stdout)
                        .ok()
                        .and_then(|v| v.get("loggedIn")?.as_bool())
                        .unwrap_or(false)
                }
                _ => false,
            }
        }
        _ => false,
    }
}

/// The Claude auth the captured environment offers, given that `claude` is
/// on PATH. `has_api_key` is the captured presence of `ANTHROPIC_API_KEY`.
fn claude_auth(has_api_key: bool) -> impl Fn(&std::path::Path) -> AuthMethod {
    move |claude| {
        if has_api_key {
            AuthMethod::ApiKey
        } else if has_cli_login(claude) {
            AuthMethod::CliLogin
        } else {
            AuthMethod::Implicit
        }
    }
}

// ── Core detection ───────────────────────────────────────────────────────

impl AgentRuntime {
    /// The binary the Claude runtime needs on PATH — what the bad day's
    /// agent card names when it is gone.
    pub const AGENT_BINARY: &str = CLAUDE_CLI;

    /// Detect the best available agent runtime for the captured inputs,
    /// finding binaries and checking authentication in a single pass so the
    /// binary launched actually has working credentials.
    ///
    /// When `preferred` is `Some("copilot")`, Copilot is used exclusively.
    /// When `Some("claude")`, Claude Code is used exclusively (any auth method).
    /// When `Some("codex")`, the in-binary agent drives Codex App Server;
    /// runtime discovery checks its installed CLI and ChatGPT login on startup.
    /// When it names an API provider (`openai`, `gemini`, `ollama`), the
    /// in-binary agent serves that API — nothing on PATH is needed.
    /// When `None`, uses the standard priority order. The Claude API key is
    /// read from the captured inputs, never from the environment here.
    ///
    /// # Errors
    ///
    /// Returns an actionable error if no authenticated runtime is available.
    pub fn detect_for(
        inputs: &ProjectInputs,
        preferred: Option<&str>,
    ) -> crate::error::Result<Self> {
        Self::detect_with(
            preferred,
            |name| which::which(name).ok(),
            claude_auth(inputs.has_claude_api_key()),
        )
    }

    /// Resolve using caller-owned discovery and auth facts. This keeps the
    /// selection rules independent of installed tools and cached logins.
    fn detect_with(
        preferred: Option<&str>,
        find: impl Fn(&str) -> Option<PathBuf>,
        auth: impl Fn(&std::path::Path) -> AuthMethod,
    ) -> crate::error::Result<Self> {
        // "auto" is the existing user-config sentinel. Every other explicit
        // identifier must be supported before probing any runtime or auth.
        let preferred = preferred.filter(|provider| *provider != "auto");
        if let Some(provider) = preferred
            && ![CLAUDE_PROVIDER, COPILOT_PROVIDER, CODEX_PROVIDER].contains(&provider)
            && !API_PROVIDERS.contains(&provider)
        {
            return Err(crate::error::MagistrError::Acp(format!(
                "Unsupported agent provider {provider:?}. Select claude, copilot, codex, openai, gemini, ollama, or auto. openai is a chat-only API provider."
            )));
        }
        if let Some(provider) =
            preferred.filter(|p| *p == CODEX_PROVIDER || API_PROVIDERS.contains(p))
        {
            return Self::in_binary(provider).map_err(|e| {
                crate::error::MagistrError::Acp(format!("cannot locate this executable: {e}"))
            });
        }

        let claude_path = find(CLAUDE_CLI);
        let copilot_path = find(COPILOT_BINARY);

        // Explicit provider preference — use ONLY that provider
        match preferred {
            Some(COPILOT_PROVIDER) => {
                return copilot_path.map(Self::copilot).ok_or_else(|| {
                    crate::error::MagistrError::Acp(format!(
                        "GitHub Copilot was selected, but the {COPILOT_BINARY} command was not found on PATH. Install GitHub Copilot, then reopen Magistr."
                    ))
                });
            }
            Some(CLAUDE_PROVIDER) => {
                return claude_path
                    .map(|claude| Self::claude(auth(&claude)))
                    .transpose()?
                    .ok_or_else(|| {
                        crate::error::MagistrError::Acp(format!(
                            "Claude Code was selected, but the {CLAUDE_CLI} command was not found on PATH. Install Claude Code, then reopen Magistr."
                        ))
                    });
            }
            _ => {}
        }

        // Auto-detect: standard priority order
        // 1. claude with direct auth — preferred
        let claude_auth = claude_path.as_deref().map(auth);
        if let Some(auth @ (AuthMethod::ApiKey | AuthMethod::CliLogin)) = claude_auth.clone() {
            return Self::claude(auth);
        }

        // 2. Codex owns its login check during provider startup.
        if find(CODEX_PROVIDER).is_some() {
            return Self::in_binary(CODEX_PROVIDER).map_err(|error| {
                crate::error::MagistrError::Acp(format!("cannot locate this executable: {error}"))
            });
        }

        // 3. copilot --acp — authenticated via GitHub session
        if let Some(path) = copilot_path {
            return Ok(Self::copilot(path));
        }

        // 4. claude without explicit auth — the CLI may hold its own credentials
        if let Some(auth) = claude_auth {
            return Self::claude(auth);
        }

        // 5. No command found. This is not an authentication diagnosis.
        Err(crate::error::MagistrError::Acp(
            "No agents were found. Install Claude Code, Codex or GitHub Copilot, then reopen Magistr. If one is already installed, check that its command is available in your terminal.".into(),
        ))
    }

    /// Check whether any agent binary is available on the system.
    ///
    /// Cheaper than [`detect`](Self::detect) — only checks binary presence, not auth.
    #[must_use]
    pub fn is_any_binary_available() -> bool {
        Self::is_any_binary_available_with(|name| which::which(name).is_ok())
    }

    fn is_any_binary_available_with(exists: impl Fn(&str) -> bool) -> bool {
        exists(CLAUDE_CLI) || exists(CODEX_PROVIDER) || exists(COPILOT_BINARY)
    }

    /// The agent this binary serves itself: `magistr acp-agent --provider
    /// <provider>` (`crates/magistr-acp-agent`), spawned from
    /// `current_exe()` so nothing has to be on PATH. Authentication is the
    /// provider's own affair (`AuthMethod::Implicit`).
    pub fn in_binary(provider: &str) -> std::io::Result<Self> {
        Ok(Self {
            path: std::env::current_exe()?,
            args: vec![
                "acp-agent".to_string(),
                "--provider".to_string(),
                provider.to_string(),
            ],
            auth: AuthMethod::Implicit,
            provider: provider.to_string(),
        })
    }

    /// The Claude runtime: the in-binary `claude` provider, with the auth
    /// the environment offers.
    fn claude(auth: AuthMethod) -> crate::error::Result<Self> {
        let mut runtime = Self::in_binary(CLAUDE_PROVIDER).map_err(|e| {
            crate::error::MagistrError::Acp(format!("cannot locate this executable: {e}"))
        })?;
        runtime.auth = auth;
        Ok(runtime)
    }

    /// The Copilot runtime: `copilot --acp`.
    fn copilot(path: PathBuf) -> Self {
        Self {
            path,
            args: vec!["--acp".to_string()],
            auth: AuthMethod::Copilot,
            provider: COPILOT_PROVIDER.to_string(),
        }
    }

    /// Detect all available agent runtimes (not just the best one).
    ///
    /// Returns one runtime per distinct provider. Used by the provider settings
    /// to probe each provider's model list independently.
    #[must_use]
    pub fn detect_all_for(inputs: &ProjectInputs) -> Vec<Self> {
        let mut runtimes = Self::detect_all_with(
            |name| which::which(name).ok(),
            claude_auth(inputs.has_claude_api_key()),
        );
        for provider in inputs.configured_api_providers() {
            if !runtimes
                .iter()
                .any(|runtime| runtime.provider_id() == provider)
                && let Ok(runtime) = Self::in_binary(provider)
            {
                runtimes.push(runtime);
            }
        }
        runtimes
    }

    fn detect_all_with(
        find: impl Fn(&str) -> Option<PathBuf>,
        auth: impl Fn(&std::path::Path) -> AuthMethod,
    ) -> Vec<Self> {
        let mut runtimes = Vec::new();

        // Claude Code (best available auth), through the in-binary agent
        if let Some(claude) = find(CLAUDE_CLI)
            && let Ok(runtime) = Self::claude(auth(&claude))
        {
            runtimes.push(runtime);
        }

        if find(CODEX_PROVIDER).is_some()
            && let Ok(runtime) = Self::in_binary(CODEX_PROVIDER)
        {
            runtimes.push(runtime);
        }

        if find(OLLAMA_PROVIDER).is_some()
            && let Ok(runtime) = Self::in_binary(OLLAMA_PROVIDER)
        {
            runtimes.push(runtime);
        }

        // GitHub Copilot
        if let Some(path) = find(COPILOT_BINARY) {
            runtimes.push(Self::copilot(path));
        }

        runtimes
    }
}

#[cfg(test)]
#[path = "runtime.tests.rs"]
mod tests;
