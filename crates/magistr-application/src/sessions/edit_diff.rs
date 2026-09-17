//! The diff a tool call carried.

use alloc::string::String;

/// A file modification an ACP tool call reported as diff content: the
/// path and both sides of the change, ready for the turn's edit card.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EditDiff {
    /// The file the edit lands on, as the agent reported it.
    pub path: String,
    /// The content before the edit — empty for a new file.
    pub old_text: String,
    /// The content after the edit.
    pub new_text: String,
}
