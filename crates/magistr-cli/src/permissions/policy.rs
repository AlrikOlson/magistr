//! Trait abstraction for permission evaluation.
//!
//! Enables dependency injection and testability by decoupling permission
//! logic from the concrete profile/rules/gate-command configuration.

use super::permission_decision::PermissionDecision;

/// Evaluates whether a tool call should be allowed, denied, or prompted.
///
/// The default implementation ([`DefaultPermissionPolicy`](super::DefaultPermissionPolicy))
/// delegates to the existing `evaluate()` function with a configured profile,
/// rules, and gate commands. Alternative implementations enable testing
/// permission-dependent code without real config.
pub trait PermissionPolicy: Send + Sync {
    /// Evaluate whether a tool call should be allowed, denied, or prompted.
    fn evaluate(&self, tool_name: &str, tool_input: &serde_json::Value) -> PermissionDecision;
}

#[cfg(test)]
#[path = "policy.tests.rs"]
mod tests;
