//! Automatic rollback on catastrophic failure.
//!
//! Detects when quality gates go from passing to failing during a workflow
//! iteration and rolls back changes using `git stash` (safe, reversible).
//!
//! The rollback flow:
//! 1. Before each iteration, capture baseline gate status (pass/fail)
//! 2. After agent makes changes, run gates again
//! 3. If gates went from passing → failing, stash all changes
//! 4. Report the rollback with details on what failed

use std::path::{Path, PathBuf};
use std::process::Command;

use crate::workflow::rollback_result::RollbackResult;

/// Tracks quality gate health across an iteration for rollback detection.
#[derive(Debug, Clone)]
pub struct RollbackGuard {
    /// Project directory.
    project_dir: PathBuf,
    /// Whether quality gates were passing at the start of this iteration.
    baseline_passing: bool,
    /// Git HEAD at the start of the iteration (for commit-level rollback).
    pub baseline_head: Option<String>,
    /// Whether a rollback has been performed this iteration.
    rolled_back: bool,
}

impl RollbackGuard {
    /// Create a new rollback guard for a project directory.
    ///
    /// Captures the current HEAD sha and baseline gate status.
    pub fn new(project_dir: &Path, baseline_passing: bool) -> Self {
        let baseline_head = git_head(project_dir);
        Self {
            project_dir: project_dir.to_owned(),
            baseline_passing,
            baseline_head,
            rolled_back: false,
        }
    }

    /// Whether the baseline gates were passing when this guard was created.
    pub fn baseline_passing(&self) -> bool {
        self.baseline_passing
    }

    /// Whether a rollback has already been performed.
    pub fn is_rolled_back(&self) -> bool {
        self.rolled_back
    }

    /// Check if a catastrophic failure occurred and roll back if needed.
    ///
    /// A catastrophic failure is when:
    /// - Quality gates were passing at baseline (`baseline_passing == true`)
    /// - Quality gates are now failing (`current_passing == false`)
    ///
    /// If detected, performs `git stash` to save and revert all working tree changes.
    pub fn check_and_rollback(&mut self, current_passing: bool) -> RollbackResult {
        // Only roll back if gates went from passing → failing
        if !self.baseline_passing || current_passing || self.rolled_back {
            return RollbackResult::NotNeeded;
        }

        // Count dirty files before stashing
        let files_affected = count_dirty_files(&self.project_dir);
        if files_affected == 0 {
            return RollbackResult::NotNeeded;
        }

        // Stash all changes (tracked + untracked)
        match git_stash(&self.project_dir) {
            Ok(stash_ref) => {
                self.rolled_back = true;
                RollbackResult::Stashed {
                    stash_ref,
                    files_affected,
                    failure_reason: "Quality gates went from passing to failing".to_string(),
                }
            }
            Err(e) => RollbackResult::Failed(e),
        }
    }
}

/// Get the current git HEAD sha.
fn git_head(project_dir: &Path) -> Option<String> {
    Command::new("git")
        .args(["rev-parse", "HEAD"])
        .current_dir(project_dir)
        .output()
        .ok()
        .filter(|o| o.status.success())
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
}

/// Count the number of dirty (modified/added/deleted) files in the working tree.
fn count_dirty_files(project_dir: &Path) -> u32 {
    Command::new("git")
        .args(["status", "--porcelain"])
        .current_dir(project_dir)
        .output()
        .ok()
        .map(|o| {
            String::from_utf8_lossy(&o.stdout)
                .lines()
                .filter(|l| !l.is_empty())
                .count() as u32
        })
        .unwrap_or(0)
}

/// Stash all working tree changes (tracked and untracked).
///
/// Returns the stash reference on success.
fn git_stash(project_dir: &Path) -> Result<String, String> {
    let output = Command::new("git")
        .args([
            "stash",
            "push",
            "-u",
            "-m",
            "magistr: automatic rollback — quality gates failed",
        ])
        .current_dir(project_dir)
        .output()
        .map_err(|e| format!("failed to run git stash: {e}"))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("git stash failed: {stderr}"));
    }

    // The stash ref is always stash@{0} after a successful push
    Ok("stash@{0}".to_string())
}

#[cfg(test)]
#[path = "rollback.tests.rs"]
mod tests;
