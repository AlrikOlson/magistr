use serde::{Deserialize, Serialize};

use super::UndoScope;
use crate::undo::UndoConflictPolicy;

/// One rewind request. Rewinds are never confirm-gated; conflicts are surfaced
/// as rows unless the caller chose keep-current or force explicitly.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UndoInput {
    /// How many operations (write batches, not files) to rewind.
    #[serde(default = "default_steps")]
    pub steps: usize,
    /// Exact journal sequences instead of the newest `steps`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ops: Option<Vec<i64>>,
    #[serde(default)]
    pub scope: UndoScope,
    #[serde(default, with = "policy_wire")]
    pub on_conflict: UndoConflictPolicy,
    #[serde(default = "default_budget")]
    pub budget: u64,
}

impl Default for UndoInput {
    fn default() -> Self {
        Self {
            steps: default_steps(),
            ops: None,
            scope: UndoScope::Session,
            on_conflict: UndoConflictPolicy::Surface,
            budget: default_budget(),
        }
    }
}

fn default_steps() -> usize {
    1
}

fn default_budget() -> u64 {
    4000
}

mod policy_wire {
    use crate::undo::UndoConflictPolicy;
    use serde::{Deserialize, Deserializer, Serializer};

    pub fn serialize<S: Serializer>(
        policy: &UndoConflictPolicy,
        serializer: S,
    ) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(match policy {
            UndoConflictPolicy::Surface => "surface",
            UndoConflictPolicy::KeepCurrent => "keepCurrent",
            UndoConflictPolicy::Force => "force",
        })
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(
        deserializer: D,
    ) -> Result<UndoConflictPolicy, D::Error> {
        match String::deserialize(deserializer)?.as_str() {
            "surface" => Ok(UndoConflictPolicy::Surface),
            "keepCurrent" => Ok(UndoConflictPolicy::KeepCurrent),
            "force" => Ok(UndoConflictPolicy::Force),
            other => Err(serde::de::Error::custom(format!(
                "onConflict must be surface, keepCurrent or force, got {other:?}"
            ))),
        }
    }
}
