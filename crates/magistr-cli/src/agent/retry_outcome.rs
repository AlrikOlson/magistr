//! Outcome of a retry operation.

use crate::error::MagistrError;

/// Outcome of a retry operation.
#[derive(Debug)]
pub struct RetryOutcome<T> {
    /// The final result (success or last error).
    pub result: Result<T, MagistrError>,
    /// Number of attempts made (1 = succeeded on first try).
    pub attempts: u32,
}
