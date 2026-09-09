//! Project state supplied by an adapter, without filesystem or Git access.

use alloc::string::String;
use alloc::vec::Vec;
use serde::{Deserialize, Serialize};

/// A point-in-time project snapshot for context injection.
///
/// Adapters gather the observations; this value neither reads project state
/// nor establishes an active execution cycle. Counts use `usize`, matching
/// collection and spec-parser counts without narrowing at the core boundary.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProjectContext {
    /// Recent Git commits (short log, most recent first).
    pub recent_commits: Vec<String>,
    /// Total items in the spec or native roadmap.
    pub spec_total: usize,
    /// Checked items in the spec or native roadmap.
    pub spec_checked: usize,
    /// Unchecked items in the spec or native roadmap.
    pub spec_unchecked: usize,
    /// Files with uncommitted changes (working tree and staged).
    pub dirty_files: Vec<String>,
    /// Active plan content, if supplied by the adapter.
    pub active_plan: Option<String>,
    /// Active task name, if supplied by the adapter.
    pub active_task: Option<String>,
}

impl ProjectContext {
    /// Create a context, deriving remaining items from total and checked counts.
    #[must_use]
    pub fn new(
        recent_commits: Vec<String>,
        spec_total: usize,
        spec_checked: usize,
        dirty_files: Vec<String>,
        active_plan: Option<String>,
        active_task: Option<String>,
    ) -> Self {
        Self {
            recent_commits,
            spec_total,
            spec_checked,
            spec_unchecked: spec_total.saturating_sub(spec_checked),
            dirty_files,
            active_plan,
            active_task,
        }
    }

    /// Create an empty context when no observations are available.
    #[must_use]
    pub fn empty() -> Self {
        Self::new(Vec::new(), 0, 0, Vec::new(), None, None)
    }

    /// Observed checked-to-total ratio, or zero for an empty spec.
    ///
    /// As with the historical engine snapshot, over-counted checked items may
    /// produce a ratio above one; this does not discard supplied observations.
    #[must_use]
    pub fn spec_completion(&self) -> f64 {
        if self.spec_total == 0 {
            0.0
        } else {
            self.spec_checked as f64 / self.spec_total as f64
        }
    }

    /// Whether the nonempty spec is fully complete.
    #[must_use]
    pub fn is_spec_complete(&self) -> bool {
        self.spec_total > 0 && self.spec_checked >= self.spec_total
    }

    /// Whether there are uncommitted changes.
    #[must_use]
    pub fn has_dirty_files(&self) -> bool {
        !self.dirty_files.is_empty()
    }

    /// Whether there is an active plan.
    #[must_use]
    pub fn has_active_plan(&self) -> bool {
        self.active_plan.is_some()
    }

    /// Whether there is an active task.
    #[must_use]
    pub fn has_active_task(&self) -> bool {
        self.active_task.is_some()
    }

    /// Most recent commit message, if any.
    #[must_use]
    pub fn latest_commit(&self) -> Option<&str> {
        self.recent_commits.first().map(String::as_str)
    }
}

#[cfg(test)]
#[path = "project.tests.rs"]
mod tests;
