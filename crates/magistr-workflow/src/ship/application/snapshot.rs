use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::ship::domain::{
    objective::Objective,
    task::{Task, TaskStatus},
};
use crate::ship::engine::ShipEngine;

use super::{ShipStatus, TaskAdmission};

/// Native execution state with stable run and task-instance identity.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShipSnapshot {
    pub project_id: String,
    pub execution: Option<super::ExecutionBinding>,
    pub run_id: String,
    pub objective: Option<Objective>,
    pub tasks: Vec<Task>,
    task_instances: BTreeMap<String, String>,
}

impl ShipSnapshot {
    pub(crate) fn from_engine(engine: &ShipEngine) -> Self {
        Self {
            project_id: engine.project_id.clone(),
            execution: engine.binding.clone(),
            run_id: engine.execution.run_id.clone(),
            objective: engine.objective.clone(),
            tasks: engine.tasks.clone(),
            task_instances: engine.execution.tasks.clone(),
        }
    }

    pub fn active_task(&self) -> Option<&Task> {
        self.tasks
            .iter()
            .find(|task| task.status == TaskStatus::Active)
    }

    pub fn status(&self) -> ShipStatus {
        ShipStatus::from_snapshot(self)
    }

    pub fn admit_task(&self, explicit: Option<&str>) -> Result<TaskAdmission, String> {
        let task_id = explicit
            .map(str::to_owned)
            .or_else(|| self.active_task().map(|task| task.id.clone()))
            .ok_or_else(|| "no task_id provided and no active task".to_owned())?;
        let matches = self.tasks.iter().filter(|task| task.id == task_id).count();
        if matches == 0 {
            return Err(format!("task '{task_id}' not found"));
        }
        if matches != 1 {
            return Err(format!(
                "task '{task_id}' is ambiguous; task admission requires a unique task id"
            ));
        }
        let task_instance = self
            .task_instances
            .get(&task_id)
            .ok_or_else(|| format!("task '{task_id}' has no execution identity"))?
            .clone();
        Ok(TaskAdmission {
            project_id: self.project_id.clone(),
            execution: self.execution.clone(),
            run_id: self.run_id.clone(),
            task_id,
            task_instance,
        })
    }

    pub(crate) fn contains(&self, target: &TaskAdmission) -> bool {
        self.admit_task(Some(&target.task_id))
            .is_ok_and(|current| current == *target)
    }
}
