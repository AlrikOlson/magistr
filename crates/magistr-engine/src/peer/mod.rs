//! Immutable peer plans, exact dual approvals and bounded reconciliation.
//! Authentication and dispatch belong to the host; see docs/architecture/peers.md.

mod agreement_error;
mod authority_action;
mod authority_outcome;
mod instruction_lease;
mod journal_version;
mod outage_policy;
mod peer_briefing;
mod peer_evidence;
mod peer_identity;
mod peer_request;
mod peer_role;
mod plan_agreement;
mod plan_body;
mod plan_context;
mod plan_reference;
mod plan_revision;
mod plan_task;
mod receive_peer;
mod reconciliation;
mod reconciliation_action;
mod reconciliation_observation;
mod reconciliation_state;
mod reconciliation_stop;
mod run_contract;
mod run_limits;
mod shared_context;
mod task_authority;
mod worker_admission;
mod worker_model;
mod worker_permit;

pub use agreement_error::AgreementError;
pub use authority_action::AuthorityAction;
pub use authority_outcome::AuthorityOutcome;
pub use instruction_lease::InstructionLease;
pub use outage_policy::OutagePolicy;
pub use peer_briefing::PeerBriefing;
pub use peer_evidence::PeerEvidence;
pub use peer_identity::PeerIdentity;
pub use peer_request::PeerRequest;
pub use peer_role::Peer;
pub use plan_agreement::PlanAgreement;
pub use plan_body::PlanBody;
pub use plan_context::PlanContext;
pub use plan_reference::PlanReference;
pub use plan_revision::PlanRevision;
pub use plan_task::PlanTask;
pub use reconciliation::Reconciliation;
pub use reconciliation_action::ReconciliationAction;
pub use reconciliation_observation::ReconciliationObservation;
pub use reconciliation_state::ReconciliationState;
pub use reconciliation_stop::ReconciliationStop;
pub use run_contract::RunContract;
pub use run_limits::RunLimits;
pub use task_authority::TaskAuthority;
pub use worker_model::WorkerModel;
pub use worker_permit::WorkerPermit;

fn require_text(name: &str, value: &str) -> Result<(), AgreementError> {
    if value.trim().is_empty() {
        return Err(AgreementError::Invalid(format!("{name} must not be blank")));
    }
    Ok(())
}

fn require_list(name: &str, values: &[String]) -> Result<(), AgreementError> {
    if values.is_empty() {
        return Err(AgreementError::Invalid(format!("{name} must not be empty")));
    }
    for value in values {
        require_text(name, value)?;
    }
    Ok(())
}

#[cfg(test)]
#[path = "mod.tests.rs"]
mod tests;
