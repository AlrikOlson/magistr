use serde::{Deserialize, Serialize};

use super::budget_config::BudgetConfig;
use super::budget_rule::BudgetRule;
use super::resolved_budget::ResolvedBudget;
use super::scope_headroom::ScopeHeadroom;
use super::scope_spend::ScopeSpend;

/// The hierarchical budget policy a team publishes.
///
/// Resolution is deliberately arithmetic rather than procedural: the
/// effective limit is the *minimum* of the local budget and every scope's
/// remaining headroom, so a local `--budget` flag or a user-config default
/// can only ever tighten what the policy allows. There is no check to
/// forget and no override to find — a developer editing their own config
/// cannot raise a cap the roster set.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct BudgetPolicy {
    /// The caps, in whatever order the roster listed them.
    pub rules: Vec<BudgetRule>,
}

impl BudgetPolicy {
    /// A policy over the given rules.
    #[must_use]
    pub fn new(rules: Vec<BudgetRule>) -> Self {
        Self { rules }
    }

    /// Whether the policy carries any rule at all.
    #[must_use]
    pub fn is_set(&self) -> bool {
        !self.rules.is_empty()
    }

    /// Resolve the policy against a local budget and what each scope has
    /// already spent in its window.
    ///
    /// Ties go to the local budget: when a scope's headroom equals the
    /// local limit exactly, nothing was taken away, so no scope is named
    /// as the binding constraint.
    #[must_use]
    pub fn resolve(&self, local: &BudgetConfig, spent: &ScopeSpend) -> ResolvedBudget {
        let mut effective = local.clone();
        let mut binding = None;
        let mut headroom = Vec::new();

        let mut rules: Vec<&BudgetRule> = self.rules.iter().collect();
        rules.sort_by_key(|r| (r.scope, r.period));

        for rule in rules {
            let already = spent
                .get(&(rule.scope, rule.period))
                .copied()
                .unwrap_or(0.0);

            if let Some(limit) = rule.limit {
                let room = ScopeHeadroom::new(rule.scope, rule.period, limit, already);
                if effective
                    .limit
                    .is_none_or(|current| room.remaining < current)
                {
                    effective.limit = Some(room.remaining);
                    binding = Some(rule.scope);
                }
                headroom.push(room);
            }

            if let Some(warn) = rule.warn_threshold {
                let to_warn = (warn - already).max(0.0);
                if effective
                    .warn_threshold
                    .is_none_or(|current| to_warn < current)
                {
                    effective.warn_threshold = Some(to_warn);
                }
            }
        }

        ResolvedBudget {
            effective,
            binding,
            headroom,
        }
    }
}

#[cfg(test)]
#[path = "budget_policy.tests.rs"]
mod tests;
