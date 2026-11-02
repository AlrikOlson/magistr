//! Canonical record and its related state vocabulary; these serialize and change together.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// How much project-wide verification a goal requires before its
/// plan task can complete. New work defaults to focused checks selected
/// for the change's scope. Existing stored goals retain their contract.
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum VerificationMode {
    Full,
    #[default]
    Focused,
}

impl VerificationMode {
    /// Preserve the verification promised by goals stored before this field existed.
    pub fn legacy_full() -> Self {
        Self::Full
    }

    /// Full is omitted on disk for the existing durable representation.
    pub fn is_full(&self) -> bool {
        matches!(self, Self::Full)
    }
}
