//! Time-bucketed cost summary.

use super::cost_summary::CostSummary;

/// A cost summary for a specific time bucket.
#[derive(Debug, Clone)]
pub struct TimeBucket {
    /// Label for this bucket (e.g., "2026-03-10", "2026-W11", "2026-03").
    pub label: String,
    /// Summary statistics for entries in this bucket.
    pub summary: CostSummary,
}
