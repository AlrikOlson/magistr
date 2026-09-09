//! Where one planned task stands, as `ship_status` and the ship frames name it.

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
    /// The state named `name`; anything unrecognised reads as `Planned`.
    #[must_use]
    pub fn parse(name: &str) -> Self {
        match name {
            "active" => Self::Active,
            "completed" => Self::Completed,
            "blocked" => Self::Blocked,
            "skipped" => Self::Skipped,
            _ => Self::Planned,
        }
    }

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

#[cfg(test)]
#[path = "task_state.tests.rs"]
mod tests;
