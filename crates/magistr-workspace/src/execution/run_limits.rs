use std::time::Duration;

/// Hard limits the executor enforces for one execution: the wall-clock cap
/// and the per-stream capture ceiling (head plus tail bytes kept; the rest is
/// counted, never buffered).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RunLimits {
    pub timeout: Duration,
    pub capture_bytes: usize,
}

impl RunLimits {
    /// Per-stream capture default: enough for any realistic runner transcript
    /// while a runaway producer can never exhaust the host.
    pub const CAPTURE_DEFAULT_BYTES: usize = 8 * 1024 * 1024;

    #[must_use]
    pub fn new(timeout: Duration) -> Self {
        Self {
            timeout,
            capture_bytes: Self::CAPTURE_DEFAULT_BYTES,
        }
    }

    #[must_use]
    pub fn with_capture_bytes(mut self, capture_bytes: usize) -> Self {
        self.capture_bytes = capture_bytes;
        self
    }
}

impl Default for RunLimits {
    fn default() -> Self {
        Self::new(Duration::from_secs(super::RunInput::TIMEOUT_DEFAULT_SECS))
    }
}
