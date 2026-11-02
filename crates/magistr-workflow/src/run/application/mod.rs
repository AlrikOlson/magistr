//! Typed execution use cases shared by native clients and MCP adapters.

mod accepted_check_result;
mod approval_opening;
mod approval_request;
mod approval_resolution;
mod approvals;
mod check_admission;
mod check_execution;
mod check_outcome;
mod check_request;
mod check_settlement;
mod checks;
pub(crate) mod command_runner;
mod commands;
mod completion;
mod completion_evidence;
mod completion_refusal;
mod evidence_port;
mod gate_contract;
mod goal_set;
mod observation_drift;
mod outstanding_gate;
mod plan_change;
mod plan_snapshot;
mod recovered_failure;
mod run_error;
mod run_report;
mod snapshot;
mod source_freshness;
mod source_observation;
mod source_witness;
mod status;
mod step_admission;

pub use accepted_check_result::AcceptedCheckResult;
pub use approval_request::ApprovalRequest;
pub use check_admission::CheckAdmission;
pub use check_execution::CheckExecution;
pub use check_outcome::CheckOutcome;
mod check_options;
pub use check_options::CheckOptions;
mod check_budget;
pub use check_budget::CheckBudget;
mod timeout_origin;
pub use check_request::CheckRequest;
pub use timeout_origin::TimeoutOrigin;
mod reuse;
pub use approval_opening::ApprovalOpening;
pub use approval_resolution::ApprovalResolution;
pub use check_settlement::CheckSettlement;
pub use completion_evidence::CompletionEvidence;
pub use completion_refusal::CompletionRefusal;
pub use evidence_port::EvidencePort;
pub use gate_contract::GateContract;
pub use goal_set::GoalSet;
pub use observation_drift::ObservationDrift;
pub use outstanding_gate::OutstandingGate;
pub use plan_change::PlanChange;
pub use plan_snapshot::PlanSnapshot;
pub use recovered_failure::RecoveredFailure;
pub use run_error::RunError;
pub use run_report::RunReport;
pub use snapshot::RunSnapshot;
pub use source_freshness::SourceFreshness;
pub use source_observation::SourceObservation;
pub use source_witness::SourceWitness;
pub use status::RunStatus;
pub use step_admission::StepAdmission;
mod execution_binding;
pub use execution_binding::ExecutionBinding;

use std::collections::BTreeMap;
use std::sync::{Arc, Mutex, MutexGuard};

use super::engine::RunEngine;

/// The application boundary over the same native engine served to every
/// client. Carries the host's evidence capability, when one was composed,
/// and the witnesses of checks admitted in this process until they settle.
#[derive(Clone)]
pub struct RunUseCases {
    engine: Arc<Mutex<RunEngine>>,
    evidence: Option<Arc<dyn EvidencePort>>,
    witnesses: Arc<Mutex<BTreeMap<String, Arc<dyn SourceWitness>>>>,
}

impl RunUseCases {
    pub fn new(engine: Arc<Mutex<RunEngine>>) -> Self {
        Self {
            engine,
            evidence: None,
            witnesses: Arc::new(Mutex::new(BTreeMap::new())),
        }
    }

    /// Compose the host's evidence capability: from here on every admitted
    /// check observes the source through it and completion consults it.
    #[must_use]
    pub fn with_evidence(mut self, port: Arc<dyn EvidencePort>) -> Self {
        self.evidence = Some(port);
        self
    }

    /// The composed evidence capability, if any.
    #[must_use]
    pub fn evidence(&self) -> Option<&Arc<dyn EvidencePort>> {
        self.evidence.as_ref()
    }

    fn lock(&self) -> Result<MutexGuard<'_, RunEngine>, RunError> {
        let mut engine = self
            .engine
            .lock()
            .map_err(|_| "engine mutex poisoned".to_owned())?;
        engine.refresh_if_stale()?;
        Ok(engine)
    }
}
