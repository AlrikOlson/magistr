/// The file's existence transition in a write batch.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum JournalFileKind {
    Create,
    Write,
    Delete,
}

impl JournalFileKind {
    pub(super) fn as_str(self) -> &'static str {
        match self {
            Self::Create => "create",
            Self::Write => "write",
            Self::Delete => "delete",
        }
    }
}
