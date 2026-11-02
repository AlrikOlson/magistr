use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::run::domain::{
    goal::Goal,
    step::{Step, StepStatus},
};
use crate::run::engine::RunEngine;

use super::{RunStatus, StepAdmission};

/// Native execution state with stable run and step-instance identity.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RunSnapshot {
    pub project_id: String,
    pub execution: Option<super::ExecutionBinding>,
    pub run_id: String,
    pub goal: Option<Goal>,
    pub steps: Vec<Step>,
    step_instances: BTreeMap<String, String>,
}

impl RunSnapshot {
    pub(crate) fn from_engine(engine: &RunEngine) -> Self {
        Self {
            project_id: engine.project_id.clone(),
            execution: engine.binding.clone(),
            run_id: engine.execution.run_id.clone(),
            goal: engine.goal.clone(),
            steps: engine.steps.clone(),
            step_instances: engine.execution.steps.clone(),
        }
    }

    pub fn active_step(&self) -> Option<&Step> {
        self.steps
            .iter()
            .find(|step| step.status == StepStatus::Active)
    }

    pub fn status(&self) -> RunStatus {
        RunStatus::from_snapshot(self)
    }

    pub fn admit_step(&self, explicit: Option<&str>) -> Result<StepAdmission, String> {
        let step_id = explicit
            .map(str::to_owned)
            .or_else(|| self.active_step().map(|step| step.id.clone()))
            .ok_or_else(|| "no step_id provided and no active step".to_owned())?;
        let matches = self.steps.iter().filter(|step| step.id == step_id).count();
        if matches == 0 {
            return Err(format!("step '{step_id}' not found"));
        }
        if matches != 1 {
            return Err(format!(
                "step '{step_id}' is ambiguous; step admission requires a unique step id"
            ));
        }
        let step_instance = self
            .step_instances
            .get(&step_id)
            .ok_or_else(|| format!("step '{step_id}' has no execution identity"))?
            .clone();
        Ok(StepAdmission {
            project_id: self.project_id.clone(),
            execution: self.execution.clone(),
            run_id: self.run_id.clone(),
            step_id,
            step_instance,
        })
    }
}
