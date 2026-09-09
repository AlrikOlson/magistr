//! Quality gate health check for rollback detection.

use std::path::Path;
use std::process::Command;

/// Detect whether quality gates are currently passing.
///
/// Runs the check, test, and lint commands. Returns `true` only if all pass.
pub fn gates_passing(project_dir: &Path, gate: &crate::types::QualityGate) -> bool {
    let commands = [&gate.check, &gate.test, &gate.lint];
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
