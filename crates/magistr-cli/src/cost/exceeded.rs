//! Budget exceeded screen — session summary and extension options.
//!
//! Provides a data structure for rendering a "budget exceeded" screen
//! in the interface, including session cost breakdown, turn history, and
//! instructions for extending the budget.

use super::cost_summary::CostSummary;
use super::types::TokenBreakdown;

/// Data for the "budget exceeded" screen shown when a session hits its budget limit.
///
/// This is a pure data struct — rendering is handled by the interface module.
#[derive(Debug, Clone, PartialEq)]
pub struct BudgetExceededScreen {
    /// The budget limit that was exceeded (USD).
    pub limit: f64,
    /// Total amount spent (USD).
    pub total_spent: f64,
    /// Amount over the limit (USD).
    pub overage: f64,
    /// Number of turns completed.
    pub turns_completed: u32,
    /// Token usage breakdown.
    pub tokens: TokenBreakdown,
    /// Average cost per turn (USD).
    pub avg_cost_per_turn: f64,
    /// Per-turn cost history for sparkline display.
    pub turn_costs: Vec<f64>,
    /// Suggested new budget (2x the current limit).
    pub suggested_extension: f64,
}

impl BudgetExceededScreen {
    /// Build a budget exceeded screen from session state.
    ///
    /// `limit` is the configured budget limit.
    /// `total_spent` is how much has been spent so far.
    /// `turns` is the number of completed turns.
    /// `tokens` is the cumulative token usage.
    /// `turn_costs` is the per-turn cost history.
    pub fn new(
        limit: f64,
        total_spent: f64,
        turns: u32,
        tokens: &TokenBreakdown,
        turn_costs: &[f64],
    ) -> Self {
        let overage = (total_spent - limit).max(0.0);
        let avg_cost_per_turn = if turns > 0 {
            total_spent / f64::from(turns)
        } else {
            0.0
        };

        Self {
            limit,
            total_spent,
            overage,
            turns_completed: turns,
            tokens: tokens.clone(),
            avg_cost_per_turn,
            turn_costs: turn_costs.to_vec(),
            suggested_extension: limit * 2.0,
        }
    }

    /// Format the session summary as a multi-line string.
    ///
    /// Used for both TUI rendering and `--no-tui` plain text output.
    #[must_use]
    pub fn summary(&self) -> String {
        let mut lines = Vec::new();
        lines.push("━━━ Budget Exceeded ━━━".to_string());
        lines.push(String::new());
        lines.push(format!("  Limit:       ${:.2}", self.limit));
        lines.push(format!(
            "  Spent:       ${:.2}  (+${:.2} over)",
            self.total_spent, self.overage
        ));
        lines.push(format!("  Turns:       {}", self.turns_completed));
        lines.push(format!("  Avg/turn:    ${:.4}", self.avg_cost_per_turn));
        lines.push(String::new());
        lines.push(format!(
            "  Tokens:      {} input, {} output",
            self.tokens.input_tokens, self.tokens.output_tokens
        ));
        if self.tokens.cache_read_tokens > 0 || self.tokens.cache_write_tokens > 0 {
            lines.push(format!(
                "  Cache:       {} read, {} write",
                self.tokens.cache_read_tokens, self.tokens.cache_write_tokens
            ));
        }
        lines.push(String::new());
        lines.push("  To continue, re-run with a higher budget:".to_string());
        lines.push(format!(
            "    magistr run --budget {:.2} --continue",
            self.suggested_extension
        ));

        lines.join("\n")
    }

    /// Build from a `CostSummary` and budget limit (convenience for post-session reports).
    pub fn from_summary(limit: f64, summary: &CostSummary, turn_costs: &[f64]) -> Self {
        Self::new(
            limit,
            summary.total_cost,
            summary.total_turns,
            &summary.total_tokens,
            turn_costs,
        )
    }
}

#[cfg(test)]
#[path = "exceeded.tests.rs"]
mod tests;
