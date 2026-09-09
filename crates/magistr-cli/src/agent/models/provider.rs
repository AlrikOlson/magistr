//! AI model provider enum.

use serde::{Deserialize, Serialize};

/// AI model provider.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Provider {
    /// Anthropic Claude models.
    Claude,
    /// OpenAI models (GPT-4o, o1, etc.).
    #[serde(alias = "openai")]
    OpenAi,
    /// Local models via Ollama or compatible API.
    Local,
}

impl Provider {
    /// Human-readable label.
    pub fn label(self) -> &'static str {
        match self {
            Self::Claude => "Claude",
            Self::OpenAi => "OpenAI",
            Self::Local => "Local",
        }
    }
}

impl std::fmt::Display for Provider {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.label())
    }
}
