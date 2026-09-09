use super::budget_config::BudgetConfig;

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
    #[must_use]
    pub fn should_stop(&self) -> bool {
        matches!(self, BudgetStatus::Exceeded { .. })
    }

    /// Whether this status should trigger a warning notification.
    #[must_use]
    pub fn should_warn(&self) -> bool {
        matches!(self, BudgetStatus::Warning { .. })
    }
}

/// Check current spending against budget configuration.
///
/// Returns `Ok` if no budget is set or spending is within all thresholds.
#[must_use]
pub(crate) fn check_budget(spent: f64, config: &BudgetConfig) -> BudgetStatus {
    // Check hard limit first
    if let Some(limit) = config.limit
        && spent >= limit
    {
        return BudgetStatus::Exceeded {
            overage: spent - limit,
            limit,
        };
    }

    // Check warning threshold
    if let Some(warn) = config.warn_threshold
        && spent >= warn
    {
        let remaining = config.limit.map(|l| l - spent).unwrap_or(0.0);
        return BudgetStatus::Warning {
            remaining,
            threshold: warn,
        };
    }

    BudgetStatus::Ok
}
