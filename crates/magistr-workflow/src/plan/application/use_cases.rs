use magistr_workflow_contracts::{Observed, WriteError};
use std::sync::{Arc, Mutex, MutexGuard};

use crate::plan::domain::{Task, TaskStatus};
use crate::plan::engine::PlanEngine;
use crate::run::RunUseCases;
use crate::signal::engine::SignalEngine;

use super::{PlanError, TaskDraft};

/// Admitted native plan operations shared by MCP and local clients.
///
/// The run family is the plan's completion authority, by construction:
/// a task completes only on that run's current evidence — every route —
/// MCP, an owner command, the cockpit, a loop — lands here, and none of
/// them can mark a task done another way. A run family composed without
/// an evidence capability refuses every completion with the reason, so a
/// plan can never be composed over a permissive path.
#[derive(Clone)]
pub struct PlanUseCases {
    pub(super) engine: Arc<Mutex<PlanEngine>>,
    pub(super) signal: Option<Arc<Mutex<SignalEngine>>>,
    pub(super) workspace_root: Option<std::path::PathBuf>,
    pub(super) completion: RunUseCases,
}

#[cfg(test)]
#[path = "use_cases.tests.rs"]
mod tests;

impl PlanUseCases {
    /// Compose the plan over `engine` with `completion` — the run
    /// family whose receipts govern every route to `done`.
    pub fn new(engine: Arc<Mutex<PlanEngine>>, completion: RunUseCases) -> Self {
        Self {
            engine,
            signal: None,
            workspace_root: None,
            completion,
        }
    }

    pub fn with_signal(mut self, signal: Arc<Mutex<SignalEngine>>) -> Self {
        self.signal = Some(signal);
        self
    }

    /// The refusal every route to `done` other than `complete_task`
    /// answers with: completion is decided on the run's receipts, never
    /// by naming the status.
    pub(super) fn refuse_direct_done(id: &str) -> PlanError {
        PlanError::InvalidState(format!(
            "task '{id}' cannot be set done directly: completion is governed by the run's verification receipts — use plan_complete_task"
        ))
    }

    pub fn engine(&self) -> Arc<Mutex<PlanEngine>> {
        Arc::clone(&self.engine)
    }

    pub fn with_workspace_root(
        mut self,
        root: impl Into<std::path::PathBuf>,
    ) -> std::io::Result<Self> {
        self.workspace_root = Some(std::path::absolute(root.into())?);
        Ok(self)
    }

    /// The engine, current: every query and mutation observes the store
    /// before it answers or acts, so two roles on one store answer one
    /// state through a validated read of the selected store.
    pub(super) fn lock(&self) -> Result<MutexGuard<'_, PlanEngine>, PlanError> {
        let mut engine = self.engine.lock().map_err(|_| PlanError::Poisoned)?;
        engine
            .refresh_observed()
            .map_err(|error| PlanError::Store(WriteError::read(error)))?;
        Ok(engine)
    }

    pub fn add_task(&self, draft: TaskDraft) -> Result<Observed<Task>, PlanError> {
        if let Some(content) = &draft.content {
            content.validate().map_err(PlanError::InvalidArgs)?;
        }
        self.lock()?
            .add_task_with_content(
                draft.id,
                draft.title,
                draft.name,
                draft.status,
                draft.priority,
                draft.description,
                draft.acceptance,
                draft.deps,
                draft.shared,
                draft.content,
                draft.feature,
            )
            .map_err(PlanError::invalid_state)
    }

    pub fn set_status(&self, id: &str, status: TaskStatus) -> Result<Observed<Task>, PlanError> {
        if status == TaskStatus::Done {
            return Err(Self::refuse_direct_done(id));
        }
        self.lock()?
            .set_status(id, status)
            .map_err(PlanError::invalid_state)
    }

    pub fn start_task(&self, id: &str) -> Result<Observed<Task>, PlanError> {
        self.lock()?
            .start_task(id)
            .map_err(PlanError::invalid_state)
    }

    /// Mark `id` done with its proof-of-run reference. The run goal
    /// must be scoped to this task and its verification task — the `step:`
    /// reference when given, else the task holding the run's receipts —
    /// must carry current, passing, verified receipts for every required
    /// gate; the refusal names what is missing. The stored reference is the
    /// caller's, or the resolved task when none was given, so a completion
    /// always names its task.
    pub fn complete_task(
        &self,
        id: &str,
        step_ref: Option<&str>,
    ) -> Result<Observed<Task>, PlanError> {
        let scope = format!("task:{id}");
        let task = step_ref
            .and_then(|reference| reference.trim().strip_prefix("step:"))
            .map(str::trim);
        let evidence = self
            .completion
            .verify_completion(&scope, task)
            .map_err(|refusal| PlanError::InvalidState(refusal.to_string()))?;
        let resolved = step_ref
            .map(str::to_owned)
            .unwrap_or_else(|| format!("step:{}", evidence.step_id));
        self.lock()?
            .complete_task(id, Some(resolved.as_str()))
            .map_err(PlanError::invalid_state)
    }

    pub fn retire_task(&self, id: &str, reason: String) -> Result<Observed<Task>, PlanError> {
        self.lock()?
            .retire_task(id, reason)
            .map_err(PlanError::invalid_state)
    }

    pub fn link_task(&self, id: &str, cross_ref: &str) -> Result<Observed<Task>, PlanError> {
        self.lock()?
            .link_task(id, cross_ref)
            .map_err(PlanError::invalid_args)
    }

    pub fn set_feature(
        &self,
        id: &str,
        feature: Option<String>,
    ) -> Result<Observed<Task>, PlanError> {
        self.lock()?
            .set_feature(id, feature)
            .map_err(PlanError::invalid_state)
    }

    pub fn record_refresh(
        &self,
        summary: String,
        think_steps: Vec<u32>,
    ) -> Result<Observed<usize>, PlanError> {
        self.lock()?
            .record_refresh(summary, think_steps)
            .map_err(PlanError::invalid_state)
    }
}
