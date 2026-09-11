use serde::Serialize;

use super::WriteAction;
use crate::transaction::{FileDisposition, FilePreview};

/// One result row per change, in input order. `applied` is false on a dry run
/// and for unchanged content; `diff` is the unified diff of the final bytes.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WriteReport {
    pub path: String,
    pub action: WriteAction,
    pub applied: bool,
    pub diff: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content_hash: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub journal_seq: Option<i64>,
    /// Parser diagnostics that remain after an allowed write to an already
    /// broken file; never a blessing of that file.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub diagnostics: Vec<String>,
    /// The formatter lane actually taken, including skipped and degraded decisions.
    pub formatting: String,
    /// The observed effect after application; `not_applied` on a dry run.
    pub disposition: String,
}

impl WriteReport {
    pub(super) fn from_preview(
        preview: &FilePreview,
        disposition: FileDisposition,
        journal_seq: Option<i64>,
    ) -> Self {
        let action = match (&preview.before, &preview.after) {
            _ if preview.before == preview.after => WriteAction::Unchanged,
            (None, _) => WriteAction::Created,
            (_, None) => WriteAction::Deleted,
            _ => WriteAction::Modified,
        };
        Self {
            path: preview.path.clone(),
            action,
            applied: disposition == FileDisposition::Applied,
            diff: super::diff::unified_diff(
                &preview.path,
                preview.before.as_ref(),
                preview.after.as_ref(),
            ),
            content_hash: preview.after.as_ref().map(|after| after.content_hash()),
            journal_seq,
            diagnostics: preview.diagnostics.clone(),
            formatting: preview.formatting.clone(),
            disposition: match disposition {
                FileDisposition::Unchanged => "unchanged",
                FileDisposition::NotApplied => "not_applied",
                FileDisposition::Applied => "applied",
                FileDisposition::RolledBack => "rolled_back",
                FileDisposition::Indeterminate => "indeterminate",
            }
            .into(),
        }
    }
}
