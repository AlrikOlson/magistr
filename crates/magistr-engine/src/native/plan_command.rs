//! The plan verbs an attached client can ask its owner for.

use magistr_workflow::content::StructuredContent;
use magistr_workflow::plan::domain::{TaskStatus, WaitingReason};
use serde::{Deserialize, Serialize};

/// One plan mutation, argument for argument the
/// [`NativeEngines`](super::NativeEngines) verb of the same name.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "verb", rename_all = "snake_case")]
pub enum PlanCommand {
    /// `plan_start_task`.
    StartTask { id: String },
    /// `plan_complete_task`.
    CompleteTask {
        id: String,
        step_ref: Option<String>,
    },
    /// `plan_add_task`.
    AddTask {
        id: String,
        title: String,
        name: String,
        status: TaskStatus,
        priority: u32,
        description: String,
        acceptance: Vec<String>,
        deps: Vec<String>,
        content: Option<StructuredContent>,
        feature: Option<String>,
    },
    /// `plan_update_task`.
    UpdateTask {
        id: String,
        title: Option<String>,
        priority: Option<u32>,
        description: Option<String>,
        acceptance: Option<Vec<String>>,
        deps: Option<Vec<String>>,
        content: Option<StructuredContent>,
    },
    /// `plan_clear_waiting_on`.
    ClearWaitingOn { id: String },
    /// `plan_set_waiting_on`.
    SetWaitingOn {
        id: String,
        kind: WaitingReason,
        reason: String,
        evidence: Option<String>,
    },
    /// `plan_set_status`.
    SetStatus { id: String, to: TaskStatus },
    /// `plan_retire_task`.
    RetireTask { id: String, reason: String },
    /// `plan_reprioritize`.
    Reprioritize {
        id: String,
        suggested_priority: u32,
        reason: String,
    },
    /// `plan_propose_title`.
    ProposeTitle {
        id: String,
        suggested_title: String,
        reason: String,
        source: String,
    },
    /// `plan_propose_status`.
    ProposeStatus {
        id: String,
        suggested_status: TaskStatus,
        reason: String,
        source: String,
    },
    /// `plan_link`.
    Link { id: String, cross_ref: String },
    /// `plan_set_feature`.
    SetFeature { id: String, feature: Option<String> },
    /// `plan_resolve_title_proposal`.
    ResolveTitleProposal { id: String, accept: bool },
    /// `plan_resolve_status_proposal`.
    ResolveStatusProposal { id: String, accept: bool },
    /// `plan_resolve_reprioritize`.
    ResolveReprioritize { id: String, accept: bool },
}
