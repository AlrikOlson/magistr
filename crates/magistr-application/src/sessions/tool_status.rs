//! Where a tool call stands.

/// The status of a tool call the agent reported.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ToolStatus {
    /// Announced, not started.
    Pending,
    /// Running.
    InProgress,
    /// Finished.
    Completed,
    /// Failed.
    Failed,
}
