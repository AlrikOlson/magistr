use serde::{Deserialize, Serialize};

use super::{SearchInclude, SearchMode};

/// Typed native search input using the retained tool's defaults.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct SearchInput {
    pub query: String,
    #[serde(default)]
    pub mode: SearchMode,
    pub kind: Option<Vec<String>>,
    pub path: Option<String>,
    pub include: Option<SearchInclude>,
    #[serde(default = "default_context")]
    pub context: usize,
    #[serde(default = "default_budget")]
    pub budget: u64,
}

impl SearchInput {
    pub fn new(query: impl Into<String>) -> Self {
        Self {
            query: query.into(),
            mode: SearchMode::Auto,
            kind: None,
            path: None,
            include: None,
            context: default_context(),
            budget: default_budget(),
        }
    }
}

fn default_context() -> usize {
    2
}

fn default_budget() -> u64 {
    4000
}
