//! A single git commit with basic metadata.

/// A single git commit with basic metadata.
#[derive(Debug, Clone, PartialEq)]
pub struct CommitInfo {
    /// Short SHA (7 chars).
    pub sha: String,
    /// First line of commit message.
    pub subject: String,
    /// When the commit was made.
    pub date: String,
}
