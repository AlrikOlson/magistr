//! Scheduling policy is shared; the writer supplies current records and names.
use super::{PlanEngine, Task};
use magistr_workflow_contracts::plan::Readiness;

impl PlanEngine {
    pub(super) fn readiness<'a>(&self, task: &'a Task) -> Readiness<'a> {
        task.readiness(|id| {
            self.plan
                .tasks
                .iter()
                .find(|c| c.id == id)
                .map(|c| c.status)
        })
    }
}
