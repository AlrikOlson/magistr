use serde::{Deserialize, Serialize};

use super::budget_period::BudgetPeriod;
use super::budget_scope::BudgetScope;

/// One cap in a [`BudgetPolicy`]: how much a scope may spend per window.
///
/// A rule with neither threshold set is inert — it is kept rather than
/// rejected so a roster can carry a placeholder a team fills in later.
///
/// [`BudgetPolicy`]: super::budget_policy::BudgetPolicy
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BudgetRule {
    /// The level this cap constrains.
    pub scope: BudgetScope,
    /// The window the cap resets over.
    #[serde(default)]
    pub period: BudgetPeriod,
    /// Hard spending limit in USD for the scope's window.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub limit: Option<f64>,
    /// Warning threshold in USD for the scope's window.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub warn_threshold: Option<f64>,
}

impl BudgetRule {
    /// A rule for a scope and window with no thresholds set yet.
    #[must_use]
    pub fn new(scope: BudgetScope, period: BudgetPeriod) -> Self {
        Self {
            scope,
            period,
            limit: None,
            warn_threshold: None,
        }
    }

    /// Set the hard limit (self-consuming builder).
    #[must_use]
    pub fn limit(mut self, limit: f64) -> Self {
        self.limit = Some(limit);
        self
    }

    /// Set the warning threshold (self-consuming builder).
    #[must_use]
    pub fn warn_threshold(mut self, warn: f64) -> Self {
        self.warn_threshold = Some(warn);
        self
    }
}

#[cfg(test)]
#[path = "budget_rule.tests.rs"]
mod tests;
