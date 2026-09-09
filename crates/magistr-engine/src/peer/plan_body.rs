use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};

use super::{AgreementError, PlanContext, PlanTask, RunContract, WorkerModel};

/// Consent-bearing work and policy. Editing any field requires a new revision.
/// Vector order is deliberate and participates in the digest, including task order.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PlanBody {
    pub scope: Vec<String>,
    pub context: PlanContext,
    pub tasks: Vec<PlanTask>,
    pub worker_models: Vec<WorkerModel>,
    pub routing_policy: String,
    pub retry_allowance: u32,
    pub acceptance: Vec<String>,
}

impl PlanBody {
    pub(super) fn validate(&self, contract: &RunContract) -> Result<(), AgreementError> {
        super::require_list("scope", &self.scope)?;
        super::require_list("acceptance", &self.acceptance)?;
        super::require_text("roadmap_revision", &self.context.roadmap_revision)?;
        super::require_text("code_revision", &self.context.code_revision)?;
        super::require_text(
            "routing_history_revision",
            &self.context.routing_history_revision,
        )?;
        for constraint in &self.context.constraints {
            super::require_text("constraint", constraint)?;
        }
        super::require_text("routing_policy", &self.routing_policy)?;
        if self.worker_models.is_empty()
            || self.worker_models.iter().collect::<BTreeSet<_>>().len() != self.worker_models.len()
            || self
                .worker_models
                .iter()
                .any(|m| !contract.allowed_worker_models.contains(m))
        {
            return Err(AgreementError::Invalid(
                "plan workers must be unique and allowed by the run contract".into(),
            ));
        }
        if self.retry_allowance > contract.limits.max_retries_per_task {
            return Err(AgreementError::Invalid(
                "plan retries exceed the run contract".into(),
            ));
        }
        if self.tasks.is_empty() || self.tasks.len() > contract.limits.max_attempts as usize {
            return Err(AgreementError::Invalid(
                "task count must fit the run attempt ceiling".into(),
            ));
        }
        self.validate_tasks()
    }

    fn validate_tasks(&self) -> Result<(), AgreementError> {
        let mut tasks = BTreeMap::new();
        for task in &self.tasks {
            super::require_text("task id", &task.id)?;
            super::require_text("task description", &task.description)?;
            super::require_list("task acceptance", &task.acceptance)?;
            for risk in &task.risks {
                super::require_text("risk", risk)?;
            }
            if tasks.insert(task.id.as_str(), task).is_some() {
                return Err(AgreementError::Invalid(format!(
                    "duplicate task {}",
                    task.id
                )));
            }
        }
        for task in &self.tasks {
            let mut seen = BTreeSet::new();
            for dependency in &task.dependencies {
                if !tasks.contains_key(dependency.as_str()) || !seen.insert(dependency) {
                    return Err(AgreementError::Invalid(format!(
                        "unknown or duplicate dependency for {}",
                        task.id
                    )));
                }
            }
        }
        // Iterative topological elimination avoids a recursive call per task.
        let mut resolved = BTreeSet::new();
        while resolved.len() < tasks.len() {
            let before = resolved.len();
            for task in &self.tasks {
                if task
                    .dependencies
                    .iter()
                    .all(|d| resolved.contains(d.as_str()))
                {
                    resolved.insert(task.id.as_str());
                }
            }
            if resolved.len() == before {
                return Err(AgreementError::Invalid(
                    "task dependencies contain a cycle".into(),
                ));
            }
        }
        Ok(())
    }
}
