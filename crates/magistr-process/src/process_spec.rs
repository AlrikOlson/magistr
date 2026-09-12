use std::path::PathBuf;
use std::time::Duration;

use crate::{Launch, OutputMode, Termination};

/// Everything one run needs, decided by the consumer before the spawn.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProcessSpec {
    /// What to start.
    pub launch: Launch,
    /// The working directory; `None` inherits this process's.
    pub cwd: Option<PathBuf>,
    /// The wall-clock cap; when it elapses the group is ended.
    pub timeout: Duration,
    /// Pipes or a pty.
    pub output: OutputMode,
    /// Per-stream capture ceiling: head plus tail bytes kept, the rest
    /// counted, never buffered.
    pub capture_bytes: usize,
    /// How the group is ended on a timeout or a cancel.
    pub termination: Termination,
}

impl ProcessSpec {
    /// Per-stream capture default: enough for any realistic transcript
    /// while a runaway producer can never exhaust the host.
    pub const CAPTURE_DEFAULT_BYTES: usize = 8 * 1024 * 1024;

    /// A spec with the defaults: inherited working directory, pipes, the
    /// default capture ceiling and a graceful five-second termination.
    #[must_use]
    pub fn new(launch: Launch, timeout: Duration) -> Self {
        Self {
            launch,
            cwd: None,
            timeout,
            output: OutputMode::Pipes,
            capture_bytes: Self::CAPTURE_DEFAULT_BYTES,
            termination: Termination::Graceful {
                grace: Duration::from_secs(5),
            },
        }
    }

    /// Run in `cwd`.
    #[must_use]
    pub fn in_dir(mut self, cwd: impl Into<PathBuf>) -> Self {
        self.cwd = Some(cwd.into());
        self
    }

    /// Capture on a pty of `cols` × `rows` instead of pipes.
    #[must_use]
    pub fn on_terminal(mut self, cols: u16, rows: u16) -> Self {
        self.output = OutputMode::Terminal { cols, rows };
        self
    }

    /// Replace the per-stream capture ceiling.
    #[must_use]
    pub fn with_capture_bytes(mut self, capture_bytes: usize) -> Self {
        self.capture_bytes = capture_bytes;
        self
    }

    /// Replace how the group is ended.
    #[must_use]
    pub fn with_termination(mut self, termination: Termination) -> Self {
        self.termination = termination;
        self
    }
}
