//! Configuration for a single AI model.

use serde::{Deserialize, Serialize};

use super::provider::Provider;

/// Configuration for a single AI model.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ModelConfig {
    /// Unique model identifier (e.g., "claude-opus-4", "gpt-4o").
    pub id: String,

    /// AI provider.
    pub provider: Provider,

    /// Human-readable display name (e.g., "Claude Opus 4").
    pub display_name: String,

    /// Cost per 1M input tokens in USD.
    pub cost_per_million_input: f64,

    /// Cost per 1M output tokens in USD.
    pub cost_per_million_output: f64,

    /// Environment variable that holds the API key (e.g., "ANTHROPIC_API_KEY").
    /// `None` for local models that don't need authentication.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub api_key_env: Option<String>,

    /// Optional API base URL override (for local models or proxies).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub base_url: Option<String>,
}

impl ModelConfig {
    /// Check whether the required API key env var is set.
    ///
    /// Returns `true` if no API key is needed (local models) or if the
    /// env var is set and non-empty.
    pub fn is_available(&self) -> bool {
        match &self.api_key_env {
            None => true,
            Some(var) => std::env::var(var).ok().is_some_and(|v| !v.is_empty()),
        }
    }

    /// Calculate cost in USD for a given token usage.
    pub fn calculate_cost(&self, input_tokens: u64, output_tokens: u64) -> f64 {
        let input_cost = (input_tokens as f64 / 1_000_000.0) * self.cost_per_million_input;
        let output_cost = (output_tokens as f64 / 1_000_000.0) * self.cost_per_million_output;
        input_cost + output_cost
    }
}
