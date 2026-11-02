//! Canonical record and its related state vocabulary; these serialize and change together.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::artifact::Artifact;
use super::check::Check;
use super::progress::Progress;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum StepStatus {
    Planned,
    Active,
    Waiting,
    Done,
    Skipped,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum StepType {
    Implement,
    Test,
    Review,
    Config,
    Docs,
    Research,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Step {
    pub id: String,
    pub title: String,
    #[serde(rename = "type")]
    pub step_type: StepType,
    pub status: StepStatus,
    pub estimate: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub started_at: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub completed_at: Option<String>,
    #[serde(default)]
    pub artifacts: Vec<Artifact>,
    #[serde(default)]
    pub checks: Vec<Check>,
    #[serde(default)]
    pub progress: Vec<Progress>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub waiting_reason: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub decision_thread: Option<String>,
}

#[cfg(test)]
#[path = "step.tests.rs"]
mod tests;
