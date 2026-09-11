use serde::{Deserialize, Serialize};

use super::EditOp;

/// One edit call, mirroring the retained wire shape. Serialization yields the
/// exact intent bytes the confirmation token binds: `confirm`, `dry_run`,
/// `auto_apply` and `budget` are excluded so the identical intent re-invoked
/// with a token or preview flag stays the same sealed request.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EditInput {
    pub op: EditOp,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub file: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub symbol: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub content: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub new_name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pattern: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rewrite_pattern: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lang: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub old: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub new: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub occurrence: Option<usize>,
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

impl EditInput {
    #[must_use]
    pub fn new(op: EditOp) -> Self {
        Self {
            op,
            file: None,
            symbol: None,
            content: None,
            new_name: None,
            pattern: None,
            rewrite_pattern: None,
            path: None,
            lang: None,
            old: None,
            new: None,
            occurrence: None,
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
