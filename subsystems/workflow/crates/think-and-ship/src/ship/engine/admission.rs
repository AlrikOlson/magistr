use chrono::Utc;

use crate::ship::application::{CheckAdmission, CheckOutcome, CheckSettlement};
use crate::ship::broadcast::BroadcastFrame;
use crate::ship::domain::check::Check;
use crate::ship::domain::execution_identity::ExecutionIdentity;
use crate::ship::persistence::PersistedState;

use super::ShipEngine;

impl ShipEngine {
    pub(crate) fn plan_task(
        &mut self,
        id: String,
        title: String,
        task_type: crate::ship::domain::task::TaskType,
        estimate: Option<String>,
        think_branch: Option<String>,
    ) -> Result<(), String> {
        if self.tasks.iter().any(|task| task.id == id) {
            return Err(format!("task '{id}' already exists"));
        }
        self.add_task(id, title, task_type, estimate, think_branch);
        Ok(())
    }

    /// Reload the current native store before admitting asynchronous work;
    /// another admitted host role may have advanced the execution run.
    pub(crate) fn refresh_current(&mut self) -> Result<(), String> {
        if let Some(persistence) = &self.persistence
            && persistence.enabled()
        {
            let state = persistence
                .current_state(&self.project_id)
                .map_err(|error| error.to_string())?;
            self.adopt_current(state);
        }
        Ok(())
    }

    pub(crate) fn settle_admitted_check(
        &mut self,
        admission: CheckAdmission,
        outcome: CheckOutcome,
    ) -> Result<CheckSettlement, String> {
        if admission.target.project_id != self.project_id {
            return Ok(CheckSettlement::Stale { admission, outcome });
        }
        let check = Check {
            check_type: admission.request.check_type.clone(),
            name: admission.request.name.clone(),
            passed: outcome.passed,
            details: outcome.details.clone(),
            required: admission.request.required,
            verified: outcome.verified,
            command: outcome.command.clone(),
            exit_code: outcome.exit_code,
            report: outcome.report.clone(),
            results: outcome.results.clone(),
            timestamp: Utc::now().to_rfc3339(),
        };
        let applied = if let Some(persistence) = &self.persistence
            && persistence.enabled()
        {
            let (current, applied) = persistence
                .settle_check(&admission.target, &check)
                .map_err(|error| error.to_string())?;
            self.adopt_current(current);
            applied
        } else if self.snapshot().contains(&admission.target) {
            let task = self
                .tasks
                .iter_mut()
                .find(|task| task.id == admission.target.task_id)
                .expect("admitted task exists");
            task.checks.push(check.clone());
            true
        } else {
            false
        };
        if !applied {
            return Ok(CheckSettlement::Stale { admission, outcome });
        }
        self.broadcast(BroadcastFrame::CheckRecorded {
            task_id: admission.target.task_id,
            check: check.clone(),
        });
        Ok(CheckSettlement::Applied { check })
    }

    fn adopt_current(&mut self, current: Option<PersistedState>) {
        match current {
            Some(state) => {
                self.execution = state.execution.unwrap_or_else(|| {
                    ExecutionIdentity::legacy(&self.project_id, &state.objective, &state.tasks)
                });
                self.objective = state.objective;
                self.tasks = state.tasks;
                self.next_action_id = state.next_action_id;
            }
            None if self.objective.is_some() || !self.tasks.is_empty() => {
                self.objective = None;
                self.tasks.clear();
                self.next_action_id = 1;
                self.execution = ExecutionIdentity::fresh();
            }
            None => {}
        }
    }
}
