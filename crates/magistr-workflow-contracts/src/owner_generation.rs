//! The fencing generation an admission mints.

use std::io;

use serde::{Deserialize, Serialize};

/// Which incarnation of a role's owner this is: a counter kept in the
/// role's lock file, incremented under the held lock at every admission,
/// so it is monotonic across processes and restarts without a clock. It
/// changes when ownership changes, never on a state write.
///
/// An attached client carries the generation it was welcomed under on
/// every command, and the owner refuses a command from any generation but
/// its own — so a client that outlived one owner cannot act on the next
/// until it reconnects and re-reads what it is looking at.
#[derive(
    Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize,
)]
#[serde(transparent)]
pub struct OwnerGeneration(u64);

impl OwnerGeneration {
    /// Wrap the nonzero generation minted by another admitted local owner.
    pub fn admitted(value: u64) -> io::Result<Self> {
        if value == 0 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "magistr: an owner generation must be nonzero",
            ));
        }
        Ok(Self(value))
    }

    /// The counter's value.
    #[must_use]
    pub const fn get(self) -> u64 {
        self.0
    }
}

impl std::fmt::Display for OwnerGeneration {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}
