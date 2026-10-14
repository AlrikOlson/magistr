use std::collections::{BTreeMap, BTreeSet};

use super::task::Task;
use serde::{Deserialize, Serialize};

/// The recorded run and task instances in the current Ship persistence record.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct ExecutionIdentity {
    pub run_id: String,
    pub tasks: BTreeMap<String, String>,
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

    pub(crate) fn has_task_instances(&self, tasks: &[Task]) -> bool {
        let ids: BTreeSet<_> = tasks.iter().map(|task| task.id.as_str()).collect();
        !self.run_id.is_empty()
            && ids.len() == tasks.len()
            && self.tasks.len() == tasks.len()
            && tasks.iter().all(|task| {
                self.tasks.get(&task.id).is_some_and(|instance| {
                    !instance.is_empty() && !self.removed_instances.contains(instance)
                })
            })
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
