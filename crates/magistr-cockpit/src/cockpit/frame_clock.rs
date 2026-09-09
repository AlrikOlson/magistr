//! The only clock `view` ever sees.

use std::time::Duration;

/// Which frame is being drawn and how long the cockpit has been up —
/// and, when the runtime says so, what time it is.
///
/// The runtime samples the wall clock once per frame and passes the
/// result here, so `view` stays a pure function of its arguments and a
/// test can render frame 17 at 3.2 s without waiting for either. The
/// epoch reading is the same sample carried whole: a surface that
/// prints an age (the Signals inbox's `2 d`) reads it from here, never
/// from a clock of its own, and a test that passes none draws no age.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct FrameClock {
    frame: u64,
    elapsed: Duration,
    epoch_secs: Option<u64>,
}

impl FrameClock {
    /// Frame number `frame`, `elapsed` since the cockpit started; no
    /// wall-clock reading.
    #[must_use]
    pub const fn new(frame: u64, elapsed: Duration) -> Self {
        Self {
            frame,
            elapsed,
            epoch_secs: None,
        }
    }

    /// The same clock carrying the wall-clock reading the runtime took
    /// for this frame, in seconds since the Unix epoch.
    #[must_use]
    pub const fn with_epoch_secs(mut self, secs: u64) -> Self {
        self.epoch_secs = Some(secs);
        self
    }

    /// The frame being drawn (the first is `0`).
    #[must_use]
    pub const fn frame(self) -> u64 {
        self.frame
    }

    /// Time since the cockpit started.
    #[must_use]
    pub const fn elapsed(self) -> Duration {
        self.elapsed
    }

    /// The wall clock at this frame, in seconds since the Unix epoch —
    /// `None` when the runtime gave none, which is every test that did
    /// not ask for one.
    #[must_use]
    pub const fn epoch_secs(self) -> Option<u64> {
        self.epoch_secs
    }
}

#[cfg(test)]
#[path = "frame_clock.tests.rs"]
mod tests;
