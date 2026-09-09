//! Group cost entries into time buckets and summarize each.

use std::collections::BTreeMap;

use super::cost_entry::CostEntry;
use super::cost_summary::CostSummary;
use super::time_bucket::TimeBucket;
use super::time_period::TimePeriod;

/// Group entries into time buckets and summarize each.
///
/// Returns buckets sorted chronologically (oldest first).
pub fn summarize_by_period(entries: &[CostEntry], period: TimePeriod) -> Vec<TimeBucket> {
    let mut buckets: BTreeMap<String, Vec<CostEntry>> = BTreeMap::new();

    for entry in entries {
        let label = bucket_label(&entry.timestamp, period);
        buckets.entry(label).or_default().push(entry.clone());
    }

    buckets
        .into_iter()
        .map(|(label, entries)| TimeBucket {
            label,
            summary: CostSummary::from_entries(&entries),
        })
        .collect()
}

/// Generate a bucket label from a timestamp and period.
pub(crate) fn bucket_label(ts: &chrono::DateTime<chrono::Utc>, period: TimePeriod) -> String {
    match period {
        TimePeriod::Daily => ts.format("%Y-%m-%d").to_string(),
        TimePeriod::Weekly => ts.format("%G-W%V").to_string(),
        TimePeriod::Monthly => ts.format("%Y-%m").to_string(),
    }
}

#[cfg(test)]
#[path = "summarize_by_period.tests.rs"]
mod tests;
