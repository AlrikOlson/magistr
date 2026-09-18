use serde::{Deserialize, Serialize};

/// Where one operation attempt stands. A settled outcome latches; an
/// unsettled one is either owned by a live process or, after a restart,
/// `Indeterminate` until reconciliation establishes what actually happened.
///
/// None of these certifies exactly-once execution of the effect they name:
/// the record says what Magistr wrote before and after the effect, not what
/// the runtime, filesystem or native store did in between.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OperationOutcome {
    /// Admitted and written durably; the effect has not been dispatched.
    Accepted,
    /// The owner began dispatching; the effect may already have started.
    Running,
    /// The effect is established as applied.
    Completed,
    /// The effect is established as not applied, or as failed.
    Failed,
    /// The effect was not attempted, or nothing remains to redo.
    Cancelled,
    /// The owner was lost with the effect in flight; nothing is established.
    Indeterminate,
}

impl OperationOutcome {
    /// Whether the outcome is final. Settled outcomes never change.
    #[must_use]
    pub const fn is_settled(self) -> bool {
        matches!(self, Self::Completed | Self::Failed | Self::Cancelled)
    }

    /// Whether the owner of a live attempt may report `next` from `self`.
    /// `Indeterminate` can be reported (a lost response with the process
    /// alive) but never left this way: that takes [`Self::can_reconcile_to`].
    #[must_use]
    pub const fn can_settle_to(self, next: Self) -> bool {
        match self {
            Self::Accepted => !matches!(next, Self::Accepted),
            Self::Running => !matches!(next, Self::Accepted | Self::Running),
            Self::Completed | Self::Failed | Self::Cancelled | Self::Indeterminate => false,
        }
    }

    /// Whether reconciliation may establish `next` from `self`: only an
    /// `Indeterminate` attempt is open to it, and only to a settled outcome.
    #[must_use]
    pub const fn can_reconcile_to(self, next: Self) -> bool {
        matches!(self, Self::Indeterminate) && next.is_settled()
    }

    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Accepted => "accepted",
            Self::Running => "running",
            Self::Completed => "completed",
            Self::Failed => "failed",
            Self::Cancelled => "cancelled",
            Self::Indeterminate => "indeterminate",
        }
    }
}

impl core::fmt::Display for OperationOutcome {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str(self.as_str())
    }
}

#[cfg(test)]
#[path = "operation_outcome.tests.rs"]
mod tests;
