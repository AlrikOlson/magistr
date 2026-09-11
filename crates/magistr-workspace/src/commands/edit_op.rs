use serde::{Deserialize, Serialize};

/// The retained edit verbs: symbol verbs, structural rewrite and anchored text.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EditOp {
    ReplaceBody,
    InsertAfter,
    InsertBefore,
    Rename,
    Rewrite,
    ReplaceText,
}

impl EditOp {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::ReplaceBody => "replace_body",
            Self::InsertAfter => "insert_after",
            Self::InsertBefore => "insert_before",
            Self::Rename => "rename",
            Self::Rewrite => "rewrite",
            Self::ReplaceText => "replace_text",
        }
    }

    #[must_use]
    pub const fn is_symbol_verb(self) -> bool {
        matches!(
            self,
            Self::ReplaceBody | Self::InsertAfter | Self::InsertBefore | Self::Rename
        )
    }
}
