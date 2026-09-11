use super::{BaselineSource, ExpectedState};
use crate::file_content::FileContent;

/// A caller-resolved exact file target, checked again by the transaction boundary.
#[derive(Clone, Debug)]
pub struct TransactionChange {
    pub path: String,
    pub expected: ExpectedState,
    /// None deletes; Some(empty) creates or retains an empty file.
    pub content: Option<FileContent>,
    /// A move/copy may retain the source file's existing parser diagnostics.
    pub baseline: Option<BaselineSource>,
}
