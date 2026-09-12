use serde::Serialize;

use crate::ship::domain::task::{Task, TaskStatus, TaskType};

/// Typed plan view; its serialization preserves the MCP plan response.
#[derive(Debug, Clone, Serialize)]
pub struct PlanSnapshot {
    tasks: Vec<PlanTask>,
    total: usize,
}

impl PlanSnapshot {
    pub(crate) fn new(tasks: &[Task]) -> Self {
        Self {
            tasks: tasks
                .iter()
                .map(|task| PlanTask {
                    id: task.id.clone(),
                    title: task.title.clone(),
                    task_type: task.task_type.clone(),
                    status: task.status,
                    estimate: task.estimate.clone(),
                })
                .collect(),
            total: tasks.len(),
        }
    }
}

#[derive(Debug, Clone, Serialize)]
struct PlanTask {
    id: String,
    title: String,
    #[serde(rename = "type")]
    task_type: TaskType,
    status: TaskStatus,
    estimate: Option<String>,
}
