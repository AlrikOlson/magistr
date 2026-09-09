//! Workflow guard that checks quality gate results at `pre_commit`.

use crate::validate::ValidationResult;
use crate::workflow::guards::{GuardContext, GuardResult, WorkflowGuard};

/// Workflow guard that checks quality gate results at `pre_commit`.
///
/// This guard does NOT run the gates itself (they're async). Instead, it
/// checks whether the orchestrator has populated gate results. If gates
/// failed, it steers the agent with structured feedback.
///
/// The orchestrator is responsible for calling `QualityGateRunner::run_all()`
/// before invoking `GuardRunner::run_pre_commit()`, and storing the results
/// via `set_last_result()`.
pub struct QualityGateGuard {
    /// Last gate results, set by the orchestrator after running gates.
    last_result: Option<ValidationResult>,
}

impl QualityGateGuard {
    /// Create a new guard with no results.
    pub fn new() -> Self {
        Self { last_result: None }
    }

    /// Set the last gate results (called by the orchestrator after running gates).
    pub fn set_last_result(&mut self, result: ValidationResult) {
        self.last_result = Some(result);
    }

    /// Clear stored results.
    pub fn clear(&mut self) {
        self.last_result = None;
    }
}

impl Default for QualityGateGuard {
    fn default() -> Self {
        Self::new()
    }
}

impl WorkflowGuard for QualityGateGuard {
    fn name(&self) -> &str {
        "quality_gate"
    }

    fn pre_commit(&self, _ctx: &GuardContext) -> GuardResult {
        match &self.last_result {
            Some(result) => {
                if result.all_passed() {
                    GuardResult::ok(self.name())
                } else {
                    let feedback = feedback_for_agent(result)
                        .unwrap_or_else(|| "Quality gates failed.".into());
                    GuardResult::steer(self.name(), feedback)
                }
            }
            None => {
                // No results yet — gates haven't been run
                GuardResult::steer(
                    self.name(),
                    "Quality gates have not been run yet. Run all quality gates \
                     (check, test, lint) before committing.",
                )
            }
        }
    }
}

/// Generate structured feedback for the agent when gates fail.
///
/// Returns `None` if all gates passed. Returns a structured message
/// listing each failed gate with its output, suitable for prompt steering.
#[must_use]
pub fn feedback_for_agent(result: &ValidationResult) -> Option<String> {
    if result.all_passed() {
        return None;
    }

    let mut feedback =
        String::from("Quality gate failures detected. Fix these before committing:\n\n");

    for gate_result in &result.results {
        if !gate_result.passed {
            let output = if gate_result.output.len() > 500 {
                &gate_result.output[..500]
            } else {
                &gate_result.output
            };
            feedback.push_str(&format!(
                "❌ Quality gate `{}` failed (command: `{}`):\n```\n{}\n```\n\n",
                gate_result.gate.label(),
                gate_result.command,
                output,
            ));
        }
    }

    feedback.push_str("Fix the failing gates and run them again before attempting to commit.");

    Some(feedback)
}

#[cfg(test)]
#[path = "gate_guard.tests.rs"]
mod tests;
