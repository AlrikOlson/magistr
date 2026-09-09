//! One line of a chargeback report.

use super::types::TokenBreakdown;

/// What one dimension key owes for one reporting period.
#[derive(Debug, Clone, PartialEq)]
pub struct ChargebackRow {
    /// The period bucket label — `2026-08-29`, `2026-W35`, `2026-08`.
    pub period: String,
    /// The dimension key charged — an identity, a project, a model.
    pub key: String,
    /// Money owed for this period.
    pub cost_usd: f64,
    /// Agent turns metered.
    pub turns: u32,
    /// Sessions metered.
    pub sessions: usize,
    /// Tokens metered.
    pub tokens: TokenBreakdown,
    /// This key's fraction of *this period's* spend, in `0.0..=1.0`.
    pub share: f64,
}
