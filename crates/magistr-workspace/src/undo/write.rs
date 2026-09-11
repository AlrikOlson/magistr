use crate::file_content::FileContent;

/// One exact-byte target for the later admitted mutation pipeline.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct UndoWrite {
    pub path: String,
    /// None deletes the path; Some(empty) creates or restores an empty file.
    pub content: Option<FileContent>,
    /// Hash of the observed bytes (empty bytes when absent), for compatibility.
    pub expected_hash: String,
    /// Distinguishes a missing path from an existing empty file at admission.
    pub expected_exists: bool,
}
