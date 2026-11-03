//! What a check really did.

use std::time::Duration;

use crate::utils::check::CheckRun;

/// One check's outcome as the judge is shown it — built from the driver's
/// [`CheckRun`] and from nothing else. There is deliberately no way
/// to make one from text: the judge sees what the check's process exited
/// with, never what an agent said about it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CheckEvidence {
    /// The check's name (`check`, `test`, `lint`, a custom check).
    pub name: String,
    /// Whether its command exited zero within the time allowed.
    pub passed: bool,
    /// How long it ran.
    pub duration: Duration,
}

impl CheckEvidence {
    /// The line the judge reads for this check.
    #[must_use]
    pub fn line(&self) -> String {
        format!(
            "- {}: {} ({:.1}s)",
            self.name,
            if self.passed { "PASSED" } else { "FAILED" },
            self.duration.as_secs_f64()
        )
    }
}

impl From<&CheckRun> for CheckEvidence {
    fn from(outcome: &CheckRun) -> Self {
        Self {
            name: outcome.name.clone(),
            passed: outcome.passed(),
            duration: outcome.duration,
        }
    }
}

#[cfg(test)]
#[path = "check_evidence.tests.rs"]
mod tests;
