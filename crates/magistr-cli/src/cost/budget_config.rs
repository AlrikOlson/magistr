//! Budget configuration for a session.

use serde::{Deserialize, Serialize};

/// Budget configuration for a session.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct BudgetConfig {
    /// Hard spending limit in USD. Session stops when exceeded.
    pub limit: Option<f64>,
    /// Warning threshold in USD. Toast shown when reached.
    pub warn_threshold: Option<f64>,
}

impl BudgetConfig {
    /// No budget constraints.
    #[must_use]
    pub fn none() -> Self {
        Self {
            limit: None,
            warn_threshold: None,
        }
    }

    /// Create a budget with a hard limit.
    #[must_use]
    pub fn with_limit(limit: f64) -> Self {
        Self {
            limit: Some(limit),
            warn_threshold: None,
        }
    }

    /// Create a budget with both limit and warning threshold.
    #[must_use]
    pub fn with_limit_and_warn(limit: f64, warn: f64) -> Self {
        Self {
            limit: Some(limit),
            warn_threshold: Some(warn),
        }
    }

    /// Set the hard spending limit (self-consuming builder).
    #[must_use]
    pub fn limit(mut self, amount: f64) -> Self {
        self.limit = Some(amount);
        self
    }

    /// Set the warning threshold (self-consuming builder).
    #[must_use]
    pub fn warn_threshold(mut self, amount: f64) -> Self {
        self.warn_threshold = Some(amount);
        self
    }

    /// Whether any budget constraint is set.
    pub fn is_set(&self) -> bool {
        self.limit.is_some() || self.warn_threshold.is_some()
    }

    /// Merge CLI flags with persistent defaults from user config.
    ///
    /// CLI flags take precedence over config file defaults.
    /// If a CLI flag is `Some`, it wins. Otherwise, the config default is used.
    pub fn from_cli_and_defaults(
        cli_limit: Option<f64>,
        cli_warn: Option<f64>,
        config_limit: Option<f64>,
        config_warn: Option<f64>,
    ) -> Self {
        Self {
            limit: cli_limit.or(config_limit),
            warn_threshold: cli_warn.or(config_warn),
        }
    }
}

impl Default for BudgetConfig {
    fn default() -> Self {
        Self::none()
    }
}
