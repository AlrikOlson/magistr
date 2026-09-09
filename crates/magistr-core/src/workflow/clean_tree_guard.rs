//! Clean tree guard — checks for uncommitted changes after a session.

use std::path::PathBuf;
use std::process::Command;

use crate::workflow::guards::{GuardContext, GuardResult, WorkflowGuard};

/// Checks for uncommitted changes after a session.
///
/// Uses `git status --porcelain` to detect dirty working tree. Warns but
/// doesn't block — the agent will be prompted to commit.
pub struct CleanTreeGuard;

impl CleanTreeGuard {
    /// Check if the git working tree has uncommitted changes.
    fn has_uncommitted_changes(project_dir: &std::path::Path) -> bool {
        Command::new("git")
            .args(["status", "--porcelain"])
            .current_dir(project_dir)
            .output()
            .map(|o| !o.stdout.is_empty())
            .unwrap_or(false)
    }
}

impl WorkflowGuard for CleanTreeGuard {
    fn name(&self) -> &str {
        "clean_tree"
    }

    fn post_session(&self, ctx: &GuardContext) -> GuardResult {
        let project_dir = PathBuf::from(&ctx.project_dir);
        if Self::has_uncommitted_changes(&project_dir) {
            GuardResult::warn(
                self.name(),
                "Uncommitted changes detected. Commit your work before finishing \
                 the iteration.",
            )
        } else {
            GuardResult::ok(self.name())
        }
    }
}
