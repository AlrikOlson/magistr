use serde::{Deserialize, Serialize};

/// One script call, mirroring the retained wire shape. Serialization yields
/// the intent bytes the confirmation token binds: the source and the dry-run
/// decision, exactly the retained `argsHash` inputs; `confirm`, `autoApply`,
/// the caps and `budget` are excluded so the identical intent re-invoked with
/// a token stays the same sealed request.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScriptInput {
    pub source: String,
    /// Preview by default (§9.7): a script never applies unless asked to.
    #[serde(default = "default_true")]
    pub dry_run: bool,
    #[serde(default, skip_serializing)]
    pub confirm: Option<String>,
    #[serde(default, skip_serializing)]
    pub auto_apply: bool,
    /// Requested fuel per round; clamps downward only.
    #[serde(default, skip_serializing)]
    pub fuel: Option<u64>,
    /// Requested memory cap; clamps downward only.
    #[serde(default, skip_serializing)]
    pub memory_cap_bytes: Option<usize>,
    #[serde(default = "default_budget", skip_serializing)]
    pub budget: u64,
}

impl ScriptInput {
    #[must_use]
    pub fn new(source: impl Into<String>) -> Self {
        Self {
            source: source.into(),
            dry_run: true,
            confirm: None,
            auto_apply: false,
            fuel: None,
            memory_cap_bytes: None,
            budget: default_budget(),
        }
    }
}

fn default_true() -> bool {
    true
}

fn default_budget() -> u64 {
    4000
}
