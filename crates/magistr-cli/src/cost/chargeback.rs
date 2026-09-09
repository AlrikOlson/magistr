//! The chargeback report — what each cost centre owes, per period.
//!
//! Shares are computed within a period rather than across the whole
//! ledger: a chargeback line answers "what fraction of August did this
//! team spend", and a run of quiet months must not dilute it.

use std::collections::BTreeMap;

use super::chargeback_row::ChargebackRow;
use super::cost_dimension::CostDimension;
use super::cost_entry::CostEntry;
use super::summarize_by_dimension::summarize_by_dimension;
use super::summarize_by_period::bucket_label;
use super::time_period::TimePeriod;

/// Charge every entry to its dimension key, bucketed by period.
///
/// Rows run chronologically, and within a period biggest charge first.
#[must_use]
pub fn chargeback(
    entries: &[CostEntry],
    dimension: CostDimension,
    period: TimePeriod,
) -> Vec<ChargebackRow> {
    let mut periods: BTreeMap<String, Vec<CostEntry>> = BTreeMap::new();
    for entry in entries {
        periods
            .entry(bucket_label(&entry.timestamp, period))
            .or_default()
            .push(entry.clone());
    }

    periods
        .into_iter()
        .flat_map(|(label, in_period)| {
            summarize_by_dimension(&in_period, dimension)
                .into_iter()
                .map(move |group| ChargebackRow {
                    period: label.clone(),
                    key: group.key,
                    cost_usd: group.summary.total_cost,
                    turns: group.summary.total_turns,
                    sessions: group.summary.entry_count,
                    tokens: group.summary.total_tokens,
                    share: group.share,
                })
        })
        .collect()
}

#[cfg(test)]
#[path = "chargeback.tests.rs"]
mod tests;
