use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// How much project-wide verification an objective requires before its
/// roadmap chunk can complete. Full preserves the stored-state default;
/// focused uses recorded checks selected for a bounded change's scope.
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum VerificationMode {
    #[default]
    Full,
    Focused,
}

impl VerificationMode {
    /// Whether this is the default, every-gate mode — the serializer skips
    /// the field when it is, so objectives written before the mode existed
    /// and objectives written at the default read the same on disk.
    pub(crate) fn is_full(&self) -> bool {
        matches!(self, Self::Full)
    }
}
