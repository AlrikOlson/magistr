use serde::{Deserialize, Serialize};

/// What a manifest path was when it was observed. Text and binary files are
/// distinct kinds (like Nix's content addressing, unlike Git's blobs), a
/// symlink is hashed by its target rather than followed, and an explicit
/// input that did not exist is recorded as missing so its appearance is a
/// change.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EntryKind {
    Text,
    Binary,
    Symlink,
    Missing,
}

impl EntryKind {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Text => "text",
            Self::Binary => "binary",
            Self::Symlink => "symlink",
            Self::Missing => "missing",
        }
    }
}
