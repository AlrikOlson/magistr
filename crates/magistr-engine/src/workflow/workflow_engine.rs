use serde::{Deserialize, Serialize};
use tracing::{debug, instrument};

use crate::error::{EngineError, Result};
use magistr_domain::workflow::{RunMode, RunState, StopReason};

use super::iteration_state::IterationState;
use super::iteration_status::IterationStatus;
use super::workflow_command::WorkflowCommand;
use super::workflow_config::WorkflowConfig;
use super::workflow_phase::WorkflowPhase;

/// Domain service for workflow phase orchestration.
///
/// Manages phase transitions with validation and tracks iteration progress.
/// Pure domain logic — no IO, no process spawning.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WorkflowEngine {
    /// Current workflow phase.
    phase: WorkflowPhase,
    /// Workflow configuration.
    config: WorkflowConfig,
    /// Iteration tracking (for loop mode).
    iteration: IterationState,
    /// Shared run policy; the iteration value above is a compatibility view.
    run: RunState,
}

impl WorkflowEngine {
    /// Create a new workflow engine.
    pub fn new(config: WorkflowConfig) -> Self {
        Self::with_run_mode(config, RunMode::ChunkLoop)
    }

    /// Construct an explicitly selected execution mode.
    pub fn with_run_mode(config: WorkflowConfig, mode: RunMode) -> Self {
        let max_iterations = config.max_iterations;
        Self {
            phase: WorkflowPhase::Researching,
            config,
            iteration: IterationState {
                iteration: 0,
                max_iterations,
                last_status: IterationStatus::None,
                spec_checked: 0,
                spec_total: 0,
            },
            run: RunState::new(mode, max_iterations, None),
        }
    }

    // ── Accessors ───────────────────────────────────────────────────────

    pub fn phase(&self) -> WorkflowPhase {
        self.phase
    }

    pub fn config(&self) -> &WorkflowConfig {
        &self.config
    }

    pub fn iteration(&self) -> &IterationState {
        &self.iteration
    }

    pub fn consecutive_no_commits(&self) -> u32 {
        self.run.consecutive_no_commits
    }

    pub fn run(&self) -> &RunState {
        &self.run
    }

    /// Record an external terminal outcome without inferring successful work.
    pub fn stop(&mut self, reason: StopReason) {
        self.run.stop(reason);
    }

    /// Install the governor's configured limit and cumulative observation.
    pub fn observe_budget(&mut self, total_usd: f64, limit_usd: Option<f64>) {
        self.run.budget_limit_usd = limit_usd;
        self.run.observe_budget(total_usd);
    }

    /// Current iteration for display (1-based).
    pub fn display_iteration(&self) -> u32 {
        self.iteration.iteration.saturating_add(1)
    }

    /// Whether the loop is paused.
    pub fn is_paused(&self) -> bool {
        self.run.terminal_reason == Some(StopReason::Paused)
    }

    /// Whether the loop has been aborted.
    pub fn is_aborted(&self) -> bool {
        self.run.terminal_reason == Some(StopReason::Aborted)
    }

    // ── Phase transitions ───────────────────────────────────────────────

    /// Advance to the next workflow phase.
    ///
    /// Validates that the transition follows the workflow order:
    /// Researching → Planning → Executing → Verifying → Complete.
    /// Skipping phases forward is allowed. Regression is not.
    #[instrument(skip(self), fields(from = %self.phase, to = %to))]
    pub fn advance_phase(&mut self, to: WorkflowPhase) -> Result<()> {
        if !self.phase.can_advance_to(to) {
            return Err(EngineError::invalid_phase_transition(self.phase, to));
        }
        self.phase = to;
        Ok(())
    }

    /// Force-set the phase (bypass validation). For crash recovery.
    pub fn force_phase(&mut self, phase: WorkflowPhase) {
        self.phase = phase;
    }

    /// Reset to Researching for the next iteration.
    pub fn reset_for_next_iteration(&mut self) {
        self.phase = WorkflowPhase::Researching;
    }

    // ── Iteration management ────────────────────────────────────────────

    /// Whether the spec is fully complete.
    pub fn is_spec_complete(&self) -> bool {
        self.iteration.is_spec_complete()
    }

    /// Whether the iteration limit has been reached.
    pub fn is_iteration_limit_reached(&self) -> bool {
        self.run.is_iteration_limit_reached()
    }

    /// Whether the loop should stop due to repeated no-commit failures.
    pub fn should_stop_no_commits(&self) -> bool {
        self.run.is_no_progress_exhausted()
    }

    /// Whether any stop condition is met.
    pub fn should_stop(&self) -> bool {
        self.stop_reason().is_some()
    }

    pub fn stop_reason(&self) -> Option<StopReason> {
        self.run.stop_reason().or_else(|| {
            (self.run.mode != RunMode::Conversation && self.is_spec_complete())
                .then_some(StopReason::SpecComplete)
        })
    }

    /// Update spec progress counts.
    pub fn update_spec_progress(&mut self, checked: u32, total: u32) {
        self.iteration.spec_checked = checked;
        self.iteration.spec_total = total;
    }

    /// Record a committed iteration.
    pub fn record_committed(&mut self) {
        debug!(iteration = self.iteration.iteration, "iteration committed");
        self.iteration.last_status = IterationStatus::Committed;
        self.run.record_commit(true);
    }

    /// Record an iteration with no commit.
    pub fn record_no_commit(&mut self) {
        debug!(
            iteration = self.iteration.iteration,
            consecutive = self.run.consecutive_no_commits.saturating_add(1),
            "iteration had no commit"
        );
        self.iteration.last_status = IterationStatus::NoCommit;
        self.run.record_commit(false);
    }

    /// Record a failed iteration.
    pub fn record_failed(&mut self) {
        debug!(iteration = self.iteration.iteration, "iteration failed");
        self.iteration.last_status = IterationStatus::Failed;
        self.run.stop(StopReason::Failed);
    }

    /// Put the loop back where a snapshot says it was.
    ///
    /// `consecutive_no_commits` is restored alongside the iteration
    /// state on purpose. It is a *stop condition*, not a statistic: a
    /// loop that has failed to commit twice must stop. Dropping the count would
    /// let a wedged loop launder its own stop by moving machines.
    /// New checkpoints must use `restore_run` to carry settlement and budget state.
    pub fn restore_iteration(
        &mut self,
        iteration: IterationState,
        consecutive_no_commits: u32,
    ) -> Result<()> {
        let mut run = self.run.clone();
        run.completed_attempts = iteration.iteration;
        run.max_iterations = iteration.max_iterations;
        run.consecutive_no_commits = consecutive_no_commits;
        self.restore_run(iteration, run)
    }

    /// Restore validated canonical run evidence and its compatibility view.
    pub fn restore_run(&mut self, iteration: IterationState, run: RunState) -> Result<()> {
        run.validate()
            .map_err(|message| EngineError::Precondition(message.into()))?;
        if run.completed_attempts != iteration.iteration
            || run.max_iterations != iteration.max_iterations
        {
            return Err(EngineError::Precondition(
                "run and iteration checkpoint disagree".into(),
            ));
        }
        if !self.run.is_pristine() || self.phase != WorkflowPhase::Researching {
            run.validate_recovery(&self.run)
                .map_err(|message| EngineError::Precondition(message.into()))?;
            if self.is_spec_complete() && !iteration.is_spec_complete() {
                return Err(EngineError::Precondition(
                    "checkpoint would erase observed spec completion".into(),
                ));
            }
        }
        self.config.max_iterations = run.max_iterations;
        self.iteration = iteration;
        self.run = run;
        Ok(())
    }

    /// Advance to the next iteration.
    #[instrument(skip(self), fields(iteration = self.iteration.iteration))]
    pub fn advance_iteration(&mut self) {
        self.run.advance_attempt();
        self.iteration.iteration = self.run.completed_attempts;
        self.reset_for_next_iteration();
    }

    // ── Command validation & execution ──────────────────────────────────

    /// Validate whether a workflow command can be executed in the current state.
    pub fn validate_command(&self, command: &WorkflowCommand) -> Result<()> {
        match command {
            WorkflowCommand::StartIteration => {
                if let Some(reason) = self.stop_reason() {
                    return Err(EngineError::invalid_command(
                        command,
                        reason,
                        "cannot start another attempt while the run is stopped",
                    ));
                }
                if self.phase == WorkflowPhase::Complete {
                    let mut next = self.run.clone();
                    next.advance_attempt();
                    if let Some(reason) = next.stop_reason() {
                        return Err(EngineError::invalid_command(
                            command,
                            reason,
                            "next attempt exceeds the run policy",
                        ));
                    }
                }
                if self.phase != WorkflowPhase::Researching && self.phase != WorkflowPhase::Complete
                {
                    return Err(EngineError::invalid_command(
                        command,
                        self.phase,
                        "can only start iteration from Researching or Complete phase",
                    ));
                }
                Ok(())
            }
            WorkflowCommand::Pause => {
                if self.is_paused() {
                    return Err(EngineError::invalid_command(
                        command,
                        "paused",
                        "already paused",
                    ));
                }
                if let Some(reason) = self.run.terminal_reason {
                    return Err(EngineError::invalid_command(
                        command,
                        reason,
                        "cannot pause a terminal run",
                    ));
                }
                Ok(())
            }
            WorkflowCommand::Resume => {
                if !self.is_paused() {
                    return Err(EngineError::invalid_command(
                        command,
                        "running",
                        "not paused",
                    ));
                }
                Ok(())
            }
            WorkflowCommand::Skip => {
                if let Some(reason) = self.stop_reason() {
                    return Err(EngineError::invalid_command(
                        command,
                        reason,
                        "cannot skip while the run is stopped",
                    ));
                }
                Ok(())
            }
            WorkflowCommand::AbortLoop => {
                if let Some(reason) = self.run.terminal_reason
                    && reason != StopReason::Paused
                {
                    return Err(EngineError::invalid_command(
                        command,
                        reason,
                        "run already has a terminal outcome",
                    ));
                }
                Ok(())
            }
        }
    }

    /// Execute a validated workflow command, mutating engine state.
    ///
    /// Callers should call `validate_command` first, or use this method
    /// which validates internally and returns an error if invalid.
    #[instrument(skip(self), fields(%command, phase = %self.phase))]
    pub fn execute_command(&mut self, command: &WorkflowCommand) -> Result<()> {
        self.validate_command(command)?;
        match command {
            WorkflowCommand::StartIteration => {
                if self.phase == WorkflowPhase::Complete {
                    self.advance_iteration();
                }
                // Already in Researching — nothing more to do.
            }
            WorkflowCommand::Pause => {
                self.run.stop(StopReason::Paused);
            }
            WorkflowCommand::Resume => {
                self.run.resume();
            }
            WorkflowCommand::Skip => {
                self.record_no_commit();
                self.advance_iteration();
            }
            WorkflowCommand::AbortLoop => {
                self.run.stop(StopReason::Aborted);
            }
        }
        Ok(())
    }
}

#[cfg(test)]
#[path = "workflow_engine.tests.rs"]
mod tests;
