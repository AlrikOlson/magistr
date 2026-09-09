//! Cost estimate data type.

/// A cost estimate with low and high bounds.
#[derive(Debug, Clone, PartialEq)]
pub struct CostEstimate {
    /// Estimated low cost (25th percentile per turn × estimated turns).
    pub low: f64,
    /// Estimated high cost (75th percentile per turn × estimated turns).
    pub high: f64,
    /// Estimated number of turns.
    pub estimated_turns: u32,
    /// Average cost per turn from historical data.
    pub avg_cost_per_turn: f64,
    /// Number of historical sessions used for the estimate.
    pub sample_size: usize,
}

impl CostEstimate {
    /// Human-readable estimate string.
    pub fn display(&self) -> String {
        if self.low < 0.01 && self.high < 0.01 {
            format!(
                "${:.4}–${:.4} (~{} turns)",
                self.low, self.high, self.estimated_turns
            )
        } else {
            format!(
                "${:.2}–${:.2} (~{} turns)",
                self.low, self.high, self.estimated_turns
            )
        }
    }
}

#[cfg(test)]
#[path = "estimate.tests.rs"]
mod tests;
