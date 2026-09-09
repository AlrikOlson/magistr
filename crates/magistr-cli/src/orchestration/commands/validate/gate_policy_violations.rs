//! Enforce the org's `required_gates` over a gate run.

use magistr_core::validate::ValidationResult;
use magistr_engine::policy::GovernancePolicy;

/// The required gates this run did not satisfy.
///
/// A gate the project never configured reports as `passed: true` with
/// "skipped (no command configured)" — which is the hole this closes.
/// The policy is asked only about gates that actually ran a command and
/// came back green, so "we have no test gate" and "the test gate failed"
/// are the same answer to the org: not satisfied.
///
/// Returns empty when the policy names no required gates, or when it
/// permits a bypass — a bypass is a decision the roster makes, not one a
/// developer makes at the prompt.
#[must_use]
pub fn gate_policy_violations(policy: &GovernancePolicy, result: &ValidationResult) -> Vec<String> {
    if policy.allow_gate_bypass {
        return Vec::new();
    }
    let satisfied: Vec<&str> = result
        .results
        .iter()
        .filter(|gate| gate.passed && !gate.command.is_empty())
        .map(|gate| gate.gate.label())
        .collect();
    policy.missing_required_gates(&satisfied)
}

#[cfg(test)]
#[path = "gate_policy_violations.tests.rs"]
mod tests;
