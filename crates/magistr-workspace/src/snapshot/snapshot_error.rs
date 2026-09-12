use std::path::PathBuf;

use thiserror::Error;

/// Every way a snapshot, validation, export or receipt can refuse. Each is
/// a refusal, never a degraded answer: a snapshot is complete or absent.
#[derive(Debug, Error)]
pub enum SnapshotError {
    #[error("workspace root does not exist: {0}")]
    RootMissing(PathBuf),
    #[error("workspace generation cannot be observed: {0}")]
    Generation(String),
    #[error("explicit input {path:?} is refused: {reason}")]
    InvalidInput { path: String, reason: String },
    #[error(
        "source selection exceeds {limit} entries; the snapshot is refused rather than truncated"
    )]
    SelectionTruncated { limit: usize },
    #[error("{path}: {detail}")]
    Io { path: String, detail: String },
    #[error("{0}: changed while its bytes were being hashed")]
    ChangedDuringCapture(String),
    #[error("path is not UTF-8: {0}")]
    NonUtf8Path(PathBuf),
    #[error(
        "a snapshot of {expected} cannot be validated against {actual}: another workspace never substitutes"
    )]
    ForeignWorkspace { expected: PathBuf, actual: PathBuf },
    #[error("{0}")]
    UnsupportedCertification(String),
    #[error("isolated export does not reproduce the origin observation: {0}")]
    ExportMismatch(String),
}
