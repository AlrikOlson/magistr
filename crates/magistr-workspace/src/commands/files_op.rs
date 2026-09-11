use serde::{Deserialize, Serialize};

/// The retained typed file operations.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FilesOp {
    Move,
    Copy,
    Create,
    Delete,
    Mkdir,
}

impl FilesOp {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Move => "move",
            Self::Copy => "copy",
            Self::Create => "create",
            Self::Delete => "delete",
            Self::Mkdir => "mkdir",
        }
    }
}
