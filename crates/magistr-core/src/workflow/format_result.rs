//! Result of a formatting run.

use std::path::{Path, PathBuf};
use std::process::Command;

/// Result of a formatting run.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FormatResult {
    /// Files that were successfully formatted.
    pub files_formatted: Vec<PathBuf>,
    /// Files that failed formatting, with error messages.
    pub files_failed: Vec<(PathBuf, String)>,
    /// Whether formatting was skipped entirely.
    pub skipped: bool,
    /// Reason for skipping, if applicable.
    pub skip_reason: Option<String>,
}

impl FormatResult {
    /// Create a skipped result with a reason.
    pub(crate) fn skipped(reason: impl Into<String>) -> Self {
        Self {
            files_formatted: Vec::new(),
            files_failed: Vec::new(),
            skipped: true,
            skip_reason: Some(reason.into()),
        }
    }

    /// Whether the formatter made any changes to files.
    ///
    /// Checks `git diff --name-only` after formatting to detect modifications.
    pub fn has_changes(project_dir: &Path) -> bool {
        let output = Command::new("git")
            .args(["diff", "--name-only"])
            .current_dir(project_dir)
            .output();

        match output {
            Ok(out) if out.status.success() => {
                !String::from_utf8_lossy(&out.stdout).trim().is_empty()
            }
            _ => false,
        }
    }

    /// Total number of files processed (formatted + failed).
    pub fn total_processed(&self) -> usize {
        self.files_formatted.len() + self.files_failed.len()
    }
}
