//! What the loop does with a verdict.

use super::gate_evidence::GateEvidence;
use super::verdict::Verdict;

/// The loop's next move, reconciled from the judge's [`Verdict`] and the
/// real gates. This is the one place the authority rule lives:
/// `ship_check`'s exit codes complete a chunk, a verdict never does.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Decision {
    /// The judge says met and every gate passed: complete the chunk.
    Complete {
        /// The judge's reason.
        reason: String,
    },
    /// Not yet: run the next iteration, the reason on the stage.
    Continue {
        /// The judge's reason.
        reason: String,
    },
    /// The judge says met but a gate is red: the chunk stays in
    /// progress and the disagreement is reported — a bug in the
    /// condition or the evidence, never a completion.
    Disagreement {
        /// The judge's reason.
        reason: String,
        /// The gates that failed, by name.
        failed: Vec<String>,
    },
    /// The judge says the acceptance cannot be met as written: stop the
    /// loop and ask a human, never silently.
    Stop {
        /// The judge's reason.
        reason: String,
    },
}

impl Decision {
    /// Reconcile `verdict` with `gates`.
    #[must_use]
    pub fn reconcile(verdict: &Verdict, gates: &[GateEvidence]) -> Self {
        let reason = verdict.reason.clone();
        if verdict.impossible {
            return Self::Stop { reason };
        }
        if !verdict.ok {
            return Self::Continue { reason };
        }
        let failed: Vec<String> = gates
            .iter()
            .filter(|gate| !gate.passed)
            .map(|gate| gate.name.clone())
            .collect();
        if failed.is_empty() {
            Self::Complete { reason }
        } else {
            Self::Disagreement { reason, failed }
        }
    }

    /// The judge's reason, whichever move this is.
    #[must_use]
    pub fn reason(&self) -> &str {
        match self {
            Self::Complete { reason }
            | Self::Continue { reason }
            | Self::Disagreement { reason, .. }
            | Self::Stop { reason } => reason,
        }
    }

    /// Whether the loop runs another iteration after this.
    #[must_use]
    pub const fn continues(&self) -> bool {
        matches!(self, Self::Continue { .. } | Self::Disagreement { .. })
    }
}

#[cfg(test)]
#[path = "decision.tests.rs"]
mod tests;
