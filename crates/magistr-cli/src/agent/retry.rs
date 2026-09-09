//! Retry configuration for transient backend failures.

use std::time::Duration;

/// Configuration for retry with exponential backoff.
#[derive(Debug, Clone)]
pub struct RetryConfig {
    /// Maximum number of retry attempts (0 = no retries, just the initial attempt).
    pub max_retries: u32,
    /// Initial delay before the first retry.
    pub initial_delay: Duration,
    /// Maximum delay between retries (caps the exponential growth).
    pub max_delay: Duration,
    /// Multiplier applied to the delay after each retry.
    pub backoff_factor: f64,
}

impl Default for RetryConfig {
    fn default() -> Self {
        Self {
            max_retries: 3,
            initial_delay: Duration::from_millis(500),
            max_delay: Duration::from_secs(30),
            backoff_factor: 2.0,
        }
    }
}

impl RetryConfig {
    /// Create a retry config with the given max retries.
    #[must_use]
    pub fn new(max_retries: u32) -> Self {
        Self {
            max_retries,
            ..Default::default()
        }
    }

    /// Set the initial delay.
    #[must_use]
    pub fn initial_delay(mut self, delay: Duration) -> Self {
        self.initial_delay = delay;
        self
    }

    /// Set the maximum delay cap.
    #[must_use]
    pub fn max_delay(mut self, delay: Duration) -> Self {
        self.max_delay = delay;
        self
    }

    /// Set the backoff multiplier.
    #[must_use]
    pub fn backoff_factor(mut self, factor: f64) -> Self {
        self.backoff_factor = factor;
        self
    }

    /// Calculate the delay for a given attempt number (0-based).
    #[must_use]
    pub fn delay_for_attempt(&self, attempt: u32) -> Duration {
        let delay_ms =
            self.initial_delay.as_millis() as f64 * self.backoff_factor.powi(attempt as i32);
        let capped_ms = delay_ms.min(self.max_delay.as_millis() as f64);
        Duration::from_millis(capped_ms as u64)
    }
}

#[cfg(test)]
#[path = "retry.tests.rs"]
mod tests;
