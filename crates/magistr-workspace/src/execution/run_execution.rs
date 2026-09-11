/// What one execution produced — the executor's whole answer. Every terminal
/// outcome is concrete: an exit code, a signal, a timeout kill or a
/// cancellation kill; partial output travels with all of them.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct RunExecution {
    /// Process exit code; `None` when a signal, the timeout or a cancel ended it.
    pub exit_code: Option<i32>,
    /// The signal that ended the process, when the platform reports one.
    pub signal: Option<i32>,
    pub stdout: String,
    pub stderr: String,
    pub timed_out: bool,
    pub cancelled: bool,
    pub duration_ms: u64,
    /// Bytes the capture ceiling discarded across both streams.
    pub dropped_bytes: u64,
    /// The spawned process id (also its dedicated group id on Unix).
    pub pid: Option<u32>,
}

impl RunExecution {
    #[must_use]
    pub fn exited(exit_code: i32) -> Self {
        Self {
            exit_code: Some(exit_code),
            duration_ms: 5,
            ..Self::default()
        }
    }

    #[must_use]
    pub fn with_stdout(mut self, stdout: impl Into<String>) -> Self {
        self.stdout = stdout.into();
        self
    }

    #[must_use]
    pub fn with_stderr(mut self, stderr: impl Into<String>) -> Self {
        self.stderr = stderr.into();
        self
    }

    /// The process ran to its own end and reported an exit code.
    #[must_use]
    pub fn is_complete(&self) -> bool {
        self.exit_code.is_some() && !self.timed_out && !self.cancelled
    }
}
