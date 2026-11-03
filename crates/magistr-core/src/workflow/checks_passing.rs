//! Check health check for rollback detection.

use std::path::Path;
use std::process::Command;

/// Detect whether checks are currently passing.
///
/// Runs the check, test, and lint commands. Returns `true` only if all pass.
pub fn checks_passing(project_dir: &Path, check: &crate::types::StandardChecks) -> bool {
    let commands = [&check.check, &check.test, &check.lint];
    for cmd in &commands {
        if cmd.is_empty() {
            continue;
        }
        let result = Command::new("sh")
            .arg("-c")
            .arg(cmd)
            .current_dir(project_dir)
            .output();
        match result {
            Ok(output) if output.status.success() => {}
            _ => return false,
        }
    }
    true
}
