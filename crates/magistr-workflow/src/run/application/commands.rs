use magistr_workflow_contracts::Observed;
use std::sync::{Arc, Mutex};

use crate::run::domain::{
    artifact::Artifact,
    goal::VerificationMode,
    progress::{Progress, ProgressType},
    step::{Step, StepType},
};
use crate::run::engine::RunEngine;

use super::{
    GoalSet, PlanChange, PlanSnapshot, RunError, RunReport, RunSnapshot, RunStatus, RunUseCases,
};

impl RunUseCases {
    pub fn change_plan(&self, change: PlanChange) -> Result<Observed<PlanSnapshot>, RunError> {
        let mut engine = self.lock()?;
        let committed = match change {
            PlanChange::Add {
                id,
                title,
                step_type,
                estimate,
                think_branch,
            } => {
                let title = title.unwrap_or_else(|| id.clone());
                engine.plan_step(
                    id,
                    title,
                    step_type.unwrap_or(StepType::Implement),
                    estimate,
                    think_branch,
                )?
            }
            PlanChange::Remove { id } => engine.remove_step(&id)?,
            PlanChange::Reorder { id, after } => engine.reorder_step(&id, after.as_deref())?,
            PlanChange::Unknown => {
                return Err(RunError::invalid_args(
                    "progress must be one of: add, remove, reorder",
                ));
            }
        };
        Ok(committed.map(|()| PlanSnapshot::new(&engine.steps)))
    }

    pub fn engine(&self) -> Arc<Mutex<RunEngine>> {
        Arc::clone(&self.engine)
    }

    pub fn set_goal(
        &self,
        description: String,
        acceptance_criteria: Vec<String>,
        constraints: Vec<String>,
        scope: String,
    ) -> Result<Observed<GoalSet>, RunError> {
        self.set_goal_with_verification(
            description,
            acceptance_criteria,
            constraints,
            scope,
            VerificationMode::Focused,
        )
    }

    pub fn set_goal_with_verification(
        &self,
        description: String,
        acceptance_criteria: Vec<String>,
        constraints: Vec<String>,
        scope: String,
        verification: VerificationMode,
    ) -> Result<Observed<GoalSet>, RunError> {
        let mut engine = self.lock()?;
        let cleared = engine.set_goal_with_verification(
            description,
            acceptance_criteria,
            constraints,
            scope,
            verification,
        )?;
        let goal = engine.goal.clone().expect("set goal");
        Ok(cleared.map(|cleared| GoalSet::new(goal, cleared)))
    }

    pub fn add_step(
        &self,
        id: String,
        title: String,
        step_type: StepType,
        estimate: Option<String>,
        think_branch: Option<String>,
    ) -> Result<Observed<()>, RunError> {
        self.lock()?
            .plan_step(id, title, step_type, estimate, think_branch)
    }

    pub fn remove_step(&self, step_id: &str) -> Result<Observed<()>, RunError> {
        self.lock()?.remove_step(step_id)
    }

    pub fn reorder_step(
        &self,
        step_id: &str,
        after: Option<&str>,
    ) -> Result<Observed<()>, RunError> {
        self.lock()?.reorder_step(step_id, after)
    }

    pub fn plan(&self) -> Result<PlanSnapshot, RunError> {
        Ok(PlanSnapshot::new(&self.lock()?.steps))
    }

    pub fn start_step(&self, step_id: &str) -> Result<Observed<Step>, RunError> {
        self.lock()?.start_step(step_id)
    }

    #[allow(clippy::too_many_arguments)]
    pub fn record_progress(
        &self,
        step_id: Option<&str>,
        progress_type: ProgressType,
        description: String,
        files_touched: Vec<String>,
        tools_used: Vec<String>,
        result: String,
        think_step: Option<u32>,
    ) -> Result<Observed<Progress>, RunError> {
        self.lock()?.record_progress(
            step_id,
            progress_type,
            description,
            files_touched,
            tools_used,
            result,
            think_step,
        )
    }

    pub fn complete_step(
        &self,
        step_id: &str,
        artifacts: Vec<Artifact>,
    ) -> Result<Observed<Step>, RunError> {
        self.lock()?.complete_step(step_id, artifacts)
    }

    pub fn block_step(&self, step_id: &str, reason: String) -> Result<Observed<Step>, RunError> {
        self.lock()?.block_step(step_id, reason)
    }

    pub fn finalize(
        &self,
        artifacts: Vec<Artifact>,
        summary: Option<String>,
    ) -> Result<Observed<RunReport>, RunError> {
        self.lock()?.finalize(artifacts, summary)
    }

    pub fn snapshot(&self) -> Result<RunSnapshot, RunError> {
        Ok(self.lock()?.snapshot())
    }

    pub fn status(&self) -> Result<RunStatus, RunError> {
        Ok(self.snapshot()?.status())
    }

    pub fn export(&self, format: &str) -> Result<String, RunError> {
        Ok(self.lock()?.export(format))
    }

    pub fn reset(&self) -> Result<Observed<()>, RunError> {
        self.lock()?.reset()
    }
}
