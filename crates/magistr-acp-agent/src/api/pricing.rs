//! What a model charges.

/// Per-million-token prices, in USD — the hand-carried tables the plugin
/// bridge priced with, kept as they were.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Pricing {
    /// USD per million input (prompt) tokens.
    pub input_per_million: f64,
    /// USD per million output (completion) tokens.
    pub output_per_million: f64,
}

impl Pricing {
    /// A model that costs nothing — local inference.
    pub const FREE: Self = Self {
        input_per_million: 0.0,
        output_per_million: 0.0,
    };

    /// Prices per million tokens.
    #[must_use]
    pub const fn per_million(input: f64, output: f64) -> Self {
        Self {
            input_per_million: input,
            output_per_million: output,
        }
    }

    /// The cost of one turn, in USD.
    #[must_use]
    pub fn cost(&self, input_tokens: u64, output_tokens: u64) -> f64 {
        (input_tokens as f64 / 1_000_000.0) * self.input_per_million
            + (output_tokens as f64 / 1_000_000.0) * self.output_per_million
    }
}

#[cfg(test)]
#[path = "pricing.tests.rs"]
mod tests;
