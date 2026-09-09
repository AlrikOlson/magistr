//! What a gate really did.

use std::time::Duration;

use crate::orchestration::loop_driver::GateOutcome;

/// One gate's outcome as the judge is shown it — built from the driver's
/// [`GateOutcome`] and from nothing else. There is deliberately no way
/// to make one from text: the judge sees what the gate's process exited
/// with, never what an agent said about it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GateEvidence {
    /// The gate's name (`check`, `test`, `lint`, a custom gate).
    pub name: String,
    /// Whether its command exited zero within the time allowed.
    pub passed: bool,
    /// How long it ran.
    pub duration: Duration,
}

impl GateEvidence {
    /// The line the judge reads for this gate.
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

impl From<&GateOutcome> for GateEvidence {
    fn from(outcome: &GateOutcome) -> Self {
        Self {
            name: outcome.name.clone(),
            passed: outcome.passed(),
            duration: outcome.duration,
        }
    }
}

#[cfg(test)]
#[path = "gate_evidence.tests.rs"]
mod tests;
