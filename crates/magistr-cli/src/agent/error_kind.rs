//! Error classification for retry decisions.

use crate::error::MagistrError;

/// Whether an error is transient (retryable) or permanent.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ErrorKind {
    /// Temporary failure — retry may succeed.
    Transient,
    /// Permanent failure — retrying won't help.
    Permanent,
}

impl ErrorKind {
    /// Classify a `MagistrError` as transient or permanent.
    ///
    /// Transient errors include:
    /// - Network/IO errors (connection reset, timeout)
    /// - ACP errors with transient message patterns
    /// - Generic "channel closed" errors (race condition on shutdown)
    ///
    /// Permanent errors include:
    /// - Configuration errors
    /// - Authentication errors
    /// - Validation errors
    /// - Detection errors
    #[must_use]
    pub fn classify(err: &MagistrError) -> Self {
        match err {
            MagistrError::Io { .. } => Self::Transient,
            MagistrError::Acp(msg) if is_transient_message(msg) => Self::Transient,
            MagistrError::Engine(msg) if msg.contains("channel closed") => Self::Transient,
            MagistrError::Engine(msg) if is_transient_message(msg) => Self::Transient,
            MagistrError::Other(msg) if msg.contains("channel closed") => Self::Transient,
            MagistrError::Other(msg) if is_transient_message(msg) => Self::Transient,
            // Config, detection, template, budget errors are permanent
            _ => Self::Permanent,
        }
    }
}

/// Check if a generic error message indicates a transient failure.
fn is_transient_message(msg: &str) -> bool {
    let lower = msg.to_lowercase();
    lower.contains("timeout")
        || lower.contains("timed out")
        || lower.contains("connection reset")
        || lower.contains("broken pipe")
        || lower.contains("temporarily unavailable")
        || lower.contains("rate limit")
        || lower.contains("503")
        || lower.contains("502")
        || lower.contains("429")
}
