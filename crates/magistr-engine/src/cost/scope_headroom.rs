use serde::{Deserialize, Serialize};

use super::budget_period::BudgetPeriod;
use super::budget_scope::BudgetScope;

/// What one scope has left to spend in the current window.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ScopeHeadroom {
    /// The scope this headroom belongs to.
    pub scope: BudgetScope,
    /// The window the limit resets over.
    pub period: BudgetPeriod,
    /// The scope's configured limit for the window.
    pub limit: f64,
    /// What the scope has already spent inside the window.
    pub spent: f64,
    /// `limit - spent`, floored at zero.
    pub remaining: f64,
}

impl ScopeHeadroom {
    /// Compute the headroom a scope has left.
    #[must_use]
    pub fn new(scope: BudgetScope, period: BudgetPeriod, limit: f64, spent: f64) -> Self {
        Self {
            scope,
            period,
            limit,
            spent,
            remaining: (limit - spent).max(0.0),
        }
    }

    /// Whether the scope has nothing left for this window.
    #[must_use]
    pub fn is_exhausted(&self) -> bool {
        self.remaining <= 0.0
    }
}

#[cfg(test)]
#[path = "scope_headroom.tests.rs"]
mod tests;
