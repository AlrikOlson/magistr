//! Assembled project context for system prompt injection.

/// Assembled project context for system prompt injection.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProjectContext {
    /// Recent git commits (short log).
    pub recent_commits: Vec<String>,
    /// Spec file progress.
    pub spec_total: usize,
    pub spec_checked: usize,
    pub spec_unchecked: usize,
    /// Files with uncommitted changes.
    pub dirty_files: Vec<String>,
    /// Active plan content (from `.magistr/current-plan.md`).
    pub active_plan: Option<String>,
    /// Active task name (from `.magistr/current-task.md`).
    pub active_task: Option<String>,
}
