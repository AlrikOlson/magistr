//! Whether the supervisor still owns a live provider connection.

/// Connection residency is independent from the last turn's terminal outcome.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SessionResidency {
    /// The connection remains available for turns and configuration.
    Resident,
    /// Explicit termination or owner drop ended the connection.
    Terminated,
    /// The provider or its transport failed.
    Failed,
}
