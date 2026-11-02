use super::check::CheckType;
use super::check_result_settlement::CheckResultSettlement;
use super::check_result_stage::CheckResultStage;
use crate::run::application::{
    CheckExecution, CheckOptions, CheckOutcome, CheckRequest, SourceFreshness, SourceObservation,
    StepAdmission,
};
use serde::{Deserialize, Serialize};

/// Canonical admission and full execution result. Step checks refer back to
/// this record; retrying settlement never executes or invents another attempt.
///
/// Reading accepts every shape the ledger has written: check_results from before
/// the staged settlement model are converted on read (see [`StoredCheckResult`]).
#[derive(Debug, Clone, PartialEq, Serialize, schemars::JsonSchema)]
pub struct CheckResult {
    pub id: String,
    pub target: StepAdmission,
    pub attempt: u64,
    pub gate: String,
    pub check_type: CheckType,
    pub command: Option<String>,
    pub required: bool,
    /// Resolved execution controls, including the deadline and its origin.
    #[serde(default)]
    pub options: crate::run::application::CheckOptions,
    pub acceptance_revision: String,
    pub source: Option<SourceObservation>,
    pub admitted_at: String,
    pub stage: CheckResultStage,
    pub recorded_at: Option<String>,
    pub settled_at: Option<String>,
    pub duration_ms: Option<u64>,
    pub outcome: Option<CheckOutcome>,
    pub freshness: Option<SourceFreshness>,
    pub settlement: Option<CheckResultSettlement>,
}

impl CheckResult {
    #[must_use]
    #[allow(clippy::too_many_arguments)]
    pub fn admitted(
        id: String,
        target: StepAdmission,
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
            stage: CheckResultStage::Admitted,
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
        matches!(self.settlement, Some(CheckResultSettlement::Applied))
    }
}

impl<'de> Deserialize<'de> for CheckResult {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        StoredCheckResult::deserialize(deserializer).map(StoredCheckResult::into_current)
    }
}

/// Every check_result shape the ledger has written. CheckResults from before the
/// staged settlement model carry the execution at the top level and no
/// `stage`, `outcome` or `recorded_at`; they settled in one step, so their
/// settlement time is also their recording time. Converting them on read
/// keeps that history readable without rewriting it, and compaction retires
/// it like any other settled check_result.
#[derive(Deserialize)]
struct StoredCheckResult {
    id: String,
    target: StepAdmission,
    attempt: u64,
    gate: String,
    check_type: CheckType,
    command: Option<String>,
    required: bool,
    #[serde(default)]
    options: CheckOptions,
    acceptance_revision: String,
    source: Option<SourceObservation>,
    admitted_at: String,
    #[serde(default)]
    stage: Option<CheckResultStage>,
    #[serde(default)]
    recorded_at: Option<String>,
    #[serde(default)]
    settled_at: Option<String>,
    #[serde(default)]
    duration_ms: Option<u64>,
    #[serde(default)]
    outcome: Option<CheckOutcome>,
    /// The pre-staged location of the execution.
    #[serde(default)]
    execution: Option<CheckExecution>,
    #[serde(default)]
    freshness: Option<SourceFreshness>,
    #[serde(default)]
    settlement: Option<CheckResultSettlement>,
}

impl StoredCheckResult {
    fn into_current(self) -> CheckResult {
        let command = self.command;
        let outcome = self.outcome.or_else(|| {
            self.execution.map(|execution| CheckOutcome {
                passed: execution.passed(),
                details: String::new(),
                verified: execution.verified(),
                command: command.clone(),
                exit_code: execution.exit_code(),
                execution,
                report: None,
                results: None,
            })
        });
        let recorded_at = self
            .recorded_at
            .or_else(|| outcome.as_ref().and(self.settled_at.clone()));
        let stage = self.stage.unwrap_or(if self.settlement.is_some() {
            CheckResultStage::Settled
        } else if outcome.is_some() {
            CheckResultStage::Recorded
        } else {
            CheckResultStage::Admitted
        });
        CheckResult {
            id: self.id,
            target: self.target,
            attempt: self.attempt,
            gate: self.gate,
            check_type: self.check_type,
            command,
            required: self.required,
            options: self.options,
            acceptance_revision: self.acceptance_revision,
            source: self.source,
            admitted_at: self.admitted_at,
            stage,
            recorded_at,
            settled_at: self.settled_at,
            duration_ms: self.duration_ms,
            outcome,
            freshness: self.freshness,
            settlement: self.settlement,
        }
    }
}

#[cfg(test)]
#[path = "check_result.tests.rs"]
mod tests;
