use super::check::CheckType;
use super::receipt_settlement::ReceiptSettlement;
use super::receipt_stage::ReceiptStage;
use crate::ship::application::{
    CheckExecution, CheckOutcome, CheckRequest, SourceFreshness, SourceObservation, TaskAdmission,
};
use serde::{Deserialize, Serialize};

/// Canonical admission and full execution result. Task checks refer back to
/// this record; retrying settlement never executes or invents another attempt.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct CheckReceipt {
    pub id: String,
    pub target: TaskAdmission,
    pub attempt: u64,
    pub gate: String,
    pub check_type: CheckType,
    pub command: Option<String>,
    pub required: bool,
    /// Resolved execution controls, including the deadline and its origin.
    #[serde(default)]
    pub options: crate::ship::application::CheckOptions,
    pub acceptance_revision: String,
    pub source: Option<SourceObservation>,
    pub admitted_at: String,
    pub stage: ReceiptStage,
    pub recorded_at: Option<String>,
    pub settled_at: Option<String>,
    pub duration_ms: Option<u64>,
    pub outcome: Option<CheckOutcome>,
    pub freshness: Option<SourceFreshness>,
    pub settlement: Option<ReceiptSettlement>,
}

impl CheckReceipt {
    #[must_use]
    #[allow(clippy::too_many_arguments)]
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
            options: request.options.clone(),
            acceptance_revision,
            source,
            admitted_at,
            stage: ReceiptStage::Admitted,
            recorded_at: None,
            settled_at: None,
            duration_ms: None,
            outcome: None,
            freshness: None,
            settlement: None,
        }
    }

    #[must_use]
    pub fn execution(&self) -> Option<&CheckExecution> {
        self.outcome.as_ref().map(|outcome| &outcome.execution)
    }

    #[must_use]
    pub fn passed(&self) -> bool {
        self.execution().is_some_and(CheckExecution::passed)
    }

    #[must_use]
    pub fn verified(&self) -> bool {
        self.execution().is_some_and(CheckExecution::verified)
    }

    #[must_use]
    pub fn is_applied(&self) -> bool {
        matches!(self.settlement, Some(ReceiptSettlement::Applied))
    }
}
