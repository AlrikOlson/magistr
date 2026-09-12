use std::time::Duration;

/// What one run produced — the mechanism's whole answer. Every terminal
/// outcome is concrete: an exit code, a signal, a timeout kill or a
/// cancellation kill; partial output travels with all of them. An exit code
/// beside `timed_out` means the child itself exited but something it left
/// behind held its pipes until the deadline, and the group was ended then.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ProcessOutcome {
    /// Process exit code; `None` when a signal, the timeout or a cancel
    /// ended it before it reported one.
    pub exit_code: Option<i32>,
    /// The signal that ended the process, when the platform reports one.
    pub signal: Option<i32>,
    /// Captured stdout — on a pty, the one merged stream.
    pub stdout: String,
    /// Captured stderr — empty on a pty.
    pub stderr: String,
    /// The deadline elapsed and the group was ended.
    pub timed_out: bool,
    /// The caller's token fired and the group was ended.
    pub cancelled: bool,
    /// Wall-clock time from the spawn to the answer.
    pub duration: Duration,
    /// Bytes the capture ceiling discarded across both streams.
    pub dropped_bytes: u64,
    /// The spawned process id (also its group id on Unix).
    pub pid: Option<u32>,
    /// An I/O failure while waiting on or reading the child, when one
    /// happened; the process was ended and reaped regardless.
    pub fault: Option<String>,
}

impl ProcessOutcome {
    /// The process ran to its own end and reported an exit code, with no
    /// deadline or cancel involved.
    #[must_use]
    pub fn is_complete(&self) -> bool {
        self.exit_code.is_some() && !self.timed_out && !self.cancelled && self.fault.is_none()
    }
}
