use super::{FilePreview, TransactionRequest, snapshot::Snapshot};
use std::path::PathBuf;

/// Immutable preparation capability. Callers cannot substitute final bytes,
/// verification results, order, identity, anchors, or formatting decisions.
pub struct PreparedTransaction {
    pub(super) request: TransactionRequest,
    pub(super) preview: Vec<FilePreview>,
    pub(super) token: String,
    pub(super) paths: Vec<PathBuf>,
    pub(super) snapshots: Vec<Snapshot>,
    pub(super) baselines: Vec<(String, PathBuf, Snapshot)>,
}

impl PreparedTransaction {
    #[must_use]
    pub fn preview(&self) -> &[FilePreview] {
        &self.preview
    }
    #[must_use]
    pub fn confirmation_token(&self) -> &str {
        &self.token
    }
}
