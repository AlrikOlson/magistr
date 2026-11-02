//! Canonical record and its related state vocabulary; these serialize and change together.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ProgressType {
    Code,
    Test,
    Debug,
    Research,
    Config,
    Refactor,
    Review,
}

impl ProgressType {
    /// The wire spelling (snake_case), matching the serde representation.
    #[must_use]
    pub const fn as_wire(&self) -> &'static str {
        match self {
            ProgressType::Code => "code",
            ProgressType::Test => "test",
            ProgressType::Debug => "debug",
            ProgressType::Research => "research",
            ProgressType::Config => "config",
            ProgressType::Refactor => "refactor",
            ProgressType::Review => "review",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Progress {
    pub id: u32,
    pub step_id: String,
    pub timestamp: String,
    #[serde(rename = "type")]
    pub progress_type: ProgressType,
    pub description: String,
    #[serde(default)]
    pub files_touched: Vec<String>,
    /// Tools the agent CHOSE TO MENTION — a self-report, not a measurement.
    /// See the same warning on `decision::domain::note::Step::tools_used`; real
    /// invocation counts live in the workflow dispatcher.
    #[serde(default)]
    pub tools_used: Vec<String>,
    pub result: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub note: Option<u32>,
}

#[cfg(test)]
#[path = "progress.tests.rs"]
mod tests;
