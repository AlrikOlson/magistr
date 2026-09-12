/// Where the child's output goes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OutputMode {
    /// Two pipes, captured separately.
    Pipes,
    /// A fresh pseudo-terminal of the given size on both streams: the
    /// child sees a tty and both streams arrive as the one stream a tty
    /// is. Where no pty can be had (a platform without one, a master that
    /// will not open) the process falls back to pipes and says so through
    /// [`OwnedProcess::is_terminal`](crate::OwnedProcess::is_terminal).
    Terminal {
        /// Columns.
        cols: u16,
        /// Rows.
        rows: u16,
    },
}
