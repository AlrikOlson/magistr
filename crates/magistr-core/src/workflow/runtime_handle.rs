//! The handle a dispatch record keeps for the runtime it launched: enough
//! to ask the host about that particular process later, not merely about
//! whatever holds its id by then.

use std::fmt;

use crate::workflow::process_alive::is_process_alive;
use crate::workflow::process_start::process_start;

/// A provider runtime as the loop recorded it at launch: its process (group)
/// id and, where the host offers one, the start identity that pins the id
/// to this process. Spelled `pid:<id>@<start>` on the record, or `pid:<id>`
/// when the host offered no start identity.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeHandle {
    pid: u32,
    start: Option<u64>,
}

impl RuntimeHandle {
    #[must_use]
    pub const fn new(pid: u32, start: Option<u64>) -> Self {
        Self { pid, start }
    }

    /// The handle of the process `pid` names right now: its start identity
    /// read from the host while it lives. `None` for no process at all — a
    /// zero id, or one that is already gone — so a record never claims a
    /// launch that has nothing to be asked about.
    #[must_use]
    pub fn observe(pid: u32) -> Option<Self> {
        if pid == 0 || is_process_alive(pid) != Some(true) {
            return None;
        }
        Some(Self::new(pid, process_start(pid)))
    }

    /// The record's spelling back into a handle; `None` for anything else.
    #[must_use]
    pub fn parse(text: &str) -> Option<Self> {
        let rest = text.strip_prefix("pid:")?;
        let (pid, start) = match rest.split_once('@') {
            Some((pid, start)) => (pid, Some(start.parse::<u64>().ok()?)),
            None => (rest, None),
        };
        let pid = pid.parse::<u32>().ok().filter(|pid| *pid > 0)?;
        Some(Self::new(pid, start))
    }

    #[must_use]
    pub const fn pid(&self) -> u32 {
        self.pid
    }

    #[must_use]
    pub const fn start(&self) -> Option<u64> {
        self.start
    }

    /// Whether this runtime — this process, not a later holder of its id —
    /// is still running. `Some(false)` once the id is gone, or held by a
    /// process with another start identity than the one recorded; `None`
    /// when the host cannot say, which a caller treats as "not known to be
    /// gone".
    #[must_use]
    pub fn is_live(&self) -> Option<bool> {
        match is_process_alive(self.pid)? {
            false => Some(false),
            true => match (self.start, process_start(self.pid)) {
                (Some(recorded), Some(now)) => Some(recorded == now),
                // A recorded identity the host can no longer read, or a
                // handle recorded without one: alive is all that is known.
                _ => Some(true),
            },
        }
    }
}

impl fmt::Display for RuntimeHandle {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.start {
            Some(start) => write!(f, "pid:{}@{start}", self.pid),
            None => write!(f, "pid:{}", self.pid),
        }
    }
}

#[cfg(test)]
#[path = "runtime_handle.tests.rs"]
mod tests;
