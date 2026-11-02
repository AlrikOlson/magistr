use std::collections::BTreeMap;

use magistr_workflow::plan::domain::{
    FeatureCatalog, Task, TaskStatus, feature_catalog, next_ready,
};
use serde::{Deserialize, Serialize};

use super::progress::progress_from_tasks;
use crate::plan::PlanProgress;

/// What the engine surfaces about a magistr plan: status counts,
/// the next ready task, what is in flight, and the canonical current and
/// historical features. Small on purpose — a view model, not the plan.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct NativePlanSnapshot {
    pub project_id: String,
    /// Task counts keyed by status wire name (`pending`, `done`, …).
    pub counts: BTreeMap<String, u32>,
    /// The task `plan_next` would return.
    pub next_ready: Option<String>,
    /// Ids currently `in_progress`.
    pub in_progress: Vec<String>,
    /// Current work, authored history and ungrouped work from the plan owner.
    pub features: FeatureCatalog,
    pub progress: PlanProgress,
}

impl NativePlanSnapshot {
    /// Project one plan observation using the owner's readiness rules.
    #[must_use]
    pub fn from_tasks(project_id: &str, tasks: &[Task]) -> Self {
        let mut counts: BTreeMap<String, u32> = BTreeMap::new();
        let mut in_progress = Vec::new();
        for task in tasks {
            *counts
                .entry(TaskStatus::as_wire(task.status).to_owned())
                .or_insert(0) += 1;
            if task.status == TaskStatus::Retired {
                continue;
            }
            if task.status == TaskStatus::InProgress {
                in_progress.push(task.id.clone());
            }
        }
        Self {
            project_id: project_id.to_owned(),
            counts,
            next_ready: next_ready(
                tasks,
                |task| {
                    task.readiness(|id| tasks.iter().find(|c| c.id == id).map(|c| c.status))
                        .is_ready()
                },
                |task| task.priority,
            )
            .map(|task| task.id.clone()),
            in_progress,
            features: feature_catalog(tasks),
            progress: progress_from_tasks(tasks),
        }
    }

    /// Count for one status wire name.
    #[must_use]
    pub fn count(&self, status: &str) -> u32 {
        self.counts.get(status).copied().unwrap_or(0)
    }
}

#[cfg(test)]
#[path = "snapshot.tests.rs"]
mod tests;
