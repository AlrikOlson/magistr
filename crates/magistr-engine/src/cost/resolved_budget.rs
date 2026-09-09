use serde::{Deserialize, Serialize};

use super::budget_config::BudgetConfig;
use super::budget_scope::BudgetScope;
use super::scope_headroom::ScopeHeadroom;

/// The outcome of resolving a [`BudgetPolicy`] against a local budget.
///
/// [`BudgetPolicy`]: super::budget_policy::BudgetPolicy
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ResolvedBudget {
    /// The budget the session actually runs under.
    pub effective: BudgetConfig,
    /// The scope whose headroom produced the effective limit, or `None`
    /// when the local budget was already the tighter of the two.
    pub binding: Option<BudgetScope>,
    /// Headroom for every scope that carries a limit, broadest first.
    pub headroom: Vec<ScopeHeadroom>,
}

impl ResolvedBudget {
    /// Whether any scope has nothing left for its window — the session
    /// cannot start without overspending someone's cap.
    #[must_use]
    pub fn is_exhausted(&self) -> bool {
        self.headroom.iter().any(ScopeHeadroom::is_exhausted)
    }

    /// Whether a policy constrained this budget at all.
    #[must_use]
    pub fn is_governed(&self) -> bool {
        !self.headroom.is_empty()
    }

    /// The scope with the least left to spend.
    #[must_use]
    pub fn tightest(&self) -> Option<&ScopeHeadroom> {
        self.headroom
            .iter()
            .min_by(|a, b| a.remaining.total_cmp(&b.remaining))
    }
}

#[cfg(test)]
#[path = "resolved_budget.tests.rs"]
mod tests;
