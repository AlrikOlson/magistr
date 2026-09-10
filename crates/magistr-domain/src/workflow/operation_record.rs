use alloc::string::String;
use serde::{Deserialize, Serialize};

use crate::identity::AttemptId;

use super::{OperationKind, OperationOutcome};

/// One attempt of one operation, as the ledger remembers it.
///
/// `external` is the runtime's own identity for the attempt — a process id,
/// a provider session — kept apart from the command identity so a retry is a
/// new attempt of the same command, never the same external handle.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OperationRecord {
    pub attempt: AttemptId,
    pub kind: OperationKind,
    /// What the effect is aimed at, in the adapter's own words
    /// (`complete_chunk:arch-13`, `gate:test@task:arch-13-verify`).
    pub target: String,
    pub outcome: OperationOutcome,
    #[serde(deserialize_with = "required_option")]
    pub external: Option<String>,
}

fn required_option<'de, D, T>(deserializer: D) -> Result<Option<T>, D::Error>
where
    D: serde::Deserializer<'de>,
    T: Deserialize<'de>,
{
    Option::<T>::deserialize(deserializer)
}

impl OperationRecord {
    #[must_use]
    pub fn is_settled(&self) -> bool {
        self.outcome.is_settled()
    }
}
