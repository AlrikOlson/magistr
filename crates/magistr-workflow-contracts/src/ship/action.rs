//! Canonical record and its related state vocabulary; these serialize and change together.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ActionType {
    Code,
    Test,
    Debug,
    Research,
    Config,
    Refactor,
    Review,
}

impl ActionType {
    /// The wire spelling (snake_case), matching the serde representation.
    #[must_use]
    pub const fn as_wire(&self) -> &'static str {
        match self {
            ActionType::Code => "code",
            ActionType::Test => "test",
            ActionType::Debug => "debug",
            ActionType::Research => "research",
            ActionType::Config => "config",
            ActionType::Refactor => "refactor",
            ActionType::Review => "review",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Action {
    pub id: u32,
    pub task_id: String,
    pub timestamp: String,
    #[serde(rename = "type")]
    pub action_type: ActionType,
    pub description: String,
    #[serde(default)]
    pub files_touched: Vec<String>,
    /// Tools the agent CHOSE TO MENTION — a self-report, not a measurement.
    /// See the same warning on `think::domain::step::Step::tools_used`; real
    /// invocation counts live in the workflow dispatcher.
    #[serde(default)]
    pub tools_used: Vec<String>,
    pub result: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub think_step: Option<u32>,
}

#[cfg(test)]
#[path = "action.tests.rs"]
mod tests;
