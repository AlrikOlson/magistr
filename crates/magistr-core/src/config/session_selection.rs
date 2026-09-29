//! Serializable non-secret agent settings carried by a run and its checkpoint.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SessionSelection {
    pub agent: String,
    pub model: Option<String>,
    pub effort: Option<String>,
    #[serde(default)]
    pub sources: BTreeMap<String, String>,
}

impl SessionSelection {
    pub fn source(&self, key: &str) -> &str {
        self.sources
            .get(key)
            .map(String::as_str)
            .unwrap_or("saved run")
    }
}
