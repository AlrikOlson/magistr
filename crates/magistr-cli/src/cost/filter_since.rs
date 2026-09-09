//! Filter cost entries by timestamp.

use super::cost_entry::CostEntry;

/// Filter entries by minimum timestamp.
pub fn filter_since(entries: &[CostEntry], since: chrono::DateTime<chrono::Utc>) -> Vec<CostEntry> {
    entries
        .iter()
        .filter(|e| e.timestamp >= since)
        .cloned()
        .collect()
}

#[cfg(test)]
#[path = "filter_since.tests.rs"]
mod tests;
