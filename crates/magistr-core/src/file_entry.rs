//! File entry data for sidebar file trees.
//!
//! A `FileEntry` represents a single file or directory in the project's
//! file tree sidebar — generated/modified files tracked during a session.
//! Frontend-agnostic: no rendering types, just path + status + diff stats.

use super::file_status::FileStatus;

/// A single entry in the file tree.
#[derive(Debug, Clone)]
pub struct FileEntry {
    /// Relative path from project root.
    pub path: String,
    /// Depth in the directory tree (for indentation).
    pub depth: usize,
    /// Whether this is a directory (vs a file).
    pub is_dir: bool,
    /// File status indicator.
    pub status: FileStatus,
    /// Lines added (for diff stats display).
    pub additions: Option<u16>,
    /// Lines deleted (for diff stats display).
    pub deletions: Option<u16>,
    /// Whether this file was modified by the agent (vs pre-existing changes).
    pub is_agent_modified: bool,
}

#[cfg(test)]
#[path = "file_entry.tests.rs"]
mod tests;
