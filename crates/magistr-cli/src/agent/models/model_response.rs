//! Response from a single model in a comparison.

use crate::cost::types::TokenBreakdown;

/// Response from a single model.
#[derive(Debug, Clone, PartialEq)]
pub struct ModelResponse {
    /// The model ID that generated this response.
    pub model_id: String,
    /// Human-readable model name.
    pub display_name: String,
    /// The response text.
    pub output: String,
    /// Token usage breakdown.
    pub tokens: TokenBreakdown,
    /// Cost in USD.
    pub cost_usd: f64,
    /// Duration in milliseconds.
    pub duration_ms: u64,
}

impl ModelResponse {
    /// Tokens per second throughput.
    pub fn tokens_per_second(&self) -> f64 {
        if self.duration_ms == 0 {
            return 0.0;
        }
        self.tokens.output_tokens as f64 / (self.duration_ms as f64 / 1000.0)
    }

    /// Output length in characters.
    pub fn output_len(&self) -> usize {
        self.output.len()
    }
}
