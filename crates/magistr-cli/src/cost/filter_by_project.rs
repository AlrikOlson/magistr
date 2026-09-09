//! Filter cost entries by project name.

use super::cost_entry::CostEntry;

/// Filter entries by project name (case-insensitive substring match).
pub fn filter_by_project(entries: &[CostEntry], project: &str) -> Vec<CostEntry> {
    let needle = project.to_lowercase();
    entries
        .iter()
        .filter(|e| e.project.to_lowercase().contains(&needle))
        .cloned()
        .collect()
}

#[cfg(test)]
#[path = "filter_by_project.tests.rs"]
mod tests;
