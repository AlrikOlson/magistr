use alloc::boxed::Box;
use core::fmt;

use crate::identity::AttemptId;

use super::{OperationOutcome, OperationRecord};

/// Why the ledger refused an admission or a transition.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OperationError {
    /// The command's latest attempt has no established outcome: reconcile it
    /// before another attempt can be admitted.
    Unsettled(Box<OperationRecord>),
    /// The command id was reused for a different kind or target.
    Conflict(Box<OperationRecord>),
    /// No such attempt in the ledger.
    Unknown(AttemptId),
    /// The requested outcome cannot follow the recorded one.
    Transition {
        attempt: AttemptId,
        from: OperationOutcome,
        to: OperationOutcome,
    },
}

impl fmt::Display for OperationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Unsettled(record) => write!(
                f,
                "operation {} attempt {} of {} is {}: reconcile it before retrying",
                record.attempt.command().local(),
                record.attempt.number(),
                record.kind,
                record.outcome
            ),
            Self::Conflict(record) => write!(
                f,
                "operation {} is already recorded as {} on {}",
                record.attempt.command().local(),
                record.kind,
                record.target
            ),
            Self::Unknown(attempt) => write!(
                f,
                "operation {} attempt {} is not in the ledger",
                attempt.command().local(),
                attempt.number()
            ),
            Self::Transition { attempt, from, to } => write!(
                f,
                "operation {} attempt {} cannot go from {from} to {to}",
                attempt.command().local(),
                attempt.number()
            ),
        }
    }
}

impl core::error::Error for OperationError {}
