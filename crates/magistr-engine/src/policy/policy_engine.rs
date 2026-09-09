use serde::{Deserialize, Serialize};

use super::policy_decision::PolicyDecision;
use super::policy_effect::PolicyEffect;
use super::policy_request::PolicyRequest;
use super::policy_rule::PolicyRule;

/// A compiled rule set, ready to evaluate requests.
///
/// Evaluation is a fold, not a search: every matching rule contributes
/// and the effect is their maximum. Nothing about rule order changes the
/// outcome, so there is no precedence to get wrong and no way to write a
/// rule that *un*-denies what an earlier one denied.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct PolicyEngine {
    /// The rules, in the order the roster listed them.
    pub rules: Vec<PolicyRule>,
}

impl PolicyEngine {
    /// An engine over the given rules.
    #[must_use]
    pub fn new(rules: Vec<PolicyRule>) -> Self {
        Self { rules }
    }

    /// Whether the engine carries any rule at all.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.rules.is_empty()
    }

    /// How many rules this engine holds.
    #[must_use]
    pub fn len(&self) -> usize {
        self.rules.len()
    }

    /// Evaluate `request` against every rule.
    #[must_use]
    pub fn evaluate(&self, request: &PolicyRequest) -> PolicyDecision {
        PolicyDecision::from_matches(self.rules.iter().filter(|rule| rule.matches(request)))
    }

    /// Tighten a caller's own decision with the policy's.
    ///
    /// This is the whole enforcement contract in one line: the result is
    /// the maximum of what the local permission profile decided and what
    /// the org's policy says. A local config can raise the floor and can
    /// never lower it — the property is arithmetic, so there is no check
    /// to forget and no ordering to get wrong.
    #[must_use]
    pub fn tighten(&self, local: PolicyEffect, request: &PolicyRequest) -> PolicyDecision {
        let mut decision = self.evaluate(request);
        decision.effect = decision.effect.tighten(local);
        if decision.binding.is_none() && decision.effect.is_violation() {
            decision.override_allowed = false;
        }
        decision
    }

    /// Merge another engine's rules in. Merging can only ever add rules,
    /// so a merged policy is at least as strict as either input.
    #[must_use]
    pub fn merged_with(mut self, other: &PolicyEngine) -> Self {
        self.rules.extend(other.rules.iter().cloned());
        self
    }
}

#[cfg(test)]
#[path = "policy_engine.tests.rs"]
mod tests;
