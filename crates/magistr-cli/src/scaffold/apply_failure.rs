use std::fmt;

use super::{Expected, Refusal};

/// Why a change the plan meant to apply did not land. The target is left as
/// the service found it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ApplyFailure {
    /// The target no longer matches the plan's expected state: it was
    /// created, edited, replaced or removed after planning.
    Stale {
        /// What the plan expected to find.
        expected: Expected,
        /// What was there at write time.
        found: Expected,
    },
    /// The target could not be revalidated: it is no longer a regular file
    /// or cannot be read, which is not evidence of absence.
    Blocked(Refusal),
    /// The write or removal itself failed.
    Io {
        /// What went wrong.
        cause: String,
    },
}

impl fmt::Display for ApplyFailure {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Stale { expected, found } => write!(
                f,
                "changed after planning: expected {}, found {}",
                expected.label(),
                found.label()
            ),
            Self::Blocked(refusal) => write!(f, "cannot be revalidated: {refusal}"),
            Self::Io { cause } => write!(f, "write failed: {cause}"),
        }
    }
}
