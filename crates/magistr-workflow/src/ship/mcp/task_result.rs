//! A task as the verbs that move one return it.

use serde::Serialize;

use crate::ship::domain::task::Task;

/// The task a verb just moved, with the counts its `outputSchema`
/// (`TaskOutput`) requires. A client that validates `structuredContent`
/// refuses a bare [`Task`], so a task that started read as a call that failed.
#[derive(Serialize)]
pub(super) struct TaskResult {
    #[serde(flatten)]
    task: Task,
    actions_count: usize,
    checks_count: usize,
    artifacts_count: usize,
}

impl From<Task> for TaskResult {
    fn from(task: Task) -> Self {
        Self {
            actions_count: task.actions.len(),
            checks_count: task.checks.len(),
            artifacts_count: task.artifacts.len(),
            task,
        }
    }
}

#[cfg(test)]
#[path = "task_result.tests.rs"]
mod tests;
