//! Spec guard — checks if the spec file was modified after a session.

use std::path::{Path, PathBuf};
use std::process::Command;

use crate::workflow::guards::{GuardContext, GuardResult, WorkflowGuard};

/// Checks if the spec file was modified in the working tree after a session.
///
/// Uses `git diff --name-only` to detect modifications. If the spec file
/// hasn't been touched, prompts the agent to update it.
pub struct SpecGuard;

impl SpecGuard {
    /// Check if a file has been modified in the git working tree.
    fn is_file_modified(project_dir: &Path, file_path: &Path) -> bool {
        let relative = file_path.strip_prefix(project_dir).unwrap_or(file_path);
        let relative_str = relative.to_string_lossy();

        // Check unstaged changes
        let unstaged = Command::new("git")
            .args(["diff", "--name-only"])
            .current_dir(project_dir)
            .output()
            .map(|o| {
                String::from_utf8_lossy(&o.stdout)
                    .lines()
                    .any(|line| line.trim() == relative_str)
            })
            .unwrap_or(false);

        if unstaged {
            return true;
        }

        // Check staged changes
        Command::new("git")
            .args(["diff", "--cached", "--name-only"])
            .current_dir(project_dir)
            .output()
            .map(|o| {
                String::from_utf8_lossy(&o.stdout)
                    .lines()
                    .any(|line| line.trim() == relative_str)
            })
            .unwrap_or(false)
    }
}

impl WorkflowGuard for SpecGuard {
    fn name(&self) -> &str {
        "spec"
    }

    fn post_session(&self, ctx: &GuardContext) -> GuardResult {
        let project_dir = PathBuf::from(&ctx.project_dir);
        let spec_path = PathBuf::from(&ctx.spec_path);
        if Self::is_file_modified(&project_dir, &spec_path) {
            GuardResult::ok(self.name())
        } else {
            let spec_name = spec_path
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_else(|| "spec file".into());
            GuardResult::steer(
                self.name(),
                format!(
                    "Update {spec_name} before committing. Check off completed items \
                     and add any new items discovered during implementation."
                ),
            )
        }
    }
}
