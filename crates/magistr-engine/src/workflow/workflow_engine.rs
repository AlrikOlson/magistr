use serde::{Deserialize, Serialize};
use tracing::{debug, instrument};

use crate::error::{EngineError, Result};

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
    /// Count of consecutive iterations with no commit.
    consecutive_no_commits: u32,
    /// Whether the loop is paused (will stop after current iteration).
    #[serde(default)]
    paused: bool,
    /// Whether the loop has been aborted.
    #[serde(default)]
    aborted: bool,
}

/// Maximum consecutive no-commit iterations before stopping.
const MAX_NO_COMMITS: u32 = 3;

impl WorkflowEngine {
    /// Create a new workflow engine.
    pub fn new(config: WorkflowConfig) -> Self {
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
            consecutive_no_commits: 0,
            paused: false,
            aborted: false,
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
        self.consecutive_no_commits
    }

    /// Current iteration for display (1-based).
    pub fn display_iteration(&self) -> u32 {
        self.iteration.iteration + 1
    }

    /// Whether the loop is paused.
    pub fn is_paused(&self) -> bool {
        self.paused
    }

    /// Whether the loop has been aborted.
    pub fn is_aborted(&self) -> bool {
        self.aborted
    }

    // ── Phase transitions ───────────────────────────────────────────────

    /// Advance to the next workflow phase.
    ///
    /// Validates that the transition follows the workflow order:
    /// Researching → Planning → Executing → Verifying → Complete.
    /// Skipping phases forward is allowed. Regression is not.
    #[instrument(skip(self), fields(from = %self.phase, to = %to))]
    pub fn advance_phase(&mut self, to: WorkflowPhase) -> Result<()> {
        if to.step_index() <= self.phase.step_index() && !self.phase.is_terminal() {
            // Allow re-entering the same phase (idempotent)
            if to == self.phase {
                return Ok(());
            }
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
        self.iteration.spec_total > 0 && self.iteration.spec_checked >= self.iteration.spec_total
    }

    /// Whether the iteration limit has been reached.
    pub fn is_iteration_limit_reached(&self) -> bool {
        self.iteration.iteration >= self.iteration.max_iterations
    }

    /// Whether the loop should stop due to repeated no-commit failures.
    pub fn should_stop_no_commits(&self) -> bool {
        self.consecutive_no_commits >= MAX_NO_COMMITS
    }

    /// Whether any stop condition is met.
    pub fn should_stop(&self) -> bool {
        self.is_spec_complete()
            || self.is_iteration_limit_reached()
            || self.should_stop_no_commits()
            || self.paused
            || self.aborted
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
        self.consecutive_no_commits = 0;
    }

    /// Record an iteration with no commit.
    pub fn record_no_commit(&mut self) {
        debug!(
            iteration = self.iteration.iteration,
            consecutive = self.consecutive_no_commits + 1,
            "iteration had no commit"
        );
        self.iteration.last_status = IterationStatus::NoCommit;
        self.consecutive_no_commits += 1;
    }

    /// Record a failed iteration.
    pub fn record_failed(&mut self) {
        debug!(iteration = self.iteration.iteration, "iteration failed");
        self.iteration.last_status = IterationStatus::Failed;
    }

    /// Put the loop back where a snapshot says it was.
    ///
    /// `consecutive_no_commits` is restored alongside the iteration
    /// state on purpose. It is a *stop condition*, not a statistic: a
    /// loop that has failed to commit twice is one iteration from
    /// stopping, and dropping the count on a migration would let a
    /// wedged loop launder its own stop by moving machines.
    pub fn restore_iteration(&mut self, iteration: IterationState, consecutive_no_commits: u32) {
        self.iteration = iteration;
        self.consecutive_no_commits = consecutive_no_commits;
    }

    /// Advance to the next iteration.
    #[instrument(skip(self), fields(iteration = self.iteration.iteration))]
    pub fn advance_iteration(&mut self) {
        self.iteration.iteration += 1;
        self.reset_for_next_iteration();
    }

    // ── Command validation & execution ──────────────────────────────────

    /// Validate whether a workflow command can be executed in the current state.
    pub fn validate_command(&self, command: &WorkflowCommand) -> Result<()> {
        match command {
            WorkflowCommand::StartIteration => {
                if self.aborted {
                    return Err(EngineError::invalid_command(
                        command,
                        "aborted",
                        "cannot start iteration after abort",
                    ));
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
                if self.paused {
                    return Err(EngineError::invalid_command(
                        command,
                        "paused",
                        "already paused",
                    ));
                }
                if self.aborted {
                    return Err(EngineError::invalid_command(
                        command,
                        "aborted",
                        "cannot pause after abort",
                    ));
                }
                Ok(())
            }
            WorkflowCommand::Resume => {
                if !self.paused {
                    return Err(EngineError::invalid_command(
                        command,
                        "running",
                        "not paused",
                    ));
                }
                Ok(())
            }
            WorkflowCommand::Skip => {
                if self.aborted {
                    return Err(EngineError::invalid_command(
                        command,
                        "aborted",
                        "cannot skip after abort",
                    ));
                }
                Ok(())
            }
            WorkflowCommand::AbortLoop => {
                if self.aborted {
                    return Err(EngineError::invalid_command(
                        command,
                        "aborted",
                        "already aborted",
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
                self.paused = true;
            }
            WorkflowCommand::Resume => {
                self.paused = false;
            }
            WorkflowCommand::Skip => {
                self.record_no_commit();
                self.advance_iteration();
            }
            WorkflowCommand::AbortLoop => {
                self.aborted = true;
            }
        }
        Ok(())
    }
}

#[cfg(test)]
#[path = "workflow_engine.tests.rs"]
mod tests;
