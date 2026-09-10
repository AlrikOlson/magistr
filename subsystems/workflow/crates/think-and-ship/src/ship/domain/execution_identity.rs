use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};

use super::{objective::Objective, task::Task};

/// Additive identity metadata in the existing native Ship persistence record.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct ExecutionIdentity {
    pub run_id: String,
    pub tasks: BTreeMap<String, String>,
    #[serde(default)]
    pub removed_instances: BTreeSet<String>,
}

impl ExecutionIdentity {
    pub(crate) fn fresh() -> Self {
        Self {
            run_id: uuid::Uuid::new_v4().to_string(),
            tasks: BTreeMap::new(),
            removed_instances: BTreeSet::new(),
        }
    }

    pub(crate) fn legacy(project: &str, objective: &Option<Objective>, tasks: &[Task]) -> Self {
        let stamp = objective
            .as_ref()
            .and_then(|o| o.created_at.as_deref())
            .unwrap_or("unscoped");
        let run_id = format!("legacy:{project}:{stamp}");
        let tasks = tasks
            .iter()
            .enumerate()
            .map(|(index, task)| (task.id.clone(), format!("legacy:{index}:{}", task.id)))
            .collect();
        Self {
            run_id,
            tasks,
            removed_instances: BTreeSet::new(),
        }
    }

    pub(crate) fn add_task(&mut self, id: &str) {
        self.tasks
            .insert(id.to_owned(), uuid::Uuid::new_v4().to_string());
    }

    pub(crate) fn remove_task(&mut self, id: &str) {
        if let Some(instance) = self.tasks.remove(id) {
            self.removed_instances.insert(instance);
        }
    }
}
