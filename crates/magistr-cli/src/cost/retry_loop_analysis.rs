//! Result of retry loop analysis.

/// Result of retry loop analysis.
#[derive(Debug, Clone, PartialEq)]
pub struct RetryLoopAnalysis {
    /// Whether a retry loop was detected.
    pub is_looping: bool,
    /// The repeated error signature (if looping).
    pub repeated_error: Option<String>,
    /// Number of consecutive failures with the same error.
    pub consecutive_failures: u32,
    /// Estimated tokens wasted on repeated failures.
    pub estimated_wasted_tokens: u64,
}
