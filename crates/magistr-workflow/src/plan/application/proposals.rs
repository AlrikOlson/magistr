use crate::plan::domain::{Task, TaskStatus};
use magistr_workflow_contracts::Observed;

use super::{PlanError, PlanUseCases};

impl PlanUseCases {
    pub fn propose_title(
        &self,
        id: &str,
        title: String,
        reason: String,
        source: String,
    ) -> Result<Observed<Task>, PlanError> {
        self.lock()?
            .propose_title(id, title, reason, source)
            .map_err(PlanError::invalid_state)
    }

    pub fn propose_status(
        &self,
        id: &str,
        status: TaskStatus,
        reason: String,
        source: String,
    ) -> Result<Observed<Task>, PlanError> {
        self.lock()?
            .propose_status(id, status, reason, source)
            .map_err(PlanError::invalid_state)
    }

    pub fn propose_reprioritize(
        &self,
        id: &str,
        priority: u32,
        reason: String,
    ) -> Result<Observed<Task>, PlanError> {
        self.lock()?
            .propose_reprioritize(id, priority, reason)
            .map_err(PlanError::invalid_state)
    }

    pub fn resolve_title_proposal(
        &self,
        id: &str,
        accept: bool,
    ) -> Result<Observed<Task>, PlanError> {
        self.lock()?
            .resolve_title_proposal(id, accept)
            .map_err(PlanError::invalid_state)
    }

    pub fn resolve_status_proposal(
        &self,
        id: &str,
        accept: bool,
    ) -> Result<Observed<Task>, PlanError> {
        self.lock()?
            .mutate(|edit| {
                if accept
                    && edit
                        .plan()
                        .tasks
                        .iter()
                        .find(|task| task.id == id)
                        .and_then(|task| task.status_proposal.as_ref())
                        .is_some_and(|proposal| proposal.suggested_status == TaskStatus::Done)
                {
                    return Err(Self::refuse_direct_done(id).to_string());
                }
                edit.resolve_status_proposal(id, accept).cloned()
            })
            .map_err(PlanError::invalid_state)
    }

    pub fn resolve_reprioritize(
        &self,
        id: &str,
        accept: bool,
    ) -> Result<Observed<Task>, PlanError> {
        self.lock()?
            .resolve_reprioritize(id, accept)
            .map_err(PlanError::invalid_state)
    }
}
