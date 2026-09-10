//! The execution-trace verbs an attached client can ask its owner for.

use serde::{Deserialize, Serialize};
use think_and_ship::ship::domain::action::ActionType;
use think_and_ship::ship::domain::artifact::Artifact;
use think_and_ship::ship::domain::task::TaskType;

/// One execution-trace mutation, argument for argument the
/// [`NativeEngines`](super::NativeEngines) verb of the same name.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "verb", rename_all = "snake_case")]
pub enum ShipCommand {
    /// `ship_set_objective`.
    SetObjective {
        description: String,
        acceptance_criteria: Vec<String>,
        constraints: Vec<String>,
        scope: String,
    },
    /// `ship_add_task`.
    AddTask {
        id: String,
        title: String,
        task_type: TaskType,
        estimate: Option<String>,
    },
    /// `ship_start_task`.
    StartTask { task_id: String },
    /// `ship_record_action`.
    RecordAction {
        task_id: Option<String>,
        action_type: ActionType,
        description: String,
        files_touched: Vec<String>,
        tools_used: Vec<String>,
        think_step: Option<u32>,
    },
    /// `ship_record_gate`. The report, when given, is a `(format, path)`
    /// the owner reads — so the path must be one the owner's process can
    /// open, which for a local attachment is the same filesystem.
    RecordGate {
        task_id: Option<String>,
        gate: String,
        command: String,
        exit_code: i32,
        details: String,
        required: bool,
        report: Option<(String, String)>,
    },
    /// `ship_complete_task`.
    CompleteTask {
        task_id: String,
        artifacts: Vec<Artifact>,
    },
    /// `ship_finalize`.
    Finalize {
        artifacts: Vec<Artifact>,
        summary: Option<String>,
    },
}
