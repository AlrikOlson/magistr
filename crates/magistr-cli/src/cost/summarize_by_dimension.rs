//! Group cost entries by an attribution dimension — the metering half of
//! cost governance: who, on what, on which model.

use std::collections::BTreeMap;

use super::cost_dimension::CostDimension;
use super::cost_entry::CostEntry;
use super::cost_group::CostGroup;
use super::cost_summary::CostSummary;

/// Group entries by `dimension`, biggest spender first.
///
/// Ties break on the key so a report of equal spenders is stable rather
/// than reordering between runs.
#[must_use]
pub fn summarize_by_dimension(entries: &[CostEntry], dimension: CostDimension) -> Vec<CostGroup> {
    let mut buckets: BTreeMap<String, Vec<CostEntry>> = BTreeMap::new();
    for entry in entries {
        buckets
            .entry(dimension.key_of(entry))
            .or_default()
            .push(entry.clone());
    }

    let total: f64 = entries.iter().map(|e| e.cost_usd).sum();

    let mut groups: Vec<CostGroup> = buckets
        .into_iter()
        .map(|(key, entries)| {
            let summary = CostSummary::from_entries(&entries);
            let share = if total > 0.0 {
                summary.total_cost / total
            } else {
                0.0
            };
            CostGroup {
                key,
                summary,
                share,
            }
        })
        .collect();

    groups.sort_by(|a, b| {
        b.summary
            .total_cost
            .total_cmp(&a.summary.total_cost)
            .then_with(|| a.key.cmp(&b.key))
    });
    groups
}

#[cfg(test)]
#[path = "summarize_by_dimension.tests.rs"]
mod tests;
