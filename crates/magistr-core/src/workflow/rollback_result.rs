//! Result type for rollback attempts.

/// Result of a rollback attempt.
#[derive(Debug, Clone, PartialEq)]
pub enum RollbackResult {
    /// Rollback succeeded — changes stashed.
    Stashed {
        /// The stash reference (e.g., "stash@{0}").
        stash_ref: String,
        /// Number of files affected.
        files_affected: u32,
        /// Description of what failed.
        failure_reason: String,
    },
    /// No rollback needed — gates still passing or were already failing.
    NotNeeded,
    /// Rollback was needed but failed (e.g., git error).
    Failed(String),
}
