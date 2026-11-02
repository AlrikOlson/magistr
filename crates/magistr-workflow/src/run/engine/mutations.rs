use super::RunEngine;
use crate::run::application::{RunError, RunReport};
use crate::run::broadcast::BroadcastFrame;
use crate::run::domain::{
    artifact::Artifact,
    check::{Check, CheckType},
    execution_identity::ExecutionIdentity,
    goal::{Goal, GoalStatus, VerificationMode},
    progress::{Progress, ProgressType},
    step::{Step, StepStatus, StepType},
};
use crate::run::persistence::PersistedState;
use crate::run::saved_cycle::SavedCycle;
use chrono::Utc;
use magistr_workflow_contracts::Observed;

impl RunEngine {
    pub fn set_goal(
        &mut self,
        description: String,
        acceptance_criteria: Vec<String>,
        constraints: Vec<String>,
        scope: String,
    ) -> Result<Observed<usize>, RunError> {
        self.set_goal_with_verification(
            description,
            acceptance_criteria,
            constraints,
            scope,
            VerificationMode::Focused,
        )
    }

    pub fn set_goal_with_verification(
        &mut self,
        description: String,
        acceptance_criteria: Vec<String>,
        constraints: Vec<String>,
        scope: String,
        verification: VerificationMode,
    ) -> Result<Observed<usize>, RunError> {
        let result = self.mutate(&[], |state| {
            Ok(state.set_goal_with_verification(
                description,
                acceptance_criteria,
                constraints,
                scope,
                verification,
            ))
        })?;
        self.broadcast(BroadcastFrame::GoalSet {
            goal: self.goal.clone().expect("committed goal"),
        });
        Ok(result)
    }

    pub fn add_step(
        &mut self,
        id: String,
        title: String,
        step_type: StepType,
        estimate: Option<String>,
        decision_thread: Option<String>,
    ) -> Result<Observed<()>, RunError> {
        let step_id = id.clone();
        let step_title = title.clone();
        let result = self.mutate(&[], |state| {
            state
                .add_step(id, title, step_type, estimate, decision_thread)
                .map_err(Into::into)
        })?;
        self.broadcast(BroadcastFrame::StepAdded {
            step_id,
            title: step_title,
        });
        Ok(result)
    }

    pub fn remove_step(&mut self, step_id: &str) -> Result<Observed<()>, RunError> {
        self.mutate(&[step_id], |state| {
            state.remove_step(step_id).map_err(Into::into)
        })
    }

    pub fn reorder_step(
        &mut self,
        step_id: &str,
        after: Option<&str>,
    ) -> Result<Observed<()>, RunError> {
        let mut targets = vec![step_id];
        targets.extend(after);
        self.mutate(&targets, |state| {
            state.reorder_step(step_id, after).map_err(Into::into)
        })
    }

    pub fn start_step(&mut self, step_id: &str) -> Result<Observed<Step>, RunError> {
        let result = self.mutate(&[step_id], |state| {
            state.start_step(step_id).cloned().map_err(Into::into)
        })?;
        self.broadcast(BroadcastFrame::StepStarted {
            step_id: step_id.into(),
        });
        Ok(result)
    }

    #[allow(clippy::too_many_arguments)]
    pub fn record_progress(
        &mut self,
        step_id: Option<&str>,
        progress_type: ProgressType,
        description: String,
        files_touched: Vec<String>,
        tools_used: Vec<String>,
        result: String,
        note: Option<u32>,
    ) -> Result<Observed<Progress>, RunError> {
        let target = self.snapshot().admit_step(step_id)?.step_id;
        let result = self.mutate(&[&target], |state| {
            state
                .record_progress(
                    Some(&target),
                    progress_type,
                    description,
                    files_touched,
                    tools_used,
                    result,
                    note,
                )
                .cloned()
                .map_err(Into::into)
        })?;
        self.broadcast(BroadcastFrame::ProgressRecorded {
            step_id: target,
            progress: result.value.clone(),
        });
        Ok(result)
    }

    pub fn complete_step(
        &mut self,
        step_id: &str,
        artifacts: Vec<Artifact>,
    ) -> Result<Observed<Step>, RunError> {
        let result = self.mutate(&[step_id], |state| {
            state
                .complete_step(step_id, artifacts)
                .cloned()
                .map_err(Into::into)
        })?;
        self.broadcast(BroadcastFrame::StepDone {
            step: result.value.clone(),
        });
        Ok(result)
    }

    pub fn block_step(
        &mut self,
        step_id: &str,
        reason: String,
    ) -> Result<Observed<Step>, RunError> {
        let result = self.mutate(&[step_id], |state| {
            state
                .block_step(step_id, reason.clone())
                .cloned()
                .map_err(Into::into)
        })?;
        self.broadcast(BroadcastFrame::StepWaiting {
            step_id: step_id.into(),
            reason,
        });
        Ok(result)
    }

    pub fn record_check(
        &mut self,
        step_id: Option<&str>,
        check_type: CheckType,
        name: String,
        passed: bool,
        details: String,
        required: bool,
    ) -> Result<Observed<Check>, RunError> {
        self.record_check_full(
            step_id, check_type, name, passed, details, required, false, None, None, None, None,
        )
    }

    #[allow(clippy::too_many_arguments)]
    pub fn record_check_full(
        &mut self,
        step_id: Option<&str>,
        check_type: CheckType,
        name: String,
        passed: bool,
        details: String,
        required: bool,
        verified: bool,
        command: Option<String>,
        exit_code: Option<i32>,
        report: Option<crate::run::report::ReportRecord>,
        results: Option<crate::run::report::TestResults>,
    ) -> Result<Observed<Check>, RunError> {
        let target = self.snapshot().admit_step(step_id)?.step_id;
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
            step_id: target,
            check: result.value.clone(),
        });
        Ok(result)
    }

    pub fn finalize(
        &mut self,
        artifacts: Vec<Artifact>,
        summary: Option<String>,
    ) -> Result<Observed<RunReport>, RunError> {
        let result = self.mutate(&[], |state| Ok(state.finalize(artifacts, summary)))?;
        self.broadcast(BroadcastFrame::RunFinished {
            warnings: result.value.warnings.clone(),
        });
        Ok(result)
    }

    pub fn reset(&mut self) -> Result<Observed<()>, RunError> {
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

    pub fn set_goal_with_verification(
        &mut self,
        description: String,
        acceptance_criteria: Vec<String>,
        constraints: Vec<String>,
        scope: String,
        verification: VerificationMode,
    ) -> usize {
        let mut cleared = 0;
        if self.goal.is_none() {
            self.advance_selection();
        }
        let switching = self.goal.as_ref().is_some_and(|goal| goal.scope != scope);
        if switching {
            if let Some(goal) = self.goal.take() {
                let steps = std::mem::take(&mut self.steps);
                if matches!(goal.status, GoalStatus::Finished | GoalStatus::Abandoned) {
                    cleared = steps.len();
                }
                self.saved_cycles.push(SavedCycle {
                    goal,
                    steps,
                    next_progress_id: self.next_progress_id,
                    execution: self.execution.clone(),
                });
            }
            self.advance_selection();
            if let Some(index) = self.saved_cycles.iter().position(|cycle| {
                cycle.goal.scope == scope
                    && matches!(cycle.goal.status, GoalStatus::Open | GoalStatus::Active)
            }) {
                let saved = self.saved_cycles.remove(index);
                self.goal = Some(saved.goal);
                self.steps = saved.steps;
                self.next_progress_id = saved.next_progress_id;
                self.execution = saved.execution;
                return cleared;
            } else {
                self.next_progress_id = 1;
                self.execution = ExecutionIdentity::fresh();
            }
        }
        let prior_finished = matches!(
            self.goal.as_ref().map(|o| &o.status),
            Some(GoalStatus::Finished) | Some(GoalStatus::Abandoned)
        );
        if prior_finished {
            cleared = self.steps.len();
            self.advance_selection();
            self.saved_cycles.push(SavedCycle {
                goal: self.goal.take().expect("finished goal"),
                steps: std::mem::take(&mut self.steps),
                next_progress_id: self.next_progress_id,
                execution: self.execution.clone(),
            });
            self.next_progress_id = 1;
            self.execution = ExecutionIdentity::fresh();
        }
        let created_at = self
            .goal
            .as_ref()
            .filter(|_| !prior_finished)
            .and_then(|goal| goal.created_at.clone())
            .unwrap_or_else(|| Utc::now().to_rfc3339());
        self.goal = Some(Goal {
            description,
            acceptance_criteria,
            constraints,
            scope,
            verification,
            status: self
                .goal
                .as_ref()
                .filter(|_| !prior_finished)
                .map_or(GoalStatus::Open, |goal| goal.status),
            project_id: self.project_id.clone(),
            created_at: Some(created_at),
            completed_at: None,
        });
        cleared
    }

    pub fn add_step(
        &mut self,
        id: String,
        title: String,
        step_type: StepType,
        estimate: Option<String>,
        decision_thread: Option<String>,
    ) -> Result<(), String> {
        if self.steps.iter().any(|step| step.id == id) {
            return Err(format!("step '{id}' already exists"));
        }
        if let Some(obj) = &mut self.goal
            && obj.status == GoalStatus::Open
        {
            obj.status = GoalStatus::Active;
        }
        self.execution.add_step(&id);
        self.steps.push(Step {
            id,
            title,
            step_type,
            status: StepStatus::Planned,
            estimate,
            started_at: None,
            completed_at: None,
            artifacts: Vec::new(),
            checks: Vec::new(),
            progress: Vec::new(),
            waiting_reason: None,
            decision_thread,
        });
        Ok(())
    }

    pub fn remove_step(&mut self, step_id: &str) -> Result<(), String> {
        let idx = self.step_index(step_id)?;
        let status = &self.steps[idx].status;
        if *status == StepStatus::Active || *status == StepStatus::Done {
            return Err(format!(
                "cannot remove step '{step_id}' with status {status:?}"
            ));
        }
        self.steps.remove(idx);
        self.execution.remove_step(step_id);
        Ok(())
    }

    pub fn reorder_step(&mut self, step_id: &str, after: Option<&str>) -> Result<(), String> {
        let idx = self.step_index(step_id)?;
        let destination = after.map(|id| self.step_index(id)).transpose()?;
        if destination == Some(idx) {
            return Err("A step cannot be placed after itself.".into());
        }
        let step = self.steps.remove(idx);
        let insert_at = destination.map_or(0, |index| if index > idx { index } else { index + 1 });
        self.steps.insert(insert_at, step);
        Ok(())
    }

    pub fn start_step(&mut self, step_id: &str) -> Result<&Step, String> {
        if let Some(active) = self.steps.iter().find(|t| t.status == StepStatus::Active) {
            return Err(format!(
                "step '{}' is already active — complete or block it first",
                active.id
            ));
        }
        let idx = self.step_index(step_id)?;
        let step = &mut self.steps[idx];
        if step.status != StepStatus::Planned && step.status != StepStatus::Waiting {
            return Err(format!(
                "step '{step_id}' has status {:?}, cannot start",
                step.status
            ));
        }
        step.status = StepStatus::Active;
        step.started_at = Some(Utc::now().to_rfc3339());
        step.waiting_reason = None;
        Ok(&self.steps[idx])
    }

    #[allow(clippy::too_many_arguments)]
    pub fn record_progress(
        &mut self,
        step_id: Option<&str>,
        progress_type: ProgressType,
        description: String,
        files_touched: Vec<String>,
        tools_used: Vec<String>,
        result: String,
        note: Option<u32>,
    ) -> Result<&Progress, String> {
        let tid = self.resolve_step_id(step_id)?;
        let idx = self.step_index(&tid)?;
        let progress_id = self.next_progress_id;
        self.next_progress_id = self
            .next_progress_id
            .checked_add(1)
            .ok_or("The progress count is exhausted.")?;
        let progress = Progress {
            id: progress_id,
            step_id: tid,
            timestamp: Utc::now().to_rfc3339(),
            progress_type,
            description,
            files_touched,
            tools_used,
            result,
            note,
        };
        self.steps[idx].progress.push(progress);
        Ok(self.steps[idx].progress.last().unwrap())
    }

    pub fn complete_step(
        &mut self,
        step_id: &str,
        artifacts: Vec<Artifact>,
    ) -> Result<&Step, String> {
        let idx = self.step_index(step_id)?;
        let step = &mut self.steps[idx];
        if step.status != StepStatus::Active && step.status != StepStatus::Waiting {
            return Err(format!(
                "step '{step_id}' has status {:?}, cannot complete",
                step.status
            ));
        }
        step.status = StepStatus::Done;
        step.completed_at = Some(Utc::now().to_rfc3339());
        step.waiting_reason = None;
        step.artifacts.extend(artifacts);
        Ok(&self.steps[idx])
    }

    pub fn block_step(&mut self, step_id: &str, reason: String) -> Result<&Step, String> {
        let idx = self.step_index(step_id)?;
        let step = &mut self.steps[idx];
        if step.status != StepStatus::Active {
            return Err(format!(
                "step '{step_id}' has status {:?}, only active steps can be blocked",
                step.status
            ));
        }
        step.status = StepStatus::Waiting;
        step.waiting_reason = Some(reason);
        Ok(&self.steps[idx])
    }

    #[allow(clippy::too_many_arguments)]
    pub fn record_check_full(
        &mut self,
        step_id: Option<&str>,
        check_type: CheckType,
        name: String,
        passed: bool,
        details: String,
        required: bool,
        verified: bool,
        command: Option<String>,
        exit_code: Option<i32>,
        report: Option<crate::run::report::ReportRecord>,
        results: Option<crate::run::report::TestResults>,
    ) -> Result<&Check, String> {
        let tid = self.resolve_step_id(step_id)?;
        let idx = self.step_index(&tid)?;
        let check = Check {
            check_result_id: None,
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
        self.steps[idx].checks.push(check);
        Ok(self.steps[idx].checks.last().unwrap())
    }

    pub fn finalize(
        &mut self,
        artifacts: Vec<Artifact>,
        summary: Option<String>,
    ) -> crate::run::application::RunReport {
        let mut warnings: Vec<String> = Vec::new();

        let total_steps = self.steps.len();
        let completed = self
            .steps
            .iter()
            .filter(|t| t.status == StepStatus::Done)
            .count();
        let incomplete: Vec<&str> = self
            .steps
            .iter()
            .filter(|t| t.status != StepStatus::Done && t.status != StepStatus::Skipped)
            .map(|t| t.id.as_str())
            .collect();

        if !incomplete.is_empty() {
            warnings.push(format!(
                "{} step(s) not completed: {}",
                incomplete.len(),
                incomplete.join(", ")
            ));
        }

        // Required checks in recording order, latest per name last: a
        // failure a later record of the same name passed over is
        // recovered, one nothing passed over is outstanding.
        let mut latest_by_name: std::collections::BTreeMap<&str, (&Step, &Check)> =
            std::collections::BTreeMap::new();
        let mut required_in_order: Vec<(&Step, &Check)> = Vec::new();
        let mut unverified_required: Vec<String> = Vec::new();
        for step in &self.steps {
            for check in &step.checks {
                if !check.required {
                    continue;
                }
                required_in_order.push((step, check));
                latest_by_name.insert(check.name.as_str(), (step, check));
                if check.passed && !check.verified {
                    unverified_required.push(format!("{} (step: {})", check.name, step.id));
                }
            }
        }
        let outstanding_failures: Vec<String> = latest_by_name
            .values()
            .filter(|(_, check)| !check.passed)
            .map(|(step, check)| format!("{} (step: {})", check.name, step.id))
            .collect();
        let recovered_failures: Vec<String> = required_in_order
            .iter()
            .filter(|(_, check)| !check.passed)
            .filter(|(_, check)| {
                latest_by_name
                    .get(check.name.as_str())
                    .is_some_and(|(_, latest)| latest.passed)
            })
            .map(|(step, check)| format!("{} (step: {})", check.name, step.id))
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

        if let Some(obj) = &mut self.goal {
            obj.status = GoalStatus::Finished;
            obj.completed_at = Some(Utc::now().to_rfc3339());
        }

        let all_artifacts: Vec<&Artifact> = self
            .steps
            .iter()
            .flat_map(|t| &t.artifacts)
            .chain(artifacts.iter())
            .collect();

        crate::run::application::RunReport::new(
            total_steps,
            completed,
            all_artifacts.len(),
            artifacts,
            summary,
            warnings,
        )
        .with_failures(recovered_failures, outstanding_failures)
    }

    fn step_index(&self, step_id: &str) -> Result<usize, String> {
        self.steps
            .iter()
            .position(|t| t.id == step_id)
            .ok_or_else(|| format!("step '{step_id}' not found"))
    }

    fn resolve_step_id(&self, explicit: Option<&str>) -> Result<String, String> {
        if let Some(id) = explicit {
            return Ok(id.to_string());
        }
        self.steps
            .iter()
            .find(|t| t.status == StepStatus::Active)
            .map(|t| t.id.clone())
            .ok_or_else(|| "no step_id provided and no active step".to_string())
    }
}
