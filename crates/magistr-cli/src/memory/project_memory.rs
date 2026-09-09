//! Project memory — a collection of categorized observations about a project.

use super::memory_category::MemoryCategory;
use super::memory_entry::MemoryEntry;

/// Project memory — a collection of categorized observations about a project.
///
/// Stored as `.claude/memory.md` in the project directory.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct ProjectMemory {
    /// The project name (used in the markdown title).
    pub project_name: String,
    /// All memory entries.
    pub entries: Vec<MemoryEntry>,
}

impl ProjectMemory {
    /// Create empty memory for a project.
    pub fn new(project_name: impl Into<String>) -> Self {
        Self {
            project_name: project_name.into(),
            entries: Vec::new(),
        }
    }

    /// Add an entry to the memory.
    pub fn add(&mut self, entry: MemoryEntry) {
        self.entries.push(entry);
    }

    /// Add a timestamped observation.
    pub fn add_observation(&mut self, text: impl Into<String>) {
        self.entries
            .push(MemoryEntry::now(MemoryCategory::Observations, text));
    }

    /// Get all entries in a specific category.
    pub fn by_category(&self, category: MemoryCategory) -> Vec<&MemoryEntry> {
        self.entries
            .iter()
            .filter(|e| e.category == category)
            .collect()
    }

    /// Total number of entries.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Whether the memory is empty.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Remove entries matching a predicate.
    pub fn retain(&mut self, f: impl Fn(&MemoryEntry) -> bool) {
        self.entries.retain(f);
    }

    /// Count entries per category.
    pub fn category_counts(&self) -> Vec<(MemoryCategory, usize)> {
        MemoryCategory::ALL
            .iter()
            .map(|&cat| {
                let count = self.entries.iter().filter(|e| e.category == cat).count();
                (cat, count)
            })
            .filter(|(_, count)| *count > 0)
            .collect()
    }
}

#[cfg(test)]
#[path = "project_memory.tests.rs"]
mod tests;
