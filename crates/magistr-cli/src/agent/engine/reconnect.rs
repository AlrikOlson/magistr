//! Reconnection policy for remote engine clients.
//!
//! Defines how a `RemoteEngineClient` should handle connection failures:
//! retry count, backoff strategy, and per-attempt timeout. Not used by
//! `LocalEngineClient` (which never disconnects), but defined here so
//! the abstraction is complete.

use std::time::Duration;

/// Policy controlling reconnection behavior for remote engine clients.
///
/// `LocalEngineClient` ignores this (in-process connections don't disconnect).
/// `RemoteEngineClient` uses it to decide when and how to retry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReconnectionPolicy {
    /// Maximum number of reconnection attempts before giving up.
    pub max_retries: u32,

    /// Initial delay before the first retry.
    pub initial_backoff: Duration,

    /// Maximum delay between retries (caps exponential growth).
    pub max_backoff: Duration,

    /// Timeout for each individual connection attempt.
    pub connect_timeout: Duration,

    /// Backoff multiplier applied after each failed attempt.
    pub backoff_multiplier: u32,
}

impl Default for ReconnectionPolicy {
    fn default() -> Self {
        Self {
            max_retries: 5,
            initial_backoff: Duration::from_millis(500),
            max_backoff: Duration::from_secs(30),
            connect_timeout: Duration::from_secs(10),
            backoff_multiplier: 2,
        }
    }
}

impl ReconnectionPolicy {
    /// Create a policy that never retries (fail immediately on disconnect).
    #[must_use]
    pub fn no_retry() -> Self {
        Self {
            max_retries: 0,
            ..Self::default()
        }
    }

    /// Set the maximum number of retries.
    #[must_use]
    pub fn max_retries(mut self, n: u32) -> Self {
        self.max_retries = n;
        self
    }

    /// Set the initial backoff duration.
    #[must_use]
    pub fn initial_backoff(mut self, d: Duration) -> Self {
        self.initial_backoff = d;
        self
    }

    /// Set the maximum backoff duration.
    #[must_use]
    pub fn max_backoff(mut self, d: Duration) -> Self {
        self.max_backoff = d;
        self
    }

    /// Set the per-attempt connection timeout.
    #[must_use]
    pub fn connect_timeout(mut self, d: Duration) -> Self {
        self.connect_timeout = d;
        self
    }

    /// Calculate the backoff duration for a given attempt number (0-based).
    ///
    /// Uses exponential backoff: `initial_backoff * multiplier^attempt`,
    /// capped at `max_backoff`.
    #[must_use]
    pub fn backoff_for_attempt(&self, attempt: u32) -> Duration {
        let multiplier = self.backoff_multiplier.saturating_pow(attempt);
        let backoff = self.initial_backoff.saturating_mul(multiplier);
        backoff.min(self.max_backoff)
    }

    /// Whether the given attempt number (0-based) should be retried.
    #[must_use]
    pub fn should_retry(&self, attempt: u32) -> bool {
        attempt < self.max_retries
    }
}

#[cfg(test)]
#[path = "reconnect.tests.rs"]
mod tests;
