//! Iteration loop orchestration for `magistr next-loop`.
//!
//! Manages iteration state (count, max, last commit SHA, spec status) with
//! JSON persistence to `.magistr/loop-state.json` for crash recovery. Replaces
//! the ad-hoc `LoopState` in `plan.rs` and `/tmp`-based state files.
//!
//! The `IterationLoop` is backend-agnostic — it manages state and checks,
//! while the caller (TUI or CLI) manages agent sessions.

use std::path::{Path, PathBuf};

use magistr_engine::NativeEngines;
use serde::{Deserialize, Serialize};

use crate::error::{MagistrError, Result};
use crate::types::ProjectConfig;
use crate::workflow::spec::{self, SpecSummary};

// ── Persisted state ─────────────────────────────────────────────────────────

/// JSON-serializable iteration state, persisted to `.magistr/loop-state.json`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct IterationState {
    /// Current iteration number (0-based internally, displayed as 1-based).
    pub iteration: u32,
    /// Maximum iterations allowed.
    pub max_iterations: u32,
    /// Git HEAD SHA before the current iteration started.
    /// Used to detect whether the iteration produced a commit.
    pub last_commit_sha: Option<String>,
    /// Status of the last completed iteration.
    pub last_status: IterationStatus,
    /// Spec progress: number of checked items at last check.
    pub spec_checked: usize,
    /// Spec progress: total number of items at last check.
    pub spec_total: usize,
}

/// Status of a completed iteration.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum IterationStatus {
    /// No iteration has completed yet.
    None,
    /// Iteration completed successfully (commit was made).
    Success,
    /// Iteration completed but no commit was made.
    NoCommit,
    /// Iteration failed (agent error).
    Failed,
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
    /// Number of consecutive no-commit iterations.
    consecutive_no_commits: u32,
    /// The in-process think-and-ship engines when the project runs the
    /// native roadmap: the next chunk comes from `roadmap_next` and progress
    /// from the roadmap, not from the spec file.
    native: Option<NativeEngines>,
}

/// Maximum consecutive iterations without a commit before auto-stopping.
const MAX_NO_COMMIT_RETRIES: u32 = 2;

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
    /// `native` carries the think-and-ship engines a caller opened BEFORE
    /// entering raw mode. With `None`, the backend is resolved from
    /// `.magistr/project.json` (`roadmap::backend_for`): a native project
    /// opens its engines here and takes its progress from the roadmap; a
    /// yaml project reads `spec_path` as it always did.
    pub fn open(
        project_dir: impl Into<PathBuf>,
        spec_path: impl Into<PathBuf>,
        max_iterations: u32,
        native: Option<NativeEngines>,
    ) -> Result<Self> {
        let project_dir = project_dir.into();
        let spec_path = spec_path.into();
        let native = native.or_else(|| auto_native(&project_dir));

        let summary = match &native {
            Some(engines) => super::native_summary::native_spec_summary(engines),
            None => spec::spec_summary_from_project(&project_dir, &spec_path)?,
        };

        let state = IterationState {
            iteration: 0,
            max_iterations,
            last_commit_sha: git_head_sha(&project_dir),
            last_status: IterationStatus::None,
            spec_checked: summary.checked,
            spec_total: summary.total,
        };

        Ok(Self {
            state,
            project_dir,
            spec_path,
            consecutive_no_commits: 0,
            native,
        })
    }

    /// Recover from a previously persisted state (crash recovery).
    ///
    /// Reads `.magistr/loop-state.json` and reconstructs the loop.
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
        let state: IterationState = serde_json::from_str(&content)
            .map_err(|e| MagistrError::parse("JSON", "loop-state.json", e))?;
        let native = native.or_else(|| auto_native(&project_dir));

        Ok(Self {
            state,
            project_dir,
            spec_path,
            consecutive_no_commits: 0,
            native,
        })
    }

    /// Drive the loop from the think-and-ship roadmap (`None` keeps the
    /// spec-file behaviour). Open the engines BEFORE entering raw mode.
    #[must_use]
    pub fn with_native(mut self, native: Option<NativeEngines>) -> Self {
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
        // Check max iterations
        if self.state.iteration >= self.state.max_iterations {
            return Ok(PreCheckResult::MaxIterations);
        }

        // Check if all spec items are done
        let summary = self.spec_summary()?;
        self.state.spec_checked = summary.checked;
        self.state.spec_total = summary.total;

        if summary.all_done() {
            return Ok(if self.native.is_some() {
                PreCheckResult::NoReadyChunk
            } else {
                PreCheckResult::SpecComplete
            });
        }

        // Capture current HEAD for post-iteration commit detection
        self.state.last_commit_sha = git_head_sha(&self.project_dir);

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

        if commit_made {
            self.state.last_status = IterationStatus::Success;
            self.consecutive_no_commits = 0;
            PostCheckResult::CommitMade {
                new_sha: current_sha.unwrap_or_default(),
            }
        } else {
            self.state.last_status = IterationStatus::NoCommit;
            self.consecutive_no_commits += 1;
            PostCheckResult::NoCommit {
                guidance: format!(
                    "The previous iteration did not produce a commit. \
                     This is attempt {} of {}. Please ensure you commit your changes \
                     at the end of the iteration. If you're stuck, try a different approach.",
                    self.consecutive_no_commits, MAX_NO_COMMIT_RETRIES
                ),
            }
        }
    }

    /// Advance to the next iteration. Call after post-iteration check.
    pub fn advance(&mut self) {
        self.state.iteration += 1;
    }

    /// Whether the loop should stop due to repeated no-commit failures.
    #[must_use]
    pub fn should_stop_no_commits(&self) -> bool {
        self.consecutive_no_commits >= MAX_NO_COMMIT_RETRIES
    }

    /// Mark the current iteration as failed (agent error).
    pub fn mark_failed(&mut self) {
        self.state.last_status = IterationStatus::Failed;
    }

    /// Persist state to `.magistr/loop-state.json`.
    pub fn persist(&self) -> Result<()> {
        let state_path = loop_state_path(&self.project_dir);

        // Ensure .magistr/ directory exists
        if let Some(parent) = state_path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| MagistrError::io(parent, e))?;
        }

        let json = serde_json::to_string_pretty(&self.state)
            .map_err(|e| MagistrError::parse("JSON", "loop-state.json", e))?;
        std::fs::write(&state_path, json).map_err(|e| MagistrError::io(&state_path, e))
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
        self.state.iteration
    }

    /// Maximum iterations allowed.
    #[must_use]
    pub fn max_iterations(&self) -> u32 {
        self.state.max_iterations
    }

    /// Current iteration for display (1-based).
    #[must_use]
    pub fn display_iteration(&self) -> u32 {
        self.state.iteration + 1
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
        self.consecutive_no_commits
    }
}

// ── Helpers ─────────────────────────────────────────────────────────────────

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

/// The engines for a project whose `project.json` resolves to think-and-ship
/// (`None` on a yaml or spec-file project). Opens on the store the
/// environment names — prints load banners, so call it before raw mode.
fn auto_native(project_dir: &Path) -> Option<NativeEngines> {
    use magistr_engine::TasConfig;
    crate::roadmap::backend_for(project_dir)
        .is_native()
        .then(|| NativeEngines::open(&TasConfig::from_env(project_dir)))
}

#[cfg(test)]
#[path = "iteration.tests.rs"]
mod tests;
