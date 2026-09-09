//! Summarize a set of cost entries.

use super::cost_entry::CostEntry;
use super::cost_summary::CostSummary;

/// Summarize a set of entries.
pub fn summarize(entries: &[CostEntry]) -> CostSummary {
    CostSummary::from_entries(entries)
}

#[cfg(test)]
#[path = "summarize_costs.tests.rs"]
mod tests;
