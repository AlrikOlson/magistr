//! Spec file parsing — pure content analysis for driving workflow iterations.
//!
//! Parses markdown checkbox items (`- [x]` / `- [ ]`) from spec file content
//! and produces a [`SpecSummary`] with progress counts and remaining items.
//!
//! This module is IO-free. File reading is handled by infrastructure adapters
//! in `magistr-cli` that call [`SpecSummary::parse`] with the file's text.

use serde::{Deserialize, Serialize};

/// Summary of a spec file's checkbox item status.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SpecSummary {
    /// Total number of checkbox items (checked + unchecked).
    pub total: usize,
    /// Number of completed (checked) items.
    pub checked: usize,
    /// Number of remaining (unchecked) items.
    pub unchecked: usize,
    /// The unchecked item lines (trimmed text after `- [ ] `).
    pub unchecked_items: Vec<String>,
}

impl SpecSummary {
    /// Parse spec content string into a summary.
    ///
    /// Scans for markdown checkbox patterns:
    /// - `- [x]` or `- [X]` → checked item
    /// - `- [ ]` → unchecked item
    ///
    /// Items may be indented. Non-checkbox lines are ignored.
    pub fn parse(content: &str) -> Self {
        let mut checked = 0;
        let mut unchecked = 0;
        let mut unchecked_items = Vec::new();

        for line in content.lines() {
            let trimmed = line.trim();
            if trimmed.starts_with("- [x]") || trimmed.starts_with("- [X]") {
                checked += 1;
            } else if trimmed.starts_with("- [ ]") {
                unchecked += 1;
                let item = trimmed.strip_prefix("- [ ] ").unwrap_or(trimmed);
                unchecked_items.push(item.to_string());
            }
        }

        Self {
            total: checked + unchecked,
            checked,
            unchecked,
            unchecked_items,
        }
    }

    /// Whether all items are complete.
    pub fn all_done(&self) -> bool {
        self.unchecked == 0 && self.total > 0
    }

    /// Progress as a fraction (0.0 to 1.0).
    pub fn progress(&self) -> f64 {
        if self.total == 0 {
            return 0.0;
        }
        self.checked as f64 / self.total as f64
    }

    /// Build a brief status line showing current workflow state.
    pub fn status_line(&self) -> String {
        if self.all_done() {
            "All spec items are complete.".to_string()
        } else {
            format!(
                "{}/{} items complete ({:.0}%), {} remaining",
                self.checked,
                self.total,
                self.progress() * 100.0,
                self.unchecked,
            )
        }
    }
}

#[cfg(test)]
#[path = "spec.tests.rs"]
mod tests;
