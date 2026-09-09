use alloc::{boxed::Box, string::String, vec::Vec};

use serde::{Deserialize, Serialize};

use super::glob_match::glob_match;
use super::policy_request::PolicyRequest;
use super::risk_tier::RiskTier;

/// When a policy rule applies.
///
/// The leaf variants match one field of a [`PolicyRequest`] against a
/// glob; the combinators (`any_of`, `all_of`, `not`) compose them, which
/// is what makes this a condition language rather than a flat matcher
/// list. A leaf whose field the request does not carry is `false` —
/// never `true` — so an under-populated request can never trip a rule by
/// accident.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum PolicyCondition {
    /// Matches every request — the way to write a blanket rule.
    Always,
    /// The tool name matches a glob.
    Tool { pattern: String },
    /// The shell command matches a glob.
    Command { pattern: String },
    /// Any path the request touches matches a glob.
    Path { pattern: String },
    /// The session's model matches a glob.
    Model { pattern: String },
    /// The gate name matches a glob.
    Gate { pattern: String },
    /// The request's own risk classification is at least this tier.
    RiskAtLeast { tier: RiskTier },
    /// At least one inner condition holds.
    AnyOf { conditions: Vec<PolicyCondition> },
    /// Every inner condition holds.
    AllOf { conditions: Vec<PolicyCondition> },
    /// The inner condition does not hold.
    Not { condition: Box<PolicyCondition> },
}

impl PolicyCondition {
    /// A condition on the tool name.
    #[must_use]
    pub fn tool(pattern: impl Into<String>) -> Self {
        PolicyCondition::Tool {
            pattern: pattern.into(),
        }
    }

    /// A condition on the shell command.
    #[must_use]
    pub fn command(pattern: impl Into<String>) -> Self {
        PolicyCondition::Command {
            pattern: pattern.into(),
        }
    }

    /// A condition on any touched path.
    #[must_use]
    pub fn path(pattern: impl Into<String>) -> Self {
        PolicyCondition::Path {
            pattern: pattern.into(),
        }
    }

    /// A condition on the model.
    #[must_use]
    pub fn model(pattern: impl Into<String>) -> Self {
        PolicyCondition::Model {
            pattern: pattern.into(),
        }
    }

    /// A condition on the gate name.
    #[must_use]
    pub fn gate(pattern: impl Into<String>) -> Self {
        PolicyCondition::Gate {
            pattern: pattern.into(),
        }
    }

    /// A condition on the risk tier.
    #[must_use]
    pub fn risk_at_least(tier: RiskTier) -> Self {
        PolicyCondition::RiskAtLeast { tier }
    }

    /// Disjunction. An empty list is `false` — nothing to satisfy.
    #[must_use]
    pub fn any_of(conditions: Vec<PolicyCondition>) -> Self {
        PolicyCondition::AnyOf { conditions }
    }

    /// Conjunction. An empty list is `true` — nothing to violate.
    #[must_use]
    pub fn all_of(conditions: Vec<PolicyCondition>) -> Self {
        PolicyCondition::AllOf { conditions }
    }

    /// Negation. Named `negate` rather than `not` so it cannot be
    /// mistaken for `core::ops::Not::not`, which this is not.
    #[must_use]
    pub fn negate(condition: PolicyCondition) -> Self {
        PolicyCondition::Not {
            condition: Box::new(condition),
        }
    }

    /// Whether this condition holds for `request`.
    #[must_use]
    pub fn matches(&self, request: &PolicyRequest) -> bool {
        match self {
            PolicyCondition::Always => true,
            PolicyCondition::Tool { pattern } => request
                .tool
                .as_deref()
                .is_some_and(|tool| glob_match(pattern, tool)),
            PolicyCondition::Command { pattern } => request
                .command
                .as_deref()
                .is_some_and(|command| glob_match(pattern, command)),
            PolicyCondition::Path { pattern } => {
                request.paths.iter().any(|path| glob_match(pattern, path))
            }
            PolicyCondition::Model { pattern } => request
                .model
                .as_deref()
                .is_some_and(|model| glob_match(pattern, model)),
            PolicyCondition::Gate { pattern } => request
                .gate
                .as_deref()
                .is_some_and(|gate| glob_match(pattern, gate)),
            PolicyCondition::RiskAtLeast { tier } => request.risk.is_some_and(|risk| risk >= *tier),
            PolicyCondition::AnyOf { conditions } => conditions.iter().any(|c| c.matches(request)),
            PolicyCondition::AllOf { conditions } => conditions.iter().all(|c| c.matches(request)),
            PolicyCondition::Not { condition } => !condition.matches(request),
        }
    }
}

#[cfg(test)]
#[path = "policy_condition.tests.rs"]
mod tests;
