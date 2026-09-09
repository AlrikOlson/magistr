use super::PermissionAnswer;

/// Whether an answer reached the pending decision or it had already expired.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PermissionOutcome {
    /// The exact answer was accepted; this does not certify execution of the tool.
    Resolved { answer: PermissionAnswer },
    /// The decision was no longer live. No permission was granted by this command.
    Expired,
}
