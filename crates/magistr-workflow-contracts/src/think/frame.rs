//! Typed mutation records; emission belongs to the workflow host.

use super::{BranchStatus, ThinkStep};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ThinkFrame {
    /// A new step was appended to the trace (either main line or branch).
    StepAppended {
        #[serde(skip_serializing_if = "Option::is_none")]
        session_id: Option<String>,
        step: Box<ThinkStep>,
    },
    /// An older step's `revised_by` pointer was set. The revising step
    /// itself is delivered via a separate [`Self::StepAppended`] frame.
    StepRevised {
        #[serde(skip_serializing_if = "Option::is_none")]
        session_id: Option<String>,
        revised_step: u32,
        by_step: u32,
    },
    /// A branch transitioned to a new status. `merged_into` is set only
    /// when `status == Merged` and the caller named a synthesis step.
    BranchStatusChanged {
        #[serde(skip_serializing_if = "Option::is_none")]
        session_id: Option<String>,
        branch_id: String,
        status: BranchStatus,
        #[serde(skip_serializing_if = "Option::is_none")]
        merged_into: Option<u32>,
    },
    /// The `pinned` flag on a step changed.
    PinChanged {
        #[serde(skip_serializing_if = "Option::is_none")]
        session_id: Option<String>,
        step_number: u32,
        pinned: bool,
    },
    /// The most-recent step's `estimated_total` was revised in place.
    EstimateRevised {
        #[serde(skip_serializing_if = "Option::is_none")]
        session_id: Option<String>,
        old: u32,
        new: u32,
        #[serde(skip_serializing_if = "Option::is_none")]
        reason: Option<String>,
    },
    /// The entire trace was wiped.
    Cleared,
}
