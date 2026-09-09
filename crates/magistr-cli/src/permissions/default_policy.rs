//! Default [`PermissionPolicy`] implementation.
//!
//! Wraps the existing `evaluate()` function with a configured profile,
//! auto-allow rules, and gate commands.

use super::auto_allow_rule::AutoAllowRule;
use super::permission_decision::PermissionDecision;
use super::permission_profile::PermissionProfile;
use super::policy::PermissionPolicy;

/// Default permission policy backed by a profile, rules, and gate commands.
///
/// This is the production implementation of [`PermissionPolicy`].
pub struct DefaultPermissionPolicy {
    profile: PermissionProfile,
    auto_rules: Vec<AutoAllowRule>,
    gate_commands: Vec<String>,
}

impl DefaultPermissionPolicy {
    /// Create a new policy with the given configuration.
    pub fn new(
        profile: PermissionProfile,
        auto_rules: Vec<AutoAllowRule>,
        gate_commands: Vec<String>,
    ) -> Self {
        Self {
            profile,
            auto_rules,
            gate_commands,
        }
    }
}

impl PermissionPolicy for DefaultPermissionPolicy {
    fn evaluate(&self, tool_name: &str, tool_input: &serde_json::Value) -> PermissionDecision {
        super::evaluate::evaluate(
            tool_name,
            tool_input,
            self.profile,
            &self.auto_rules,
            &self.gate_commands,
        )
    }
}

#[cfg(test)]
#[path = "default_policy.tests.rs"]
mod tests;
