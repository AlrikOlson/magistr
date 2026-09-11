use serde::{Deserialize, Serialize};

/// Session scope never rewinds another session's operations; workspace scope
/// is the explicit cross-session rewind.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum UndoScope {
    #[default]
    Session,
    Workspace,
}

impl UndoScope {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Session => "session",
            Self::Workspace => "workspace",
        }
    }
}
