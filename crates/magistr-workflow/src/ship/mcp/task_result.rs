//! Compact mutation receipt; full task history stays behind the context query.
use crate::ship::domain::task::Task;
use serde::Serialize;
#[derive(Serialize)]
pub(super) struct TaskResult {
    #[serde(flatten)]
    summary: crate::ship::output_schemas::TaskOutput,
}
impl From<Task> for TaskResult {
    fn from(task: Task) -> Self {
        Self {
            summary: crate::ship::output_schemas::TaskOutput {
                id: task.id,
                title: task.title,
                task_type: serde_json::to_value(task.task_type)
                    .unwrap_or_default()
                    .as_str()
                    .unwrap_or_default()
                    .into(),
                status: serde_json::to_value(task.status)
                    .unwrap_or_default()
                    .as_str()
                    .unwrap_or_default()
                    .into(),
                started_at: task.started_at,
                completed_at: task.completed_at,
                actions_count: task.actions.len(),
                checks_count: task.checks.len(),
                artifacts_count: task.artifacts.len(),
            },
        }
    }
}
#[cfg(test)]
#[path = "task_result.tests.rs"]
mod tests;
