//! Explicit selection for reasoning reads.
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema, PartialEq)]
pub struct SessionArgs {
    /// Named session to read. Omit for the project's default session.
    #[serde(default)]
    pub session_id: Option<String>,
}
