//! Budget tracking state (domain-only).
//!
//! Core budget configuration and status checking. No exceeded-screen overlays
//! or optimization hint analysis — those are frontend-specific concerns.

/// Domain-only budget state.
///
/// Frontend implementations extend this with presentation-specific fields
/// (exceeded screen overlay, optimization hints from cost analysis).
#[derive(Debug, Clone, Default)]
pub struct BudgetState {
    /// Hard spending limit in USD. `None` means no limit.
    pub limit: Option<f64>,
    /// Warning threshold in USD. `None` means no warning.
    pub warn_threshold: Option<f64>,
}

/// Budget check result.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum BudgetStatus {
    /// Spending is within limits.
    Ok,
    /// Spending exceeds the warning threshold but not the limit.
    Warning {
        /// Current spending as a fraction of the limit (0.0–1.0).
        ratio: f64,
    },
    /// Spending has exceeded the hard limit.
    Exceeded {
        /// How much over the limit (in USD).
        overage: f64,
    },
}

impl BudgetState {
    /// Set budget configuration from CLI flags.
    pub fn set_budget(&mut self, limit: Option<f64>, warn: Option<f64>) {
        self.limit = limit;
        self.warn_threshold = warn;
    }

    /// Check current spending against the budget.
    ///
    /// Returns [`BudgetStatus::Ok`] if no budget is set.
    #[must_use]
    pub fn check(&self, total_cost_usd: f64) -> BudgetStatus {
        let Some(limit) = self.limit else {
            return BudgetStatus::Ok;
        };

        if total_cost_usd >= limit {
            return BudgetStatus::Exceeded {
                overage: total_cost_usd - limit,
            };
        }

        if let Some(warn) = self.warn_threshold
            && total_cost_usd >= warn
        {
            return BudgetStatus::Warning {
                ratio: total_cost_usd / limit,
            };
        }

        BudgetStatus::Ok
    }
}

#[cfg(test)]
#[path = "budget_state.tests.rs"]
mod tests;
