use crate::file_content::FileContent;

use super::JournalFileKind;

/// Durable hashes remain visible after eviction removes a content snapshot.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct JournalFileRecord {
    pub kind: JournalFileKind,
    pub path: String,
    pub before_hash: String,
    pub after_hash: String,
    pub before: Option<FileContent>,
    pub after: Option<FileContent>,
}
