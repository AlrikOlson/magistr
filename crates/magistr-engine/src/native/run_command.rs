//! The execution-trace verbs an attached client can ask its owner for.

use magistr_workflow::run::domain::artifact::Artifact;
use magistr_workflow::run::domain::progress::ProgressType;
use magistr_workflow::run::domain::step::StepType;
use serde::{Deserialize, Serialize};

/// One execution-trace mutation, argument for argument the
/// [`NativeEngines`](super::NativeEngines) verb of the same name.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "verb", rename_all = "snake_case")]
pub enum RunCommand {
    /// Explicitly remove settled check history outside the current run.
    CompactCheckResults,
    /// `run_open`.
    OpenRun {
        description: String,
        acceptance_criteria: Vec<String>,
        constraints: Vec<String>,
        scope: String,
    },
    /// `add_step`.
    AddStep {
        id: String,
        title: String,
        step_type: StepType,
        estimate: Option<String>,
    },
    /// `start_step`.
    StartStep { step_id: String },
    /// `record_progress`.
    RecordProgress {
        step_id: Option<String>,
        progress_type: ProgressType,
        description: String,
        files_touched: Vec<String>,
        tools_used: Vec<String>,
        note: Option<u32>,
    },
    /// `run_record_check`. The report, when given, is a `(format, path)`
    /// the owner reads — so the path must be one the owner's process can
    /// open, which for a local attachment is the same filesystem.
    RecordCheck {
        step_id: Option<String>,
        check: String,
        command: String,
        exit_code: i32,
        details: String,
        required: bool,
        report: Option<(String, String)>,
    },
    /// `finish_step`.
    CompleteStep {
        step_id: String,
        artifacts: Vec<Artifact>,
    },
    /// `run_finish`.
    Finalize {
        artifacts: Vec<Artifact>,
        summary: Option<String>,
    },
}
