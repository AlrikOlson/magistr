//! The name of a job.

use std::fmt;

/// Identifies the `n`th job assigned by one runtime handle family.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct JobId(u64);

impl JobId {
    /// The job numbered `n`.
    #[must_use]
    pub const fn new(n: u64) -> Self {
        Self(n)
    }

    /// The number.
    #[must_use]
    pub const fn get(self) -> u64 {
        self.0
    }
}

impl fmt::Display for JobId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "job {}", self.0)
    }
}
