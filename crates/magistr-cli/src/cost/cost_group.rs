//! Spend for one key of a [`CostDimension`].
//!
//! [`CostDimension`]: super::cost_dimension::CostDimension

use super::cost_summary::CostSummary;

/// Spend attributed to one key of a dimension.
#[derive(Debug, Clone, PartialEq)]
pub struct CostGroup {
    /// The dimension key — a project, an identity, a model, a session.
    pub key: String,
    /// Summary statistics for the entries under this key.
    pub summary: CostSummary,
    /// This key's fraction of total spend, in `0.0..=1.0`.
    pub share: Option<f64>,
}
