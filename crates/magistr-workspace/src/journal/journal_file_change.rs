use crate::file_content::FileContent;

use super::JournalFileKind;

/// One file's exact bytes before and after a mutation; absence uses empty bytes.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct JournalFileChange {
    pub kind: JournalFileKind,
    pub path: String,
    pub before: FileContent,
    pub after: FileContent,
}
