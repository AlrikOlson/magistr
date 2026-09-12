//! One entry of the ship timeline.

use magistr_engine::native::{Action, Check};

/// One moment on the ship half of the Trace tab: an action the agent
/// recorded, or a quality-gate check with its exit code and duration —
/// read off the store's own [`Action`] and [`Check`] records, from a
/// snapshot's tasks or the ship family's `action_recorded` and
/// `check_recorded` events.
#[derive(Clone, Debug, PartialEq)]
pub enum ShipMoment {
    /// An action the agent recorded under a task.
    Action {
        /// The task the action was recorded under.
        task_id: String,
        /// The action's type, in the store's spelling (`code`, `test`,
        /// `research`, …).
        kind: String,
        /// What was done.
        description: String,
        /// The action's ordinal on the ship ledger (`#198`) — how a
        /// receipt is named.
        id: u64,
        /// The files the action touched, as recorded.
        files: Vec<String>,
    },
    /// One quality gate and how it went.
    Check {
        /// The gate's name.
        name: String,
        /// Whether it passed.
        passed: bool,
        /// The command's exit code, when one was recorded.
        exit_code: Option<i64>,
        /// The runner-reported duration, when the check carried a
        /// parsed report.
        duration_ms: Option<u64>,
    },
}

impl ShipMoment {
    /// The moment for an action recorded under `task_id`.
    #[must_use]
    pub fn action(task_id: &str, action: &Action) -> Self {
        Self::Action {
            task_id: task_id.to_owned(),
            kind: action.action_type.as_wire().to_owned(),
            description: action.description.clone(),
            id: u64::from(action.id),
            files: action.files_touched.clone(),
        }
    }

    /// The moment for a recorded check.
    #[must_use]
    pub fn check(check: &Check) -> Self {
        Self::Check {
            name: check.name.clone(),
            passed: check.passed,
            exit_code: check.exit_code.map(i64::from),
            duration_ms: check.results.as_ref().and_then(|r| r.duration_ms),
        }
    }
}

#[cfg(test)]
#[path = "ship_moment.tests.rs"]
mod tests;
