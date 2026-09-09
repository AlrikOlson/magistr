//! Authentication method for agent runtimes.

/// How the agent binary authenticates with the API.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AuthMethod {
    /// `ANTHROPIC_API_KEY` environment variable.
    ApiKey,
    /// Claude CLI OAuth/subscription login (`claude login`).
    CliLogin,
    /// GitHub Copilot CLI — authenticated via `gh auth` / GitHub session.
    Copilot,
    /// No explicit auth detected; the binary may handle auth internally.
    Implicit,
}

impl AuthMethod {
    /// Human-readable label for display.
    #[must_use]
    pub fn label(&self) -> &'static str {
        match self {
            Self::ApiKey => "API key (ANTHROPIC_API_KEY)",
            Self::CliLogin => "Claude CLI login (subscription)",
            Self::Copilot => "GitHub Copilot CLI",
            Self::Implicit => "implicit (binary-managed)",
        }
    }
}
