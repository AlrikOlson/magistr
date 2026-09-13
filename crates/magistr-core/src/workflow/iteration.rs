//! Iteration loop orchestration for `magistr next-loop`.
//!
//! Manages iteration state (count, max, last commit SHA, spec status) with
//! JSON persistence to `.magistr/loop-state.json` for crash recovery. Replaces
//! the ad-hoc `LoopState` in `plan.rs` and `/tmp`-based state files.
//!
//! The `IterationLoop` is backend-agnostic — it manages state and checks,
//! while the caller (TUI or CLI) manages agent sessions.

use std::path::{Path, PathBuf};

pub use magistr_domain::identity::AttemptId;
use magistr_domain::identity::{ProjectId, RunId};
pub use magistr_domain::workflow::{
    IterationStatus, OperationKind, OperationLedger, OperationOutcome, OperationRecord,
};
use magistr_domain::workflow::{RunMode, RunState, StopReason};
use magistr_engine::NativeEngines;
use serde::{Deserialize, Serialize};

use crate::error::{MagistrError, Result};
use crate::types::ProjectConfig;
use crate::workflow::durable_write::durable_write;
use crate::workflow::reconciliation::Reconciliation;
use crate::workflow::spec::{self, SpecSummary};

#[path = "iteration_operations.rs"]
mod operations;

// ── Persisted state ─────────────────────────────────────────────────────────

/// JSON-serializable iteration state, persisted to `.magistr/loop-state.json`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct IterationState {
    /// Compatibility projection of `run.completed_attempts`.
    pub iteration: u32,
    /// Compatibility projection of `run.max_iterations`.
    pub max_iterations: u32,
    /// Canonical run policy and stopping evidence. Required for safe recovery.
    pub run: RunState,
    /// The run every operation record is scoped to; minted when the run
    /// opened and retained across recovery.
    pub run_id: RunId,
    /// The operations this run dispatched — reservations, provider launches,
    /// receipts, native mutations, publications — each written before its
    /// effect and settled after. Required: a checkpoint without it cannot
    /// say what was in flight.
    pub operations: OperationLedger,
    /// Attempt whose Git baseline was captured, retained across recovery.
    #[serde(deserialize_with = "required_option")]
    pub attempt_started: Option<u32>,
    /// Git HEAD SHA before the current iteration started.
    /// Used to detect whether the iteration produced a commit.
    #[serde(deserialize_with = "required_option")]
    pub last_commit_sha: Option<String>,
    /// Status of the last completed iteration.
    #[serde(with = "legacy_iteration_status")]
    pub last_status: IterationStatus,
    /// Spec progress: number of checked items at last check.
    pub spec_checked: usize,
    /// Spec progress: total number of items at last check.
    pub spec_total: usize,
}

// ── Pre/post check results ──────────────────────────────────────────────────

/// Result of a pre-iteration check.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PreCheckResult {
    /// Continue with the next iteration.
    Continue,
    /// Stop the loop — all spec items are complete.
    SpecComplete,
    /// Stop the loop — max iterations reached.
    MaxIterations,
    /// Stop the loop — the think-and-ship roadmap has no ready chunk
    /// (`roadmap_next` is `None`: everything left is done, blocked or backlog).
    NoReadyChunk,
    /// Stop for another canonical run reason, including recovery guards.
    Stopped(StopReason),
    /// Stop: recovery could not establish what happened to these attempts
    /// (a provider runtime still alive, a store it could not ask). Reconcile
    /// them — or start an explicit new run — before another attempt.
    Unreconciled(Vec<OperationRecord>),
}

/// Result of a post-iteration check.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PostCheckResult {
    /// Iteration produced a commit — advance normally.
    CommitMade {
        /// The new HEAD SHA after the commit.
        new_sha: String,
    },
    /// No commit was detected — warn and provide retry guidance.
    NoCommit {
        /// Guidance message for the next iteration's prompt.
        guidance: String,
    },
}

// ── IterationLoop ───────────────────────────────────────────────────────────

/// Orchestration-layer iteration loop manager.
///
/// Manages the lifecycle of a `magistr next-loop` run:
/// - Tracks iteration count, max, last commit SHA
/// - Runs pre/post iteration checks
/// - Persists state to `.magistr/loop-state.json` for crash recovery
/// - Bridges to TUI `LoopProgress` for display
pub struct IterationLoop {
    /// Persisted state.
    state: IterationState,
    /// Project directory (for git operations and spec parsing).
    project_dir: PathBuf,
    /// Path to the spec file.
    spec_path: PathBuf,
    /// Prevent constructor options from replacing recovered run semantics.
    recovered: bool,
    /// What recovery established about attempts left in flight.
    reconciliation: Reconciliation,
    /// The in-process think-and-ship engines when the project runs the
    /// native roadmap: the next chunk comes from `roadmap_next` and progress
    /// from the roadmap, not from the spec file.
    native: Option<NativeEngines>,
}

/// Default max iterations when `MAGISTR_MAX_ITERATIONS` is not set.
const DEFAULT_MAX_ITERATIONS: u32 = 10;

impl IterationLoop {
    /// Create a new iteration loop for the given project.
    ///
    /// Max iterations defaults to `DEFAULT_MAX_ITERATIONS` (10).
    /// Callers should pass the value from `MagistrEnv.max_iterations` if available.
    pub fn new(project_dir: impl Into<PathBuf>, spec_path: impl Into<PathBuf>) -> Result<Self> {
        Self::open(project_dir, spec_path, DEFAULT_MAX_ITERATIONS, None)
    }

    /// Create a new iteration loop with an explicit max iterations value.
    ///
    /// Ignores `MAGISTR_MAX_ITERATIONS` env var — uses the provided value.
    pub fn with_max(
        project_dir: impl Into<PathBuf>,
        spec_path: impl Into<PathBuf>,
        max_iterations: u32,
    ) -> Result<Self> {
        Self::open(project_dir, spec_path, max_iterations, None)
    }

    /// The canonical constructor.
    ///
    /// `native` carries the admitted think-and-ship writer a caller opened
    /// BEFORE entering raw mode. With `None`, the backend is resolved from
    /// `.magistr/project.json` (`roadmap::backend_for`): a native project
    /// admits this process as its host here and takes its progress from the
    /// roadmap; a yaml project reads `spec_path` as it always did.
    pub fn open(
        project_dir: impl Into<PathBuf>,
        spec_path: impl Into<PathBuf>,
        max_iterations: u32,
        native: Option<NativeEngines>,
    ) -> Result<Self> {
        let project_dir = project_dir.into();
        let spec_path = spec_path.into();
        let native = match native {
            Some(engines) => Some(engines),
            None => auto_native(&project_dir)?,
        };

        let summary = match &native {
            Some(engines) => super::native_summary::native_spec_summary(engines),
            None => spec::spec_summary_from_project(&project_dir, &spec_path)?,
        };

        let state = IterationState {
            iteration: 0,
            max_iterations,
            run: RunState::new(RunMode::ChunkLoop, max_iterations, None),
            run_id: mint_run_id(&project_dir, native.as_ref())?,
            operations: OperationLedger::new(),
            attempt_started: None,
            last_commit_sha: git_head_sha(&project_dir),
            last_status: IterationStatus::None,
            spec_checked: summary.checked,
            spec_total: summary.total,
        };

        Ok(Self {
            state,
            project_dir,
            spec_path,
            recovered: false,
            reconciliation: Reconciliation::default(),
            native,
        })
    }

    /// Open a new run only when no recovery file exists. Invalid or stopped
    /// checkpoints remain authoritative and are never silently replaced.
    pub fn open_or_recover(
        project_dir: impl Into<PathBuf>,
        spec_path: impl Into<PathBuf>,
        max_iterations: u32,
        native: Option<NativeEngines>,
    ) -> Result<Self> {
        let project_dir = project_dir.into();
        let spec_path = spec_path.into();
        if loop_state_path(&project_dir)
            .try_exists()
            .map_err(|error| MagistrError::io(loop_state_path(&project_dir), error))?
        {
            Self::recover_with(project_dir, spec_path, native)
        } else {
            Self::open(project_dir, spec_path, max_iterations, native)
        }
    }

    /// Recover from a previously persisted state (crash recovery).
    ///
    /// Reads `.magistr/loop-state.json` and reconstructs the loop. Every
    /// attempt the lost owner had in flight is marked `Indeterminate` and
    /// reconciled against its authority (see [`Self::reconciliation`]);
    /// what stays open refuses the next attempt.
    pub fn recover(project_dir: impl Into<PathBuf>, spec_path: impl Into<PathBuf>) -> Result<Self> {
        Self::recover_with(project_dir, spec_path, None)
    }

    /// [`Self::recover`] with engines a caller already opened; `None`
    /// resolves the backend like [`Self::open`].
    pub fn recover_with(
        project_dir: impl Into<PathBuf>,
        spec_path: impl Into<PathBuf>,
        native: Option<NativeEngines>,
    ) -> Result<Self> {
        let project_dir = project_dir.into();
        let spec_path = spec_path.into();
        let state_path = loop_state_path(&project_dir);

        let content =
            std::fs::read_to_string(&state_path).map_err(|e| MagistrError::io(&state_path, e))?;
        let mut state: IterationState = serde_json::from_str(&content).map_err(recovery_error)?;
        validate_state(&state).map_err(recovery_error)?;
        state.operations.mark_interrupted();
        let native = match native {
            Some(engines) => Some(engines),
            None => auto_native(&project_dir)?,
        };

        let mut recovered = Self {
            state,
            project_dir,
            spec_path,
            recovered: true,
            reconciliation: Reconciliation::default(),
            native,
        };
        recovered.reconciliation = recovered.reconcile();
        Ok(recovered)
    }

    /// Select a mode for a pristine new run. Recovery retains the saved mode.
    #[must_use]
    pub fn with_run_mode(mut self, mode: RunMode) -> Self {
        if !self.recovered
            && self.state.run.completed_attempts == 0
            && self.state.run.last_settled_attempt.is_none()
            && self.state.attempt_started.is_none()
        {
            self.state.run.mode = mode;
        }
        self
    }

    /// Canonical counters, limits, observations and terminal state.
    #[must_use]
    pub fn run_state(&self) -> &RunState {
        &self.state.run
    }

    /// Configure a budget while retaining any stricter recovered limit.
    /// Omitting an option does not remove a previously persisted limit.
    pub fn set_budget_limit(&mut self, limit: Option<f64>) -> Result<()> {
        let Some(limit) = limit else {
            return Ok(());
        };
        if !limit.is_finite() || limit < 0.0 {
            return Err(MagistrError::Budget(
                "Run budget limit must be finite and nonnegative".into(),
            ));
        }
        if self
            .state
            .run
            .budget_limit_usd
            .is_some_and(|saved| limit > saved)
        {
            return Err(MagistrError::Budget(
                "Cannot weaken a persisted run budget; start an explicit new run to change it"
                    .into(),
            ));
        }
        self.state.run.budget_limit_usd = Some(limit);
        Ok(())
    }

    /// Observe cumulative spend for the whole run, including prior providers.
    pub fn observe_budget(&mut self, total_usd: f64) {
        self.state.run.observe_budget(total_usd);
    }

    /// Persistable terminal outcome; subsequent observations cannot clear it.
    pub fn stop(&mut self, reason: StopReason) {
        self.state.run.stop(reason);
    }

    /// Drive the loop from the think-and-ship roadmap (`None` keeps the
    /// spec-file behaviour). Open the engines BEFORE entering raw mode.
    #[must_use]
    pub fn with_native(mut self, native: Option<NativeEngines>) -> Self {
        // A pristine run takes the native project's name; a recovered or
        // started one keeps the identity its records are scoped to.
        if !self.recovered
            && self.state.operations.records().is_empty()
            && let Ok(run_id) = mint_run_id(&self.project_dir, native.as_ref())
        {
            self.state.run_id = run_id;
        }
        self.native = native;
        self
    }

    /// Whether the loop reads the think-and-ship roadmap.
    #[must_use]
    pub fn is_native(&self) -> bool {
        self.native.is_some()
    }

    /// The native engines, when the loop runs on them.
    #[must_use]
    pub fn native(&self) -> Option<&NativeEngines> {
        self.native.as_ref()
    }

    /// The prompt for the next iteration, from this loop's view of progress
    /// (native: the next ready chunk is the one remaining item).
    pub fn next_prompt(&self, config: &ProjectConfig) -> Result<String> {
        let summary = self.spec_summary()?;
        if summary.all_done() {
            return Err(MagistrError::Detection(
                "Nothing to do: no ready roadmap chunk / spec items complete.".into(),
            ));
        }
        Ok(crate::workflow::prompt::build_next_prompt(config, &summary))
    }

    /// Run pre-iteration checks. Returns whether to continue or stop.
    pub fn pre_iteration_check(&mut self) -> Result<PreCheckResult> {
        if let Some(reason) = self.state.run.stop_reason() {
            self.stop(reason);
            return Ok(pre_check_stop(reason));
        }
        let open: Vec<OperationRecord> = self.state.operations.indeterminate().cloned().collect();
        if !open.is_empty() {
            return Ok(PreCheckResult::Unreconciled(open));
        }

        // Check if all spec items are done
        let summary = self.spec_summary()?;
        self.state.spec_checked = summary.checked;
        self.state.spec_total = summary.total;

        if summary.all_done() {
            let reason = if self.native.is_some() {
                StopReason::NoReadyChunk
            } else {
                StopReason::SpecComplete
            };
            self.stop(reason);
            return Ok(pre_check_stop(reason));
        }

        // A recovered in-flight attempt retains its original baseline so a
        // commit observed after a restart still counts as that attempt's work.
        if self.state.attempt_started != Some(self.state.run.completed_attempts) {
            self.state.last_commit_sha = git_head_sha(&self.project_dir);
            self.state.attempt_started = Some(self.state.run.completed_attempts);
        }
        // The attempt's slot, reserved by the checkpoint that carries this
        // record: settled the moment it is written, never refunded, and
        // absent altogether if the write never lands.
        let target = format!("attempt:{}", self.state.run.completed_attempts);
        if let Some(reservation) =
            self.begin_operation(OperationKind::Reservation, "attempt", &target)?
        {
            self.settle_operation(&reservation, OperationOutcome::Completed)?;
        }

        Ok(PreCheckResult::Continue)
    }

    /// Run post-iteration checks. Detects whether a commit was made.
    pub fn post_iteration_check(&mut self) -> PostCheckResult {
        let current_sha = git_head_sha(&self.project_dir);

        let commit_made = match (&self.state.last_commit_sha, &current_sha) {
            (Some(before), Some(after)) => before != after,
            (None, Some(_)) => true, // First commit in repo
            _ => false,
        };
        self.state.run.record_commit(commit_made);

        if commit_made {
            self.state.last_status = IterationStatus::Committed;
            PostCheckResult::CommitMade {
                new_sha: current_sha.unwrap_or_default(),
            }
        } else {
            self.state.last_status = IterationStatus::NoCommit;
            PostCheckResult::NoCommit {
                guidance: format!(
                    "The previous iteration did not produce a commit. \
                     This is attempt {} of {}. Please ensure you commit your changes \
                     at the end of the iteration. If you're stuck, try a different approach.",
                    self.state.run.consecutive_no_commits,
                    RunState::MAX_NO_COMMITS
                ),
            }
        }
    }

    /// Advance to the next iteration. Call after post-iteration check.
    pub fn advance(&mut self) {
        self.state.run.advance_attempt();
        self.state.iteration = self.state.run.completed_attempts;
        self.state.attempt_started = None;
    }

    /// Whether the loop should stop due to repeated no-commit failures.
    #[must_use]
    pub fn should_stop_no_commits(&self) -> bool {
        self.state.run.is_no_progress_exhausted()
    }

    /// Mark the current iteration as failed (agent error).
    pub fn mark_failed(&mut self) {
        self.state.last_status = IterationStatus::Failed;
        self.stop(StopReason::Failed);
    }

    /// Persist state to `.magistr/loop-state.json`, atomically: a crash at
    /// any instant leaves the previous checkpoint or this one, never a torn
    /// file. Call it after writing an operation record and before its effect.
    pub fn persist(&self) -> Result<()> {
        validate_state(&self.state).map_err(recovery_error)?;
        let state_path = loop_state_path(&self.project_dir);
        let json = serde_json::to_string_pretty(&self.state)
            .map_err(|e| MagistrError::parse("JSON", "loop-state.json", e))?;
        durable_write(&state_path, json.as_bytes()).map_err(|e| MagistrError::io(&state_path, e))
    }

    /// Remove the persisted state file (cleanup after loop completes).
    pub fn cleanup(&self) -> Result<()> {
        let state_path = loop_state_path(&self.project_dir);
        if state_path.exists() {
            std::fs::remove_file(&state_path).map_err(|e| MagistrError::io(&state_path, e))?;
        }
        Ok(())
    }

    /// Check if a persisted state file exists (for crash recovery detection).
    pub fn has_persisted_state(project_dir: &Path) -> bool {
        loop_state_path(project_dir).is_file()
    }

    // ── Accessors ───────────────────────────────────────────────────────

    /// Current iteration number (0-based).
    #[must_use]
    pub fn iteration(&self) -> u32 {
        self.state.run.completed_attempts
    }

    /// Maximum iterations allowed.
    #[must_use]
    pub fn max_iterations(&self) -> u32 {
        self.state.run.max_iterations
    }

    /// Current iteration for display (1-based).
    #[must_use]
    pub fn display_iteration(&self) -> u32 {
        self.state.run.completed_attempts.saturating_add(1)
    }

    /// Reference to the full persisted state.
    #[must_use]
    pub fn state(&self) -> &IterationState {
        &self.state
    }

    /// Get the current spec summary.
    ///
    /// Native projects: re-read the roadmap (the agent's `magistr mcp-serve`
    /// wrote it) and report chunk progress, with the next ready chunk as the
    /// single remaining item — `all_done()` then means "no ready chunk".
    /// Otherwise prefers YAML store data, falls back to markdown parsing.
    pub fn spec_summary(&self) -> Result<SpecSummary> {
        if let Some(engines) = &self.native {
            return Ok(super::native_summary::native_spec_summary(engines));
        }
        spec::spec_summary_from_project(&self.project_dir, &self.spec_path)
    }

    /// Project directory.
    #[must_use]
    pub fn project_dir(&self) -> &Path {
        &self.project_dir
    }

    /// Last commit SHA.
    #[must_use]
    pub fn last_commit_sha(&self) -> Option<&str> {
        self.state.last_commit_sha.as_deref()
    }

    /// The status of the last completed iteration.
    #[must_use]
    pub fn last_status(&self) -> IterationStatus {
        self.state.last_status
    }

    /// Number of consecutive iterations without a commit.
    #[must_use]
    pub fn consecutive_no_commits(&self) -> u32 {
        self.state.run.consecutive_no_commits
    }
}

// ── Helpers ─────────────────────────────────────────────────────────────────

fn pre_check_stop(reason: StopReason) -> PreCheckResult {
    match reason {
        StopReason::IterationLimit => PreCheckResult::MaxIterations,
        StopReason::SpecComplete => PreCheckResult::SpecComplete,
        StopReason::NoReadyChunk => PreCheckResult::NoReadyChunk,
        reason => PreCheckResult::Stopped(reason),
    }
}

fn required_option<'de, D, T>(deserializer: D) -> std::result::Result<Option<T>, D::Error>
where
    D: serde::Deserializer<'de>,
    T: Deserialize<'de>,
{
    Option::<T>::deserialize(deserializer)
}

fn validate_state(state: &IterationState) -> std::result::Result<(), &'static str> {
    state.run.validate()?;
    state.operations.validate()?;
    if state.iteration != state.run.completed_attempts
        || state.max_iterations != state.run.max_iterations
    {
        return Err("legacy iteration projections disagree with canonical run state");
    }
    if state
        .attempt_started
        .is_some_and(|attempt| attempt != state.run.completed_attempts)
    {
        return Err("Git baseline belongs to a different attempt than the run cursor");
    }
    Ok(())
}

fn recovery_error(cause: impl std::fmt::Display) -> MagistrError {
    MagistrError::Workflow(format!(
        "Cannot recover loop-state.json: {cause}. Preserve or remove .magistr/loop-state.json \
         explicitly to start a new run; missing stopping evidence cannot be inferred."
    ))
}

/// The loop-state DTO historically spelled `Committed` as `success`. The
/// canonical type remains shared while this file keeps its established wire value.
mod legacy_iteration_status {
    use super::IterationStatus;
    use serde::{Deserialize, Deserializer, Serializer};

    pub(super) fn serialize<S>(status: &IterationStatus, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        match status {
            IterationStatus::Committed => serializer.serialize_str("success"),
            status => serde::Serialize::serialize(status, serializer),
        }
    }

    pub(super) fn deserialize<'de, D>(deserializer: D) -> Result<IterationStatus, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = String::deserialize(deserializer)?;
        if value == "success" {
            Ok(IterationStatus::Committed)
        } else {
            IterationStatus::deserialize(serde::de::value::StringDeserializer::new(value))
        }
    }
}

/// The run identity every operation record is scoped to: the native
/// project's own id when there is one, else the directory's name, with a
/// run key minted here from the clock and this process — the domain
/// allocates no identities.
fn mint_run_id(project_dir: &Path, native: Option<&NativeEngines>) -> Result<RunId> {
    let project = native
        .map(|engines| engines.project_id().to_owned())
        .or_else(|| {
            project_dir
                .file_name()
                .map(|name| name.to_string_lossy().into_owned())
        })
        .filter(|name| !name.is_empty())
        .unwrap_or_else(|| "project".to_owned());
    let millis = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|since| since.as_millis())
        .unwrap_or(0);
    ProjectId::new(project)
        .and_then(|project| RunId::new(project, format!("run-{millis}-{}", std::process::id())))
        .map_err(|error| MagistrError::Workflow(format!("cannot name the run: {error}")))
}

/// Path to the loop state file within the project's `.magistr/` directory.
fn loop_state_path(project_dir: &Path) -> PathBuf {
    project_dir.join(".magistr").join("loop-state.json")
}

/// Get the current git HEAD as a short SHA.
fn git_head_sha(project_dir: &Path) -> Option<String> {
    let output = std::process::Command::new("git")
        .args(["rev-parse", "--short", "HEAD"])
        .current_dir(project_dir)
        .output()
        .ok()?;

    if output.status.success() {
        Some(String::from_utf8_lossy(&output.stdout).trim().to_string())
    } else {
        None
    }
}

// ── Tests ───────────────────────────────────────────────────────────────────

/// The admitted host writer for a project whose `project.json` resolves to
/// think-and-ship (`None` on a yaml or spec-file project). Admits on the
/// store the environment names — prints load banners, so call it before
/// raw mode — and refuses loudly when another host holds the project.
fn auto_native(project_dir: &Path) -> Result<Option<NativeEngines>> {
    use magistr_engine::{ProjectAuthority, WriterRole};
    if !crate::roadmap::backend_for(project_dir).is_native() {
        return Ok(None);
    }
    let config = crate::evidence::writer_config(project_dir, |key| std::env::var(key).ok());
    ProjectAuthority::admit(&config, WriterRole::Host)
        .map(|authority| Some(authority.open()))
        .map_err(|source| MagistrError::io(project_dir, source))
}

#[cfg(test)]
#[path = "iteration.tests.rs"]
pub(super) mod tests;
