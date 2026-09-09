use serde::{Deserialize, Serialize};

use super::{RunMode, StopReason};

/// Persisted run policy and observations shared by every execution adapter.
///
/// Fields deliberately have no deserialization defaults. An older checkpoint
/// without stopping evidence requires an explicit migration or a new run.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RunState {
    pub mode: RunMode,
    /// Zero-based attempt cursor; advanced after an attempt is settled.
    pub completed_attempts: u32,
    pub max_iterations: u32,
    pub consecutive_no_commits: u32,
    pub budget_total_usd: f64,
    #[serde(deserialize_with = "required_option")]
    pub budget_limit_usd: Option<f64>,
    #[serde(deserialize_with = "required_option")]
    pub terminal_reason: Option<StopReason>,
    /// Suppresses duplicate settlement of the same attempt after replay.
    #[serde(deserialize_with = "required_option")]
    pub last_settled_attempt: Option<u32>,
}

fn required_option<'de, D, T>(deserializer: D) -> Result<Option<T>, D::Error>
where
    D: serde::Deserializer<'de>,
    T: Deserialize<'de>,
{
    Option::<T>::deserialize(deserializer)
}

impl RunState {
    /// The shared chunk-loop stopping threshold.
    pub const MAX_NO_COMMITS: u32 = 2;

    #[must_use]
    pub fn new(mode: RunMode, max_iterations: u32, budget_limit_usd: Option<f64>) -> Self {
        Self {
            mode,
            completed_attempts: 0,
            max_iterations,
            consecutive_no_commits: 0,
            budget_total_usd: 0.0,
            budget_limit_usd,
            terminal_reason: None,
            last_settled_attempt: None,
        }
    }

    /// Validate persisted evidence before an adapter installs a checkpoint.
    pub fn validate(&self) -> Result<(), &'static str> {
        if !self.budget_total_usd.is_finite() || self.budget_total_usd < 0.0 {
            return Err("run budget observation must be finite and nonnegative");
        }
        if self
            .budget_limit_usd
            .is_some_and(|limit| !limit.is_finite() || limit < 0.0)
        {
            return Err("run budget limit must be finite and nonnegative");
        }
        if self
            .last_settled_attempt
            .is_some_and(|attempt| attempt > self.completed_attempts)
        {
            return Err("settled attempt cannot be ahead of the run cursor");
        }
        Ok(())
    }

    /// Whether this is an unused run that can receive an imported checkpoint.
    #[must_use]
    pub fn is_pristine(&self) -> bool {
        self.completed_attempts == 0
            && self.consecutive_no_commits == 0
            && self.budget_total_usd == 0.0
            && self.terminal_reason.is_none()
            && self.last_settled_attempt.is_none()
    }

    /// A checkpoint may not erase observations already held by a live owner.
    /// A real commit observation can reset progress; importing old values cannot.
    pub fn validate_recovery(&self, known: &Self) -> Result<(), &'static str> {
        self.validate()?;
        if self.mode != known.mode
            || self.completed_attempts < known.completed_attempts
            || self.max_iterations > known.max_iterations
            || self.consecutive_no_commits < known.consecutive_no_commits
            || self.budget_total_usd < known.budget_total_usd
            || self.last_settled_attempt < known.last_settled_attempt
            || (known.terminal_reason.is_some() && self.terminal_reason != known.terminal_reason)
            || known.budget_limit_usd.is_some_and(|limit| {
                self.budget_limit_usd
                    .is_none_or(|incoming| incoming > limit)
            })
        {
            return Err(
                "checkpoint would rewind known run stopping evidence; start an explicit new run instead",
            );
        }
        Ok(())
    }

    /// Record observed Git progress once per attempt, including after recovery.
    ///
    /// A later commit observation can clear the no-progress count for an already
    /// settled attempt; repeating its no-commit observation cannot increase it.
    pub fn record_commit(&mut self, committed: bool) {
        if committed {
            self.consecutive_no_commits = 0;
        } else if self.mode == RunMode::ChunkLoop
            && self.last_settled_attempt != Some(self.completed_attempts)
        {
            self.consecutive_no_commits = self.consecutive_no_commits.saturating_add(1);
        }
        self.last_settled_attempt = Some(self.completed_attempts);
    }

    /// Advance the cursor after settling an attempt. Admission is a separate check.
    pub fn advance_attempt(&mut self) {
        self.completed_attempts = self.completed_attempts.saturating_add(1);
    }

    /// Observe cumulative spend without allowing a fresh provider to erase it.
    pub fn observe_budget(&mut self, total_usd: f64) {
        if !total_usd.is_finite() || total_usd < 0.0 {
            self.stop(StopReason::BudgetExceeded);
        } else {
            self.budget_total_usd = self.budget_total_usd.max(total_usd);
        }
    }

    /// Latch a terminal reason. A later event cannot turn a stopped run successful.
    pub fn stop(&mut self, reason: StopReason) {
        if self.terminal_reason.is_none() || self.terminal_reason == Some(StopReason::Paused) {
            self.terminal_reason = Some(reason);
        }
    }

    /// Only an explicit pause is resumable; other terminal outcomes stay terminal.
    pub fn resume(&mut self) -> bool {
        if self.terminal_reason == Some(StopReason::Paused) {
            self.terminal_reason = None;
            true
        } else {
            false
        }
    }

    #[must_use]
    pub fn is_no_progress_exhausted(&self) -> bool {
        self.mode == RunMode::ChunkLoop && self.consecutive_no_commits >= Self::MAX_NO_COMMITS
    }

    #[must_use]
    pub fn is_iteration_limit_reached(&self) -> bool {
        self.mode == RunMode::ChunkLoop && self.completed_attempts >= self.max_iterations
    }

    /// Decide whether another attempt can start from the persisted observations.
    #[must_use]
    pub fn stop_reason(&self) -> Option<StopReason> {
        if let Some(reason) = self.terminal_reason {
            return Some(reason);
        }
        if self.validate().is_err()
            || self
                .budget_limit_usd
                .is_some_and(|limit| self.budget_total_usd >= limit)
        {
            return Some(StopReason::BudgetExceeded);
        }
        if self.mode == RunMode::OneChunk
            && (self.last_settled_attempt.is_some() || self.completed_attempts > 0)
        {
            return Some(StopReason::OneChunkComplete);
        }
        if self.is_no_progress_exhausted() {
            return Some(StopReason::NoProgress);
        }
        self.is_iteration_limit_reached()
            .then_some(StopReason::IterationLimit)
    }
}

#[cfg(test)]
#[path = "run_state.tests.rs"]
mod tests;
