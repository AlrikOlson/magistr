//! Project context — immutable value object for project state snapshots.
//!
//! `ProjectContext` captures a point-in-time snapshot of the project's state:
//! recent commits, spec progress, working tree status, and active plan/task.
//! It is backend-agnostic — no filesystem, no git, no IO.
//!
//! Infrastructure adapters (like `ContextBuilder` in magistr-cli) populate
//! `ProjectContext` by shelling out to git and reading the filesystem.

use serde::{Deserialize, Serialize};

// ── Project context ─────────────────────────────────────────────────────────

/// Immutable snapshot of project state for context injection.
///
/// Used by the engine to inform workflow decisions and by the protocol
/// layer to inject context into agent sessions. Refreshed on demand —
/// each instance is a frozen point-in-time view.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProjectContext {
    /// Recent git commits (short log, most recent first).
    pub recent_commits: Vec<String>,

    /// Total items in the spec file.
    pub spec_total: u32,
    /// Checked items in the spec file.
    pub spec_checked: u32,
    /// Unchecked items in the spec file.
    pub spec_unchecked: u32,

    /// Files with uncommitted changes (working tree + staged).
    pub dirty_files: Vec<String>,

    /// Active plan content (from `.magistr/current-plan.md`), if any.
    pub active_plan: Option<String>,

    /// Active task name (from `.magistr/current-task.md`), if any.
    pub active_task: Option<String>,
}

impl ProjectContext {
    /// Create a new project context with all fields specified.
    #[must_use]
    pub fn new(
        recent_commits: Vec<String>,
        spec_total: u32,
        spec_checked: u32,
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

    /// Create an empty context (no project state available).
    #[must_use]
    pub fn empty() -> Self {
        Self {
            recent_commits: Vec::new(),
            spec_total: 0,
            spec_checked: 0,
            spec_unchecked: 0,
            dirty_files: Vec::new(),
            active_plan: None,
            active_task: None,
        }
    }

    /// Spec completion fraction (0.0–1.0).
    #[must_use]
    pub fn spec_completion(&self) -> f64 {
        if self.spec_total == 0 {
            0.0
        } else {
            f64::from(self.spec_checked) / f64::from(self.spec_total)
        }
    }

    /// Whether the spec is fully complete.
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
