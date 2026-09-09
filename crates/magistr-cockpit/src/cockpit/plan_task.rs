//! One task of the ship plan, as the rail lists it.

use super::task_state::TaskState;

/// One planned task: its id, its title, where it stands, and the quality
/// gates recorded on it so far.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct PlanTask {
    /// The plan's task id (`ship_plan`).
    pub id: String,
    /// The task's title.
    pub title: String,
    /// Where the task stands.
    pub state: TaskState,
    /// Quality gates recorded on this task that passed.
    pub checks_passed: u32,
    /// Quality gates recorded on this task that failed.
    pub checks_failed: u32,
}
