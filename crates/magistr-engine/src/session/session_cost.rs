use serde::{Deserialize, Serialize};

/// Accumulated cost and token usage for a session.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SessionCost {
    /// Total cost in USD.
    pub total_usd: f64,
    /// Input tokens consumed.
    pub input_tokens: u64,
    /// Output tokens produced.
    pub output_tokens: u64,
    /// Number of completed turns.
    pub turns: u32,
}

impl Default for SessionCost {
    fn default() -> Self {
        Self {
            total_usd: 0.0,
            input_tokens: 0,
            output_tokens: 0,
            turns: 0,
        }
    }
}

impl SessionCost {
    /// Record a completed turn.
    pub fn record_turn(&mut self, cost_usd: f64, input: u64, output: u64) {
        self.total_usd += cost_usd;
        self.input_tokens += input;
        self.output_tokens += output;
        self.turns += 1;
    }
}
