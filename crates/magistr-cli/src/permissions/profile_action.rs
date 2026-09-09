//! What a profile says to do for a given tool call.

/// What a profile says to do for a given tool call.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProfileAction {
    /// Automatically allow without prompting.
    Allow,
    /// Ask the user for permission.
    Ask,
}
