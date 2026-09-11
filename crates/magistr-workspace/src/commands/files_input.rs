use serde::{Deserialize, Serialize};

use super::FilesOp;

/// One files call. Serialization yields the sealed intent bytes; `confirm`,
/// `dry_run`, `auto_apply` and `budget` never enter the confirmation binding.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FilesInput {
    pub op: FilesOp,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub from: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub to: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub expected_hash: Option<String>,
    #[serde(default, skip_serializing)]
    pub confirm: Option<String>,
    #[serde(default, skip_serializing)]
    pub dry_run: bool,
    #[serde(default, skip_serializing)]
    pub auto_apply: bool,
    #[serde(default = "default_budget", skip_serializing)]
    pub budget: u64,
}

impl FilesInput {
    #[must_use]
    pub fn new(op: FilesOp) -> Self {
        Self {
            op,
            from: None,
            to: None,
            expected_hash: None,
            confirm: None,
            dry_run: false,
            auto_apply: false,
            budget: default_budget(),
        }
    }
}

fn default_budget() -> u64 {
    4000
}
