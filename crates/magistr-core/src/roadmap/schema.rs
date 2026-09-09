//! YAML-serializable roadmap document type.
//!
//! The `Roadmap` struct defines the top-level document schema for
//! `.magistr/roadmap.yaml`. Individual types (`Phase`, `Task`, `PhaseStatus`)
//! live in their own modules.

use serde::{Deserialize, Serialize};

use crate::error::{MagistrError, Result};

use super::phase::Phase;

/// A complete roadmap document.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Roadmap {
    /// Document title (rendered as H1 in markdown).
    pub title: String,

    /// Introductory prose before the phase list.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub preamble: String,

    /// High-level vision statement.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub vision: String,

    /// Guiding principles (rendered as bullet list).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub principles: Vec<String>,

    /// Ordered list of roadmap phases.
    pub phases: Vec<Phase>,

    /// Closing prose after all phases (success criteria, migration notes, etc.).
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub epilogue: String,
}

impl Roadmap {
    /// Validate the roadmap structure.
    ///
    /// Checks:
    /// - Title is non-empty
    /// - All phase IDs are non-empty and unique
    /// - All `depends_on` references point to existing phase IDs
    /// - No circular dependencies
    pub fn validate(&self) -> Result<()> {
        if self.title.trim().is_empty() {
            return Err(MagistrError::Config("roadmap title cannot be empty".into()));
        }

        // Check unique, non-empty phase IDs
        let mut seen_ids = std::collections::HashSet::new();
        for phase in &self.phases {
            if phase.id.trim().is_empty() {
                return Err(MagistrError::Config(format!(
                    "phase '{}' has an empty ID",
                    phase.name
                )));
            }
            if !seen_ids.insert(&phase.id) {
                return Err(MagistrError::Config(format!(
                    "duplicate phase ID: '{}'",
                    phase.id
                )));
            }
        }

        // Check dependency references
        for phase in &self.phases {
            for dep in &phase.depends_on {
                if !seen_ids.contains(dep) {
                    return Err(MagistrError::Config(format!(
                        "phase '{}' depends on unknown phase '{dep}'",
                        phase.id
                    )));
                }
                if dep == &phase.id {
                    return Err(MagistrError::Config(format!(
                        "phase '{}' depends on itself",
                        phase.id
                    )));
                }
            }
        }

        // Check for circular dependencies (BFS)
        for phase in &self.phases {
            if has_cycle(&self.phases, &phase.id) {
                return Err(MagistrError::Config(format!(
                    "circular dependency detected involving phase '{}'",
                    phase.id
                )));
            }
        }

        Ok(())
    }

    /// Recompute phase statuses from task completion.
    pub fn recompute_statuses(&mut self) {
        for phase in &mut self.phases {
            phase.recompute_status();
        }
    }

    /// Count total tasks (including subtasks) across all phases.
    #[must_use]
    pub fn total_tasks(&self) -> usize {
        self.phases.iter().map(|p| p.total_tasks()).sum()
    }

    /// Count checked tasks (including subtasks) across all phases.
    #[must_use]
    pub fn checked_tasks(&self) -> usize {
        self.phases.iter().map(|p| p.checked_tasks()).sum()
    }
}

// ── Helpers ─────────────────────────────────────────────────────────────────

/// Check if there's a cycle reachable from `start_id` via `depends_on`.
fn has_cycle(phases: &[Phase], start_id: &str) -> bool {
    let mut visited = std::collections::HashSet::new();
    let mut stack = vec![start_id.to_string()];

    while let Some(current) = stack.pop() {
        if !visited.insert(current.clone()) {
            if current == start_id && visited.len() > 1 {
                return true;
            }
            continue;
        }
        if let Some(phase) = phases.iter().find(|p| p.id == current) {
            for dep in &phase.depends_on {
                if dep == start_id && !visited.is_empty() {
                    return true;
                }
                stack.push(dep.clone());
            }
        }
    }
    false
}

// ── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
#[path = "schema.tests.rs"]
mod tests;
