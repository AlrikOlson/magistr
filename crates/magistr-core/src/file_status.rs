//! File status types for sidebar file trees.

/// Status of a file in the sidebar tree.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum FileStatus {
    /// Modified existing file (sorted first — most important)
    Modified,
    /// Newly created file
    Created,
    /// Deleted file
    Deleted,
    /// Renamed file
    Renamed,
}

impl FileStatus {
    /// Sort priority (lower = sorted first).
    pub fn sort_priority(self) -> u8 {
        match self {
            Self::Modified => 0,
            Self::Created => 1,
            Self::Deleted => 2,
            Self::Renamed => 3,
        }
    }

    /// Single-character badge for git status — always available (no Nerd Font needed).
    ///
    /// `M` (modified/yellow), `A` (added/green), `D` (deleted/red), `R` (renamed/blue).
    pub fn badge(self) -> &'static str {
        match self {
            Self::Modified => "M",
            Self::Created => "A",
            Self::Deleted => "D",
            Self::Renamed => "R",
        }
    }
}

#[cfg(test)]
#[path = "file_status.tests.rs"]
mod tests;
