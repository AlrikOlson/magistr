//! What a gate run decided.

/// The five things a gate run can be.
///
/// The loop driver used to fold `timed_out || cancelled || exit != 0` into
/// one `passed: bool`, so a gate killed at the 300-second ceiling and a
/// gate whose tests failed were the same event. They are not: one is a
/// statement about the code, the other is a statement about the clock.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum GateVerdict {
    /// The command exited zero within the time allowed.
    Passed,
    /// The command exited non-zero.
    Failed,
    /// The command was killed at the timeout.
    TimedOut,
    /// The run was ended from outside — the cockpit is closing.
    Cancelled,
    /// The command failed and then passed on its retry: a flake, named
    /// rather than hidden. Counts as passed, and keeps both exit codes.
    Flaky,
}

impl GateVerdict {
    /// Whether this verdict lets the loop go on. A flake does.
    pub(crate) const fn passed(self) -> bool {
        matches!(self, Self::Passed | Self::Flaky)
    }

    /// Whether this verdict says anything about the code at all.
    ///
    /// A cancelled gate does not: the cockpit is closing and nobody let
    /// the command finish. Nothing consequential may be decided on one.
    pub(crate) const fn is_evidence(self) -> bool {
        !matches!(self, Self::Cancelled)
    }

    /// The word for this verdict.
    pub(crate) const fn label(self) -> &'static str {
        match self {
            Self::Passed => "passed",
            Self::Failed => "failed",
            Self::TimedOut => "timed out",
            Self::Cancelled => "cancelled",
            Self::Flaky => "flaky",
        }
    }
}

#[cfg(test)]
#[path = "gate_verdict.tests.rs"]
mod tests;
