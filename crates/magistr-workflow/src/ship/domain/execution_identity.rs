use super::task::Task;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

/// The current run and task instances, validated again under the owner lock.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct ExecutionIdentity {
    pub run_id: String,
    pub tasks: BTreeMap<String, String>,
}

impl ExecutionIdentity {
    pub(crate) fn fresh() -> Self {
        Self {
            run_id: uuid::Uuid::new_v4().to_string(),
            tasks: BTreeMap::new(),
        }
    }

    pub(crate) fn has_task_instances(&self, tasks: &[Task]) -> bool {
        let ids: BTreeSet<_> = tasks.iter().map(|task| task.id.as_str()).collect();
        let instances: BTreeSet<_> = self.tasks.values().collect();
        !self.run_id.is_empty()
            && ids.len() == tasks.len()
            && self.tasks.len() == tasks.len()
            && instances.len() == self.tasks.len()
            && tasks.iter().all(|task| {
                self.tasks
                    .get(&task.id)
                    .is_some_and(|instance| !instance.is_empty())
            })
    }

    pub(crate) fn add_task(&mut self, id: &str) {
        self.tasks
            .insert(id.to_owned(), uuid::Uuid::new_v4().to_string());
    }

    pub(crate) fn remove_task(&mut self, id: &str) {
        self.tasks.remove(id);
    }
}
