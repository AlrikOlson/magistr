//! Typography utilities — cost formatting.
//!
//! Centralizes display formatting for consistent CLI and cockpit output.
//! All formatters are pure functions with no side effects.

/// Format a cost value in USD with consistent decimal places.
///
/// - Values < $0.01 show 4 decimal places: `$0.0042`
/// - Values >= $0.01 show exactly 2 decimal places: `$1.23`
#[must_use]
pub(crate) fn format_cost(cost_usd: f64) -> String {
    if cost_usd < 0.01 {
        format!("${:.4}", cost_usd)
    } else {
        format!("${:.2}", cost_usd)
    }
}

#[cfg(test)]
#[path = "typography.tests.rs"]
mod tests;
