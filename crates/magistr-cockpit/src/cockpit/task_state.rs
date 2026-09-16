//! Where one planned task stands, as the ship records and events name it.

use magistr_engine::native::TaskStatus;

use crate::pixel::Icon;
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
    /// The mark the rail draws before the task's title.
    #[must_use]
    pub const fn icon(self) -> Icon {
        match self {
            Self::Planned => Icon::Planned,
            Self::Active => Icon::InProgress,
            Self::Completed => Icon::Done,
            Self::Blocked => Icon::Blocked,
            Self::Skipped => Icon::Skipped,
        }
    }

    /// The one-cell mark the rail draws before the task's title, in
    /// `icons`' vocabulary.
    #[must_use]
    pub const fn glyph(self, icons: &Icons) -> &'static str {
        icons.of(self.icon())
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
