//! Workflow guard that checks check results at `pre_commit`.

use crate::validate::ValidationResult;
use crate::workflow::guards::{GuardContext, GuardResult, WorkflowGuard};

/// Workflow guard that checks check results at `pre_commit`.
///
/// This guard does NOT run the checks itself (they're async). Instead, it
/// checks whether the orchestrator has populated check results. If checks
/// failed, it steers the agent with structured feedback.
///
/// The orchestrator is responsible for calling `CheckRunner::run_all()`
/// before invoking `GuardRunner::run_pre_commit()`, and storing the results
/// via `set_last_result()`.
pub struct CheckGuard {
    /// Last check results, set by the orchestrator after running checks.
    last_result: Option<ValidationResult>,
}

impl CheckGuard {
    /// Create a new guard with no results.
    pub fn new() -> Self {
        Self { last_result: None }
    }

    /// Set the last check results (called by the orchestrator after running checks).
    pub fn set_last_result(&mut self, result: ValidationResult) {
        self.last_result = Some(result);
    }

    /// Clear stored results.
    pub fn clear(&mut self) {
        self.last_result = None;
    }
}

impl Default for CheckGuard {
    fn default() -> Self {
        Self::new()
    }
}

impl WorkflowGuard for CheckGuard {
    fn name(&self) -> &str {
        "checks"
    }

    fn pre_commit(&self, _ctx: &GuardContext) -> GuardResult {
        match &self.last_result {
            Some(result) => {
                if result.all_passed() {
                    GuardResult::ok(self.name())
                } else {
                    let feedback =
                        feedback_for_agent(result).unwrap_or_else(|| "Checks failed.".into());
                    GuardResult::steer(self.name(), feedback)
                }
            }
            None => {
                // No results yet — checks haven't been run
                GuardResult::steer(
                    self.name(),
                    "Checks have not been run yet. Run all checks \
                     (check, test, lint) before committing.",
                )
            }
        }
    }
}

/// Generate structured feedback for the agent when checks fail.
///
/// Returns `None` if all checks passed. Returns a structured message
/// listing each failed check with its output, suitable for prompt steering.
#[must_use]
pub fn feedback_for_agent(result: &ValidationResult) -> Option<String> {
    if result.all_passed() {
        return None;
    }

    let mut feedback = String::from("Check failures detected. Fix these before committing:\n\n");

    for check_result in &result.results {
        if !check_result.passed {
            let output = if check_result.output.len() > 500 {
                &check_result.output[..500]
            } else {
                &check_result.output
            };
            feedback.push_str(&format!(
                "❌ Check `{}` failed (command: `{}`):\n```\n{}\n```\n\n",
                check_result.check.label(),
                check_result.command,
                output,
            ));
        }
    }

    feedback.push_str("Fix the failing checks and run them again before attempting to commit.");

    Some(feedback)
}

#[cfg(test)]
#[path = "check_guard.tests.rs"]
mod tests;
