use serde::{Deserialize, Serialize};

/// Budget configuration for a session.
///
/// Defines the spending constraints — a hard limit and/or a warning
/// threshold. Pure value object with no IO.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BudgetConfig {
    /// Hard spending limit in USD. Session stops when exceeded.
    pub limit: Option<f64>,
    /// Warning threshold in USD. Notification emitted when reached.
    pub warn_threshold: Option<f64>,
    /// Maximum iteration count. Loop stops when exceeded.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_iterations: Option<u32>,
    /// Maximum duration in seconds. Loop auto-pauses when exceeded.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_duration_secs: Option<u64>,
}

impl BudgetConfig {
    /// No budget constraints.
    #[must_use]
    pub fn none() -> Self {
        Self {
            limit: None,
            warn_threshold: None,
            max_iterations: None,
            max_duration_secs: None,
        }
    }

    /// Create a budget with a hard limit.
    #[must_use]
    pub fn with_limit(limit: f64) -> Self {
        Self {
            limit: Some(limit),
            warn_threshold: None,
            max_iterations: None,
            max_duration_secs: None,
        }
    }

    /// Create a budget with both limit and warning threshold.
    #[must_use]
    pub fn with_limit_and_warn(limit: f64, warn: f64) -> Self {
        Self {
            limit: Some(limit),
            warn_threshold: Some(warn),
            max_iterations: None,
            max_duration_secs: None,
        }
    }

    /// Set the hard spending limit (self-consuming builder).
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

    /// Set the maximum iteration count (self-consuming builder).
    #[must_use]
    pub fn max_iterations(mut self, n: u32) -> Self {
        self.max_iterations = Some(n);
        self
    }

    /// Set the maximum duration in seconds (self-consuming builder).
    #[must_use]
    pub fn max_duration_secs(mut self, secs: u64) -> Self {
        self.max_duration_secs = Some(secs);
        self
    }

    /// Whether any budget constraint is set.
    #[must_use]
    pub fn is_set(&self) -> bool {
        self.limit.is_some()
            || self.warn_threshold.is_some()
            || self.max_iterations.is_some()
            || self.max_duration_secs.is_some()
    }
}

#[cfg(test)]
#[path = "budget_config.tests.rs"]
mod tests;

impl Default for BudgetConfig {
    fn default() -> Self {
        Self::none()
    }
}
