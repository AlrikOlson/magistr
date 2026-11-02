use super::{PlanError, PlanUseCases, TaskPatch};
use crate::plan::domain::Task;
use magistr_workflow_contracts::Observed;

impl PlanUseCases {
    /// Validate and apply the complete patch to one candidate under the store lock.
    pub fn update_task(&self, id: &str, patch: TaskPatch) -> Result<Observed<Task>, PlanError> {
        if patch.waiting_on.is_some() && patch.clear_waiting_on {
            return Err(PlanError::InvalidArgs(
                "waiting_on and clear_waiting_on contradict each other — send one".into(),
            ));
        }
        if let Some(content) = &patch.content {
            content.validate().map_err(PlanError::InvalidArgs)?;
        }
        let mut engine = self.lock()?;
        let blocker = patch
            .waiting_on
            .map(|blocker| {
                engine.validate_waiting_on(blocker.kind, blocker.reason, blocker.evidence)
            })
            .transpose()
            .map_err(PlanError::InvalidArgs)?;
        engine
            .mutate(|edit| {
                if let Some(accept) = patch.resolve_title_proposal {
                    edit.resolve_title_proposal(id, accept)?;
                }
                if let Some(name) = patch.name.as_deref() {
                    edit.set_name(id, name)?;
                }
                if let Some(blocker) = blocker {
                    edit.set_waiting_on(id, blocker)?;
                }
                if patch.clear_waiting_on {
                    edit.clear_waiting_on(id)?;
                }
                edit.update_task(
                    id,
                    patch.title,
                    patch.priority,
                    patch.description,
                    patch.acceptance,
                    patch.deps,
                    patch.content,
                )
                .cloned()
            })
            .map_err(PlanError::invalid_state)
    }
}
