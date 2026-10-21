//! Canonical record and its related state vocabulary; these serialize and change together.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// How much project-wide verification an objective requires before its
/// roadmap chunk can complete. New work defaults to focused checks selected
/// for the change's scope. Existing stored objectives retain their contract.
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum VerificationMode {
    Full,
    #[default]
    Focused,
}

impl VerificationMode {
    /// Preserve the verification promised by objectives stored before this field existed.
    pub fn legacy_full() -> Self {
        Self::Full
    }

    /// Full is omitted on disk for the existing durable representation.
    pub fn is_full(&self) -> bool {
        matches!(self, Self::Full)
    }
}
