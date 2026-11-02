//! Typed mutation records; emission belongs to the workflow host.

use super::{Check, Goal, Progress, Step};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum RunFrame {
    GoalSet { goal: Goal },
    StepAdded { step_id: String, title: String },
    StepStarted { step_id: String },
    ProgressRecorded { step_id: String, progress: Progress },
    CheckRecorded { step_id: String, check: Check },
    StepDone { step: Step },
    StepWaiting { step_id: String, reason: String },
    RunFinished { warnings: Vec<String> },
    Cleared,
}
