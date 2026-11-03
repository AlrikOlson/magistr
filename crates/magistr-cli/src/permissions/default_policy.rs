//! Default [`PermissionPolicy`] implementation.
//!
//! Wraps the existing `evaluate()` function with a configured profile,
//! auto-allow rules, and check commands.

use super::auto_allow_rule::AutoAllowRule;
use super::permission_decision::PermissionDecision;
use super::permission_profile::PermissionProfile;
use super::policy::PermissionPolicy;

/// Default permission policy backed by a profile, rules, and check commands.
///
/// This is the production implementation of [`PermissionPolicy`].
pub struct DefaultPermissionPolicy {
    profile: PermissionProfile,
    auto_rules: Vec<AutoAllowRule>,
    check_commands: Vec<String>,
}

impl DefaultPermissionPolicy {
    /// Create a new policy with the given configuration.
    pub fn new(
        profile: PermissionProfile,
        auto_rules: Vec<AutoAllowRule>,
        check_commands: Vec<String>,
    ) -> Self {
        Self {
            profile,
            auto_rules,
            check_commands,
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
            &self.check_commands,
        )
    }
}

#[cfg(test)]
#[path = "default_policy.tests.rs"]
mod tests;
