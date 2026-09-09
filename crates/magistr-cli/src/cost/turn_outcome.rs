//! Outcome of a single turn for retry loop detection.

/// Outcome of a single turn for retry loop detection.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TurnOutcome {
    /// The turn succeeded (quality gates passed, no errors).
    Success,
    /// The turn failed with an error signature.
    /// The string is a normalized error key (e.g., "compile_error:E0308").
    Failure(String),
}
