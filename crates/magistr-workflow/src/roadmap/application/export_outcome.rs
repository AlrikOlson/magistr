use std::path::PathBuf;

use super::ExportFormat;

/// Completed rendering or atomic file-delivery evidence.
#[derive(Debug, Clone, serde::Serialize)]
#[serde(untagged)]
pub enum ExportOutcome {
    Inline {
        format: ExportFormat,
        roadmap: String,
    },
    File {
        format: ExportFormat,
        path: PathBuf,
        bytes: usize,
        written: bool,
    },
}
