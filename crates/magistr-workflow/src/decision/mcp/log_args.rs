//! Explicit selection for reasoning reads.
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema, PartialEq)]
pub struct LogArgs {
    /// Named log to read. Omit for the project's default log.
    #[serde(default)]
    pub log_id: Option<String>,
}
