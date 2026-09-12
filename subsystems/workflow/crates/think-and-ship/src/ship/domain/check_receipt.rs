use serde::{Deserialize, Serialize};

use crate::ship::application::{
    CheckExecution, CheckRequest, SourceFreshness, SourceObservation, TaskAdmission,
};

use super::check::CheckType;
use super::receipt_settlement::ReceiptSettlement;

/// One durable attempt at one gate: the exact task instance and run it was
/// admitted on, its attempt number within that instance and gate, the gate
/// contract as requested, the acceptance revision and source observation
/// current at admission, and — once settled — how the command ended, when,
/// whether the source still matched, and whether the workflow applied the
/// result or kept it stale. Written at admission and rewritten once at
/// settlement under the same id, so a late result always has a record to
/// land in whatever the objective has become.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CheckReceipt {
    pub id: String,
    pub target: TaskAdmission,
    pub attempt: u64,
    pub gate: String,
    pub check_type: CheckType,
    pub command: Option<String>,
    pub required: bool,
    pub acceptance_revision: String,
    pub source: Option<SourceObservation>,
    pub admitted_at: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub settled_at: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub duration_ms: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub execution: Option<CheckExecution>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub freshness: Option<SourceFreshness>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub settlement: Option<ReceiptSettlement>,
}

impl CheckReceipt {
    /// A receipt as admitted, before any command ran.
    #[must_use]
    pub fn admitted(
        id: String,
        target: TaskAdmission,
        attempt: u64,
        request: &CheckRequest,
        acceptance_revision: String,
        source: Option<SourceObservation>,
        admitted_at: String,
    ) -> Self {
        Self {
            id,
            target,
            attempt,
            gate: request.name.clone(),
            check_type: request.check_type.clone(),
            command: request.command.clone(),
            required: request.required,
            acceptance_revision,
            source,
            admitted_at,
            settled_at: None,
            duration_ms: None,
            execution: None,
            freshness: None,
            settlement: None,
        }
    }

    /// Whether the settled execution passed.
    #[must_use]
    pub fn passed(&self) -> bool {
        self.execution.as_ref().is_some_and(CheckExecution::passed)
    }

    /// Whether a server ran the command (never true for a self-reported check).
    #[must_use]
    pub fn verified(&self) -> bool {
        self.execution
            .as_ref()
            .is_some_and(CheckExecution::verified)
    }

    /// Whether the workflow applied this receipt's result to its task.
    #[must_use]
    pub fn is_applied(&self) -> bool {
        matches!(self.settlement, Some(ReceiptSettlement::Applied))
    }
}
