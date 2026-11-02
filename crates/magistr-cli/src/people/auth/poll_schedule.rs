//! The device-grant polling cadence, kept off the wall clock.

use std::time::Duration;

use super::device_authorization::DeviceAuthorization;

/// RFC 8628's default when a provider states no `interval`.
const DEFAULT_INTERVAL_SECS: u64 = 5;
/// The fixed increment a `slow_down` costs, per §3.5.
const SLOW_DOWN_STEP_SECS: u64 = 5;

/// How long to wait between polls, and how long the whole flow may run.
///
/// Deliberately clock-free: the schedule counts the waits it has been
/// charged for rather than reading the time, so the polling loop is a
/// pure function of the provider's answers and the tests never sleep.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PollSchedule {
    interval: Duration,
    elapsed: Duration,
    lifetime: Duration,
}

impl PollSchedule {
    /// Start the cadence a device-authorization response asks for.
    #[must_use]
    pub fn new(auth: &DeviceAuthorization) -> Self {
        PollSchedule {
            interval: Duration::from_secs(auth.interval.unwrap_or(DEFAULT_INTERVAL_SECS)),
            elapsed: Duration::ZERO,
            lifetime: Duration::from_secs(auth.expires_in),
        }
    }

    /// How long to wait before the next poll.
    #[must_use]
    pub fn interval(&self) -> Duration {
        self.interval
    }

    /// Apply a `slow_down`: five more seconds, cumulatively, for this
    /// poll and every one after it.
    pub fn slow_down(&mut self) {
        self.interval += Duration::from_secs(SLOW_DOWN_STEP_SECS);
    }

    /// Charge one interval to the schedule.
    ///
    /// Returns `false` once the device code's lifetime is spent — the
    /// caller stops rather than polling a code the provider has already
    /// forgotten.
    pub fn charge(&mut self) -> bool {
        self.elapsed += self.interval;
        self.elapsed < self.lifetime
    }

    /// Time left before the device code expires.
    #[must_use]
    pub fn remaining(&self) -> Duration {
        self.lifetime.saturating_sub(self.elapsed)
    }
}

#[cfg(test)]
#[path = "poll_schedule.tests.rs"]
mod tests;
