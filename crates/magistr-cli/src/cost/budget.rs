//! Budget status — result of checking spending against limits.

/// Result of checking current spending against budget constraints.
#[derive(Debug, Clone, PartialEq)]
pub enum BudgetStatus {
    /// Under budget, no concerns.
    Ok,
    /// Spending has reached the warning threshold but not the limit.
    Warning {
        /// Amount remaining before hitting the hard limit.
        remaining: f64,
        /// The warning threshold that was crossed.
        threshold: f64,
    },
    /// Hard budget limit has been exceeded.
    Exceeded {
        /// Amount over the limit.
        overage: f64,
        /// The configured limit.
        limit: f64,
    },
}

impl BudgetStatus {
    /// Whether this status indicates the session should stop.
    pub fn should_stop(&self) -> bool {
        matches!(self, BudgetStatus::Exceeded { .. })
    }

    /// Whether this status should trigger a warning notification.
    pub fn should_warn(&self) -> bool {
        matches!(self, BudgetStatus::Warning { .. })
    }
}

#[cfg(test)]
#[path = "budget.tests.rs"]
mod tests;
