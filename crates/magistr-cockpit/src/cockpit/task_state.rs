//! Where one planned task stands, as the ship records and events name it.

use magistr_engine::native::TaskStatus;

use crate::theme::Icons;

/// The lifecycle of a task on the ship plan.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum TaskState {
    /// In the plan, not started.
    #[default]
    Planned,
    /// Being worked right now.
    Active,
    /// Done.
    Completed,
    /// Blocked on something.
    Blocked,
    /// Skipped by the agent.
    Skipped,
}

impl TaskState {
    /// The one-cell mark the rail draws before the task's title, in
    /// `icons`' vocabulary.
    #[must_use]
    pub const fn glyph(self, icons: &Icons) -> &'static str {
        match self {
            Self::Planned => icons.planned,
            Self::Active => icons.active,
            Self::Completed => icons.done,
            Self::Blocked => icons.blocked,
            Self::Skipped => icons.skipped,
        }
    }
}

impl From<TaskStatus> for TaskState {
    fn from(status: TaskStatus) -> Self {
        match status {
            TaskStatus::Planned => Self::Planned,
            TaskStatus::Active => Self::Active,
            TaskStatus::Completed => Self::Completed,
            TaskStatus::Blocked => Self::Blocked,
            TaskStatus::Skipped => Self::Skipped,
        }
    }
}

#[cfg(test)]
#[path = "task_state.tests.rs"]
mod tests;
