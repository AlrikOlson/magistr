//! The fencing generation an admission mints.

use std::fs::File;
use std::io::{self, Read, Seek, SeekFrom, Write};

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

/// The counter is written as a fixed-width decimal so every rewrite covers
/// the previous one exactly: the lock file is never truncated, and a
/// generation is never shorter than the one before it.
const WIDTH: usize = 20;

impl OwnerGeneration {
    /// Read the counter at the head of `lock`, advance it, and write it
    /// back in place. Called once per admission, under the lock the file
    /// carries; a file that holds nothing (or something that is not a
    /// counter) starts at one.
    pub(crate) fn mint(lock: &mut File) -> io::Result<Self> {
        lock.seek(SeekFrom::Start(0))?;
        let mut bytes = Vec::with_capacity(WIDTH);
        (&*lock).take(WIDTH as u64).read_to_end(&mut bytes)?;
        let previous = String::from_utf8_lossy(&bytes)
            .trim()
            .parse::<u64>()
            .unwrap_or(0);
        let next = previous.checked_add(1).ok_or_else(|| {
            io::Error::other("magistr: the owner generation counter is exhausted")
        })?;
        lock.seek(SeekFrom::Start(0))?;
        write!(lock, "{next:0WIDTH$}")?;
        lock.flush()?;
        Ok(Self(next))
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

#[cfg(test)]
#[path = "owner_generation.tests.rs"]
mod tests;
