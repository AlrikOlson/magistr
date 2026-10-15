use super::ShipEngine;
use crate::ship::application::{ShipError, ShipReport};
use crate::ship::broadcast::BroadcastFrame;
use crate::ship::domain::{
    action::{Action, ActionType},
    artifact::Artifact,
    check::{Check, CheckType},
    execution_identity::ExecutionIdentity,
    objective::{Objective, ObjectiveStatus, VerificationMode},
    task::{Task, TaskStatus, TaskType},
};
use crate::ship::persistence::PersistedState;
use crate::ship::saved_cycle::SavedCycle;
use chrono::Utc;
use magistr_workflow_contracts::Observed;

impl ShipEngine {
    pub fn set_objective(
        &mut self,
        description: String,
        acceptance_criteria: Vec<String>,
        constraints: Vec<String>,
        scope: String,
    ) -> Result<Observed<usize>, ShipError> {
        self.set_objective_with_verification(
            description,
            acceptance_criteria,
            constraints,
            scope,
            VerificationMode::Full,
        )
    }

    pub fn set_objective_with_verification(
        &mut self,
        description: String,
        acceptance_criteria: Vec<String>,
        constraints: Vec<String>,
        scope: String,
        verification: VerificationMode,
    ) -> Result<Observed<usize>, ShipError> {
        let result = self.mutate(&[], |state| {
            Ok(state.set_objective_with_verification(
                description,
                acceptance_criteria,
                constraints,
                scope,
                verification,
            ))
        })?;
        self.broadcast(BroadcastFrame::ObjectiveSet {
            objective: self.objective.clone().expect("committed objective"),
        });
        Ok(result)
    }

    pub fn add_task(
        &mut self,
        id: String,
        title: String,
        task_type: TaskType,
        estimate: Option<String>,
        think_branch: Option<String>,
    ) -> Result<Observed<()>, ShipError> {
        let task_id = id.clone();
        let task_title = title.clone();
        let result = self.mutate(&[], |state| {
            state
                .add_task(id, title, task_type, estimate, think_branch)
                .map_err(Into::into)
        })?;
        self.broadcast(BroadcastFrame::TaskAdded {
            task_id,
            title: task_title,
        });
        Ok(result)
    }

    pub fn remove_task(&mut self, task_id: &str) -> Result<Observed<()>, ShipError> {
        self.mutate(&[task_id], |state| {
            state.remove_task(task_id).map_err(Into::into)
        })
    }

    pub fn reorder_task(
        &mut self,
        task_id: &str,
        after: Option<&str>,
    ) -> Result<Observed<()>, ShipError> {
        let mut targets = vec![task_id];
        targets.extend(after);
        self.mutate(&targets, |state| {
            state.reorder_task(task_id, after).map_err(Into::into)
        })
    }

    pub fn start_task(&mut self, task_id: &str) -> Result<Observed<Task>, ShipError> {
        let result = self.mutate(&[task_id], |state| {
            state.start_task(task_id).cloned().map_err(Into::into)
        })?;
        self.broadcast(BroadcastFrame::TaskStarted {
            task_id: task_id.into(),
        });
        Ok(result)
    }

    #[allow(clippy::too_many_arguments)]
    pub fn record_action(
        &mut self,
        task_id: Option<&str>,
        action_type: ActionType,
        description: String,
        files_touched: Vec<String>,
        tools_used: Vec<String>,
        result: String,
        think_step: Option<u32>,
    ) -> Result<Observed<Action>, ShipError> {
        let target = self.snapshot().admit_task(task_id)?.task_id;
        let result = self.mutate(&[&target], |state| {
            state
                .record_action(
                    Some(&target),
                    action_type,
                    description,
                    files_touched,
                    tools_used,
                    result,
                    think_step,
                )
                .cloned()
                .map_err(Into::into)
        })?;
        self.broadcast(BroadcastFrame::ActionRecorded {
            task_id: target,
            action: result.value.clone(),
        });
        Ok(result)
    }

    pub fn complete_task(
        &mut self,
        task_id: &str,
        artifacts: Vec<Artifact>,
    ) -> Result<Observed<Task>, ShipError> {
        let result = self.mutate(&[task_id], |state| {
            state
                .complete_task(task_id, artifacts)
                .cloned()
                .map_err(Into::into)
        })?;
        self.broadcast(BroadcastFrame::TaskCompleted {
            task: result.value.clone(),
        });
        Ok(result)
    }

    pub fn block_task(
        &mut self,
        task_id: &str,
        reason: String,
    ) -> Result<Observed<Task>, ShipError> {
        let result = self.mutate(&[task_id], |state| {
            state
                .block_task(task_id, reason.clone())
                .cloned()
                .map_err(Into::into)
        })?;
        self.broadcast(BroadcastFrame::TaskBlocked {
            task_id: task_id.into(),
            reason,
        });
        Ok(result)
    }

    pub fn record_check(
        &mut self,
        task_id: Option<&str>,
        check_type: CheckType,
        name: String,
        passed: bool,
        details: String,
        required: bool,
    ) -> Result<Observed<Check>, ShipError> {
        self.record_check_full(
            task_id, check_type, name, passed, details, required, false, None, None, None, None,
        )
    }

    #[allow(clippy::too_many_arguments)]
    pub fn record_check_full(
        &mut self,
        task_id: Option<&str>,
        check_type: CheckType,
        name: String,
        passed: bool,
        details: String,
        required: bool,
        verified: bool,
        command: Option<String>,
        exit_code: Option<i32>,
        report: Option<crate::ship::report::ReportRecord>,
        results: Option<crate::ship::report::TestResults>,
    ) -> Result<Observed<Check>, ShipError> {
        let target = self.snapshot().admit_task(task_id)?.task_id;
        let result = self.mutate(&[&target], |state| {
            state
                .record_check_full(
                    Some(&target),
                    check_type,
                    name,
                    passed,
                    details,
                    required,
                    verified,
                    command,
                    exit_code,
                    report,
                    results,
                )
                .cloned()
                .map_err(Into::into)
        })?;
        self.broadcast(BroadcastFrame::CheckRecorded {
            task_id: target,
            check: result.value.clone(),
        });
        Ok(result)
    }

    pub fn finalize(
        &mut self,
        artifacts: Vec<Artifact>,
        summary: Option<String>,
    ) -> Result<Observed<ShipReport>, ShipError> {
        let result = self.mutate(&[], |state| Ok(state.finalize(artifacts, summary)))?;
        self.broadcast(BroadcastFrame::ObjectiveShipped {
            warnings: result.value.warnings.clone(),
        });
        Ok(result)
    }

    pub fn reset(&mut self) -> Result<Observed<()>, ShipError> {
        let result = self.mutate(&[], |state| {
            *state = PersistedState::empty(state.project_id.clone());
            Ok(())
        })?;
        self.broadcast(BroadcastFrame::Cleared);
        Ok(result)
    }
}

impl PersistedState {
    fn advance_selection(&mut self) {
        self.selection_revision = self
            .selection_revision
            .saturating_add(1)
            .max(Utc::now().timestamp_micros() as u64);
    }

    pub fn set_objective_with_verification(
        &mut self,
        description: String,
        acceptance_criteria: Vec<String>,
        constraints: Vec<String>,
        scope: String,
        verification: VerificationMode,
    ) -> usize {
        let mut cleared = 0;
        if self.objective.is_none() {
            self.advance_selection();
        }
        let switching = self
            .objective
            .as_ref()
            .is_some_and(|objective| objective.scope != scope);
        if switching {
            if let Some(objective) = self.objective.take() {
                let tasks = std::mem::take(&mut self.tasks);
                if matches!(
                    objective.status,
                    ObjectiveStatus::Completed | ObjectiveStatus::Abandoned
                ) {
                    cleared = tasks.len();
                }
                self.saved_cycles.push(SavedCycle {
                    objective,
                    tasks,
                    next_action_id: self.next_action_id,
                    execution: self.execution.clone(),
                });
            }
            self.advance_selection();
            if let Some(index) = self.saved_cycles.iter().position(|cycle| {
                cycle.objective.scope == scope
                    && matches!(
                        cycle.objective.status,
                        ObjectiveStatus::Defined | ObjectiveStatus::Active
                    )
            }) {
                let saved = self.saved_cycles.remove(index);
                self.objective = Some(saved.objective);
                self.tasks = saved.tasks;
                self.next_action_id = saved.next_action_id;
                self.execution = saved.execution;
                return cleared;
            } else {
                self.next_action_id = 1;
                self.execution = ExecutionIdentity::fresh();
            }
        }
        let prior_finished = matches!(
            self.objective.as_ref().map(|o| &o.status),
            Some(ObjectiveStatus::Completed) | Some(ObjectiveStatus::Abandoned)
        );
        if prior_finished {
            cleared = self.tasks.len();
            self.advance_selection();
            self.saved_cycles.push(SavedCycle {
                objective: self.objective.take().expect("finished objective"),
                tasks: std::mem::take(&mut self.tasks),
                next_action_id: self.next_action_id,
                execution: self.execution.clone(),
            });
            self.next_action_id = 1;
            self.execution = ExecutionIdentity::fresh();
        }
        let created_at = self
            .objective
            .as_ref()
            .filter(|_| !prior_finished)
            .and_then(|objective| objective.created_at.clone())
            .unwrap_or_else(|| Utc::now().to_rfc3339());
        self.objective = Some(Objective {
            description,
            acceptance_criteria,
            constraints,
            scope,
            verification,
            status: self
                .objective
                .as_ref()
                .filter(|_| !prior_finished)
                .map_or(ObjectiveStatus::Defined, |objective| objective.status),
            project_id: self.project_id.clone(),
            created_at: Some(created_at),
            completed_at: None,
        });
        cleared
    }

    pub fn add_task(
        &mut self,
        id: String,
        title: String,
        task_type: TaskType,
        estimate: Option<String>,
        think_branch: Option<String>,
    ) -> Result<(), String> {
        if self.tasks.iter().any(|task| task.id == id) {
            return Err(format!("task '{id}' already exists"));
        }
        if let Some(obj) = &mut self.objective
            && obj.status == ObjectiveStatus::Defined
        {
            obj.status = ObjectiveStatus::Active;
        }
        self.execution.add_task(&id);
        self.tasks.push(Task {
            id,
            title,
            task_type,
            status: TaskStatus::Planned,
            estimate,
            started_at: None,
            completed_at: None,
            artifacts: Vec::new(),
            checks: Vec::new(),
            actions: Vec::new(),
            blocked_reason: None,
            think_branch,
        });
        Ok(())
    }

    pub fn remove_task(&mut self, task_id: &str) -> Result<(), String> {
        let idx = self.task_index(task_id)?;
        let status = &self.tasks[idx].status;
        if *status == TaskStatus::Active || *status == TaskStatus::Completed {
            return Err(format!(
                "cannot remove task '{task_id}' with status {status:?}"
            ));
        }
        self.tasks.remove(idx);
        self.execution.remove_task(task_id);
        Ok(())
    }

    pub fn reorder_task(&mut self, task_id: &str, after: Option<&str>) -> Result<(), String> {
        let idx = self.task_index(task_id)?;
        let destination = after.map(|id| self.task_index(id)).transpose()?;
        if destination == Some(idx) {
            return Err("A task cannot be placed after itself.".into());
        }
        let task = self.tasks.remove(idx);
        let insert_at = destination.map_or(0, |index| if index > idx { index } else { index + 1 });
        self.tasks.insert(insert_at, task);
        Ok(())
    }

    pub fn start_task(&mut self, task_id: &str) -> Result<&Task, String> {
        if let Some(active) = self.tasks.iter().find(|t| t.status == TaskStatus::Active) {
            return Err(format!(
                "task '{}' is already active — complete or block it first",
                active.id
            ));
        }
        let idx = self.task_index(task_id)?;
        let task = &mut self.tasks[idx];
        if task.status != TaskStatus::Planned && task.status != TaskStatus::Blocked {
            return Err(format!(
                "task '{task_id}' has status {:?}, cannot start",
                task.status
            ));
        }
        task.status = TaskStatus::Active;
        task.started_at = Some(Utc::now().to_rfc3339());
        task.blocked_reason = None;
        Ok(&self.tasks[idx])
    }

    #[allow(clippy::too_many_arguments)]
    pub fn record_action(
        &mut self,
        task_id: Option<&str>,
        action_type: ActionType,
        description: String,
        files_touched: Vec<String>,
        tools_used: Vec<String>,
        result: String,
        think_step: Option<u32>,
    ) -> Result<&Action, String> {
        let tid = self.resolve_task_id(task_id)?;
        let idx = self.task_index(&tid)?;
        let action_id = self.next_action_id;
        self.next_action_id = self
            .next_action_id
            .checked_add(1)
            .ok_or("The action count is exhausted.")?;
        let action = Action {
            id: action_id,
            task_id: tid,
            timestamp: Utc::now().to_rfc3339(),
            action_type,
            description,
            files_touched,
            tools_used,
            result,
            think_step,
        };
        self.tasks[idx].actions.push(action);
        Ok(self.tasks[idx].actions.last().unwrap())
    }

    pub fn complete_task(
        &mut self,
        task_id: &str,
        artifacts: Vec<Artifact>,
    ) -> Result<&Task, String> {
        let idx = self.task_index(task_id)?;
        let task = &mut self.tasks[idx];
        if task.status != TaskStatus::Active && task.status != TaskStatus::Blocked {
            return Err(format!(
                "task '{task_id}' has status {:?}, cannot complete",
                task.status
            ));
        }
        task.status = TaskStatus::Completed;
        task.completed_at = Some(Utc::now().to_rfc3339());
        task.blocked_reason = None;
        task.artifacts.extend(artifacts);
        Ok(&self.tasks[idx])
    }

    pub fn block_task(&mut self, task_id: &str, reason: String) -> Result<&Task, String> {
        let idx = self.task_index(task_id)?;
        let task = &mut self.tasks[idx];
        if task.status != TaskStatus::Active {
            return Err(format!(
                "task '{task_id}' has status {:?}, only active tasks can be blocked",
                task.status
            ));
        }
        task.status = TaskStatus::Blocked;
        task.blocked_reason = Some(reason);
        Ok(&self.tasks[idx])
    }

    #[allow(clippy::too_many_arguments)]
    pub fn record_check_full(
        &mut self,
        task_id: Option<&str>,
        check_type: CheckType,
        name: String,
        passed: bool,
        details: String,
        required: bool,
        verified: bool,
        command: Option<String>,
        exit_code: Option<i32>,
        report: Option<crate::ship::report::ReportRecord>,
        results: Option<crate::ship::report::TestResults>,
    ) -> Result<&Check, String> {
        let tid = self.resolve_task_id(task_id)?;
        let idx = self.task_index(&tid)?;
        let check = Check {
            receipt_id: None,
            check_type,
            name,
            passed,
            details,
            required,
            verified,
            command,
            exit_code,
            report,
            results,
            timestamp: Utc::now().to_rfc3339(),
        };
        self.tasks[idx].checks.push(check);
        Ok(self.tasks[idx].checks.last().unwrap())
    }

    pub fn finalize(
        &mut self,
        artifacts: Vec<Artifact>,
        summary: Option<String>,
    ) -> crate::ship::application::ShipReport {
        let mut warnings: Vec<String> = Vec::new();

        let total_tasks = self.tasks.len();
        let completed = self
            .tasks
            .iter()
            .filter(|t| t.status == TaskStatus::Completed)
            .count();
        let incomplete: Vec<&str> = self
            .tasks
            .iter()
            .filter(|t| t.status != TaskStatus::Completed && t.status != TaskStatus::Skipped)
            .map(|t| t.id.as_str())
            .collect();

        if !incomplete.is_empty() {
            warnings.push(format!(
                "{} task(s) not completed: {}",
                incomplete.len(),
                incomplete.join(", ")
            ));
        }

        // Required checks in recording order, latest per name last: a
        // failure a later record of the same name passed over is
        // recovered, one nothing passed over is outstanding.
        let mut latest_by_name: std::collections::BTreeMap<&str, (&Task, &Check)> =
            std::collections::BTreeMap::new();
        let mut required_in_order: Vec<(&Task, &Check)> = Vec::new();
        let mut unverified_required: Vec<String> = Vec::new();
        for task in &self.tasks {
            for check in &task.checks {
                if !check.required {
                    continue;
                }
                required_in_order.push((task, check));
                latest_by_name.insert(check.name.as_str(), (task, check));
                if check.passed && !check.verified {
                    unverified_required.push(format!("{} (task: {})", check.name, task.id));
                }
            }
        }
        let outstanding_failures: Vec<String> = latest_by_name
            .values()
            .filter(|(_, check)| !check.passed)
            .map(|(task, check)| format!("{} (task: {})", check.name, task.id))
            .collect();
        let recovered_failures: Vec<String> = required_in_order
            .iter()
            .filter(|(_, check)| !check.passed)
            .filter(|(_, check)| {
                latest_by_name
                    .get(check.name.as_str())
                    .is_some_and(|(_, latest)| latest.passed)
            })
            .map(|(task, check)| format!("{} (task: {})", check.name, task.id))
            .collect();
        if !outstanding_failures.is_empty() {
            warnings.push(format!(
                "required checks failed: {}",
                outstanding_failures.join(", ")
            ));
        }
        if !recovered_failures.is_empty() {
            warnings.push(format!(
                "required checks failed and later passed under the same name (recovered): {}",
                recovered_failures.join(", ")
            ));
        }
        if !unverified_required.is_empty() {
            // Self-reported green gates are exactly how a non-compiling phase
            // shipped with "tests pass". Surface them so a reviewer can tell a
            // run command apart from an agent's claim.
            warnings.push(format!(
                "required checks passed but were self-reported (not verified by running a command): {}",
                unverified_required.join(", ")
            ));
        }

        if let Some(obj) = &mut self.objective {
            obj.status = ObjectiveStatus::Completed;
            obj.completed_at = Some(Utc::now().to_rfc3339());
        }

        let all_artifacts: Vec<&Artifact> = self
            .tasks
            .iter()
            .flat_map(|t| &t.artifacts)
            .chain(artifacts.iter())
            .collect();

        crate::ship::application::ShipReport::new(
            total_tasks,
            completed,
            all_artifacts.len(),
            artifacts,
            summary,
            warnings,
        )
        .with_failures(recovered_failures, outstanding_failures)
    }

    fn task_index(&self, task_id: &str) -> Result<usize, String> {
        self.tasks
            .iter()
            .position(|t| t.id == task_id)
            .ok_or_else(|| format!("task '{task_id}' not found"))
    }

    fn resolve_task_id(&self, explicit: Option<&str>) -> Result<String, String> {
        if let Some(id) = explicit {
            return Ok(id.to_string());
        }
        self.tasks
            .iter()
            .find(|t| t.status == TaskStatus::Active)
            .map(|t| t.id.clone())
            .ok_or_else(|| "no task_id provided and no active task".to_string())
    }
}
