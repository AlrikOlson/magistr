//! Token usage breakdown for sessions and turns.

use serde::{Deserialize, Serialize};

/// Breakdown of token usage for a session or turn.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct TokenBreakdown {
    pub input_tokens: u64,
    pub output_tokens: u64,
    #[serde(default)]
    pub cache_read_tokens: u64,
    #[serde(default)]
    pub cache_write_tokens: u64,
}

impl TokenBreakdown {
    /// Total tokens across all categories.
    pub fn total(&self) -> u64 {
        self.input_tokens + self.output_tokens + self.cache_read_tokens + self.cache_write_tokens
    }

    /// Accumulate another breakdown into this one.
    pub fn accumulate(&mut self, other: &TokenBreakdown) {
        self.input_tokens += other.input_tokens;
        self.output_tokens += other.output_tokens;
        self.cache_read_tokens += other.cache_read_tokens;
        self.cache_write_tokens += other.cache_write_tokens;
    }
}

#[cfg(test)]
#[path = "types.tests.rs"]
mod tests;
