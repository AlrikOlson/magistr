//! Enforce the org's `required_checks` over a check run.

use magistr_core::validate::ValidationResult;
use magistr_engine::policy::GovernancePolicy;

/// The required checks this run did not satisfy.
///
/// A check the project never configured reports as `passed: true` with
/// "skipped (no command configured)" — which is the hole this closes.
/// The policy is asked only about checks that actually ran a command and
/// came back green, so "we have no test check" and "the test check failed"
/// are the same answer to the org: not satisfied.
///
/// Returns empty when the policy names no required checks, or when it
/// permits a bypass — a bypass is a decision the roster makes, not one a
/// developer makes at the prompt.
#[must_use]
pub fn check_policy_violations(
    policy: &GovernancePolicy,
    result: &ValidationResult,
) -> Vec<String> {
    if policy.allow_check_bypass {
        return Vec::new();
    }
    let satisfied: Vec<&str> = result
        .results
        .iter()
        .filter(|check| check.passed && !check.command.is_empty())
        .map(|check| check.check.label())
        .collect();
    policy.missing_required_checks(&satisfied)
}

#[cfg(test)]
#[path = "check_policy_violations.tests.rs"]
mod tests;
