//! Cost optimization hint type.

use super::hint_severity::HintSeverity;

/// A cost optimization hint with actionable advice.
#[derive(Debug, Clone, PartialEq)]
pub struct OptimizationHint {
    /// Short title for the hint (e.g., "Context bloat detected").
    pub title: String,
    /// Detailed description of the issue.
    pub description: String,
    /// Estimated savings per turn in USD (if applicable).
    pub estimated_savings_per_turn: Option<f64>,
    /// Severity level.
    pub severity: HintSeverity,
}

#[cfg(test)]
#[path = "optimization.tests.rs"]
mod tests;
