use super::VerificationStatus;
use crate::file_content::FileContent;

/// Complete finalized byte preview, before confirmation; never budget-truncated.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FilePreview {
    pub path: String,
    pub before: Option<FileContent>,
    pub after: Option<FileContent>,
    pub diagnostics: Vec<String>,
    pub verification: VerificationStatus,
    /// Actual formatter lane, including skipped and degraded decisions.
    pub formatting: String,
}
