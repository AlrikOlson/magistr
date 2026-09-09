//! Budget checking — compare spending against configured limits.

use super::budget::BudgetStatus;
use super::budget_config::BudgetConfig;

/// Check current spending against budget configuration.
///
/// Returns `Ok` if no budget is set or spending is within all thresholds.
pub fn check_budget(spent: f64, config: &BudgetConfig) -> BudgetStatus {
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
