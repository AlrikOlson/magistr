use alloc::string::String;

use serde::{Deserialize, Serialize};

use super::policy_condition::PolicyCondition;
use super::policy_effect::PolicyEffect;
use super::policy_request::PolicyRequest;

/// One governance rule: when `condition` holds, apply `effect`.
///
/// `override_allowed` is the escape hatch a file-protection rule needs to
/// stay usable — a `Deny` that can never be overridden turns into a
/// developer disabling the whole roster. An overridable rule still fires,
/// still records a violation, and still names itself in the audit log;
/// what it permits is a human deciding to proceed *on the record*.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PolicyRule {
    /// Stable id, recorded in the audit log and the compliance report.
    pub id: String,
    /// What this rule is for, in one line.
    #[serde(default)]
    pub description: String,
    /// When the rule applies.
    pub condition: PolicyCondition,
    /// What it does when it applies.
    pub effect: PolicyEffect,
    /// Whether a human may proceed past this rule, on the record.
    #[serde(default)]
    pub override_allowed: bool,
}

impl PolicyRule {
    /// A rule with the given id, condition and effect.
    #[must_use]
    pub fn new(id: impl Into<String>, condition: PolicyCondition, effect: PolicyEffect) -> Self {
        Self {
            id: id.into(),
            description: String::new(),
            condition,
            effect,
            override_allowed: false,
        }
    }

    /// Set the description (self-consuming builder).
    #[must_use]
    pub fn describe(mut self, description: impl Into<String>) -> Self {
        self.description = description.into();
        self
    }

    /// Let a human proceed past this rule on the record.
    #[must_use]
    pub fn overridable(mut self) -> Self {
        self.override_allowed = true;
        self
    }

    /// Whether this rule applies to `request`.
    #[must_use]
    pub fn matches(&self, request: &PolicyRequest) -> bool {
        self.condition.matches(request)
    }
}

#[cfg(test)]
#[path = "policy_rule.tests.rs"]
mod tests;
