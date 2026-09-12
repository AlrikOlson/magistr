use std::time::Duration;

/// How the process group is ended on a timeout or a cancellation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Termination {
    /// `SIGTERM` the group, wait up to `grace` for the group to disappear,
    /// then `SIGKILL` the group. A gate that traps `TERM` gets to clean up.
    Graceful {
        /// How long the group gets after `SIGTERM`.
        grace: Duration,
    },
    /// `SIGKILL` the group at once.
    Immediate,
}
