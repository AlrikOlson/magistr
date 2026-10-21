use magistr_workflow_contracts::Observed;
use std::sync::{Arc, Mutex};

use crate::ship::domain::{
    action::{Action, ActionType},
    artifact::Artifact,
    objective::VerificationMode,
    task::{Task, TaskType},
};
use crate::ship::engine::ShipEngine;

use super::{
    ObjectiveSet, PlanChange, PlanSnapshot, ShipError, ShipReport, ShipSnapshot, ShipStatus,
    ShipUseCases,
};

impl ShipUseCases {
    pub fn change_plan(&self, change: PlanChange) -> Result<Observed<PlanSnapshot>, ShipError> {
        let mut engine = self.lock()?;
        let committed = match change {
            PlanChange::Add {
                id,
                title,
                task_type,
                estimate,
                think_branch,
            } => {
                let title = title.unwrap_or_else(|| id.clone());
                engine.plan_task(
                    id,
                    title,
                    task_type.unwrap_or(TaskType::Implement),
                    estimate,
                    think_branch,
                )?
            }
            PlanChange::Remove { id } => engine.remove_task(&id)?,
            PlanChange::Reorder { id, after } => engine.reorder_task(&id, after.as_deref())?,
            PlanChange::Unknown => {
                return Err(ShipError::invalid_args(
                    "action must be one of: add, remove, reorder",
                ));
            }
        };
        Ok(committed.map(|()| PlanSnapshot::new(&engine.tasks)))
    }

    pub fn engine(&self) -> Arc<Mutex<ShipEngine>> {
        Arc::clone(&self.engine)
    }

    pub fn set_objective(
        &self,
        description: String,
        acceptance_criteria: Vec<String>,
        constraints: Vec<String>,
        scope: String,
    ) -> Result<Observed<ObjectiveSet>, ShipError> {
        self.set_objective_with_verification(
            description,
            acceptance_criteria,
            constraints,
            scope,
            VerificationMode::Focused,
        )
    }

    pub fn set_objective_with_verification(
        &self,
        description: String,
        acceptance_criteria: Vec<String>,
        constraints: Vec<String>,
        scope: String,
        verification: VerificationMode,
    ) -> Result<Observed<ObjectiveSet>, ShipError> {
        let mut engine = self.lock()?;
        let cleared = engine.set_objective_with_verification(
            description,
            acceptance_criteria,
            constraints,
            scope,
            verification,
        )?;
        let objective = engine.objective.clone().expect("set objective");
        Ok(cleared.map(|cleared| ObjectiveSet::new(objective, cleared)))
    }

    pub fn add_task(
        &self,
        id: String,
        title: String,
        task_type: TaskType,
        estimate: Option<String>,
        think_branch: Option<String>,
    ) -> Result<Observed<()>, ShipError> {
        self.lock()?
            .plan_task(id, title, task_type, estimate, think_branch)
    }

    pub fn remove_task(&self, task_id: &str) -> Result<Observed<()>, ShipError> {
        self.lock()?.remove_task(task_id)
    }

    pub fn reorder_task(
        &self,
        task_id: &str,
        after: Option<&str>,
    ) -> Result<Observed<()>, ShipError> {
        self.lock()?.reorder_task(task_id, after)
    }

    pub fn plan(&self) -> Result<PlanSnapshot, ShipError> {
        Ok(PlanSnapshot::new(&self.lock()?.tasks))
    }

    pub fn start_task(&self, task_id: &str) -> Result<Observed<Task>, ShipError> {
        self.lock()?.start_task(task_id)
    }

    #[allow(clippy::too_many_arguments)]
    pub fn record_action(
        &self,
        task_id: Option<&str>,
        action_type: ActionType,
        description: String,
        files_touched: Vec<String>,
        tools_used: Vec<String>,
        result: String,
        think_step: Option<u32>,
    ) -> Result<Observed<Action>, ShipError> {
        self.lock()?.record_action(
            task_id,
            action_type,
            description,
            files_touched,
            tools_used,
            result,
            think_step,
        )
    }

    pub fn complete_task(
        &self,
        task_id: &str,
        artifacts: Vec<Artifact>,
    ) -> Result<Observed<Task>, ShipError> {
        self.lock()?.complete_task(task_id, artifacts)
    }

    pub fn block_task(&self, task_id: &str, reason: String) -> Result<Observed<Task>, ShipError> {
        self.lock()?.block_task(task_id, reason)
    }

    pub fn finalize(
        &self,
        artifacts: Vec<Artifact>,
        summary: Option<String>,
    ) -> Result<Observed<ShipReport>, ShipError> {
        self.lock()?.finalize(artifacts, summary)
    }

    pub fn snapshot(&self) -> Result<ShipSnapshot, ShipError> {
        Ok(self.lock()?.snapshot())
    }

    pub fn status(&self) -> Result<ShipStatus, ShipError> {
        Ok(self.snapshot()?.status())
    }

    pub fn export(&self, format: &str) -> Result<String, ShipError> {
        Ok(self.lock()?.export(format))
    }

    pub fn reset(&self) -> Result<Observed<()>, ShipError> {
        self.lock()?.reset()
    }
}
