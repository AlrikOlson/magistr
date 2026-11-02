use super::step::Step;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

/// The current run and step instances, validated again under the owner lock.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct ExecutionIdentity {
    pub run_id: String,
    pub steps: BTreeMap<String, String>,
}

impl ExecutionIdentity {
    pub(crate) fn fresh() -> Self {
        Self {
            run_id: uuid::Uuid::new_v4().to_string(),
            steps: BTreeMap::new(),
        }
    }

    pub(crate) fn has_step_instances(&self, steps: &[Step]) -> bool {
        let ids: BTreeSet<_> = steps.iter().map(|step| step.id.as_str()).collect();
        let instances: BTreeSet<_> = self.steps.values().collect();
        !self.run_id.is_empty()
            && ids.len() == steps.len()
            && self.steps.len() == steps.len()
            && instances.len() == self.steps.len()
            && steps.iter().all(|step| {
                self.steps
                    .get(&step.id)
                    .is_some_and(|instance| !instance.is_empty())
            })
    }

    pub(crate) fn add_step(&mut self, id: &str) {
        self.steps
            .insert(id.to_owned(), uuid::Uuid::new_v4().to_string());
    }

    pub(crate) fn remove_step(&mut self, id: &str) {
        self.steps.remove(id);
    }
}
