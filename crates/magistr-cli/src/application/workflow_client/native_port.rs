//! The engine port over the in-process engines.

use magistr_engine::NativeEngines;
use magistr_engine::Observed;
use magistr_engine::native::{Request, RequestPromotion, Task, WorkflowMutationError};

use magistr_cockpit::cockpit::ProposalKind;
use magistr_cockpit::engine::EnginePort;

/// The production [`EnginePort`]: the same admitted in-process
/// [`NativeEngines`] the entry point opened for the store snapshot,
/// acting on the shared store the way the MCP server's verbs do — a pin
/// is `decision_pin`, a review is `decision_review`. The store
/// merges on write, so the agent's `magistr mcp` on the same
/// project sees the mutation too.
pub struct NativeEnginePort {
    engines: NativeEngines,
}

impl NativeEnginePort {
    /// A port acting on `engines`.
    #[must_use]
    pub const fn new(engines: NativeEngines) -> Self {
        Self { engines }
    }
}

impl EnginePort for NativeEnginePort {
    fn pin(
        &mut self,
        note_number: u32,
        pinned: bool,
    ) -> Result<Observed<bool>, WorkflowMutationError> {
        self.engines
            .decision_pin(note_number, pinned)
            .map_err(Into::into)
    }

    fn review(&mut self) -> Result<serde_json::Value, String> {
        self.engines.decision_review()
    }

    fn start_task(&mut self, id: &str) -> Result<Observed<Task>, WorkflowMutationError> {
        self.engines.plan_start_task(id).map_err(Into::into)
    }

    fn complete_task(&mut self, id: &str) -> Result<Observed<Task>, WorkflowMutationError> {
        self.engines
            .plan_complete_task(id, None)
            .map_err(Into::into)
    }

    fn resolve_proposal(
        &mut self,
        id: &str,
        kind: ProposalKind,
        accept: bool,
    ) -> Result<Observed<Task>, WorkflowMutationError> {
        match kind {
            ProposalKind::Title => self.engines.plan_resolve_title_proposal(id, accept),
            ProposalKind::Status => self.engines.plan_resolve_status_proposal(id, accept),
            ProposalKind::Reprioritize => self.engines.plan_resolve_reprioritize(id, accept),
        }
        .map_err(Into::into)
    }

    fn clear_blocker(&mut self, id: &str) -> Result<Observed<Task>, WorkflowMutationError> {
        self.engines.plan_clear_waiting_on(id).map_err(Into::into)
    }

    fn promote_request(&mut self, id: &str) -> Result<RequestPromotion, WorkflowMutationError> {
        self.engines.request_promote(id).map_err(Into::into)
    }

    fn dismiss_request(&mut self, id: &str) -> Result<Observed<Request>, WorkflowMutationError> {
        self.engines.request_dismiss(id).map_err(Into::into)
    }

    fn snooze_request(
        &mut self,
        id: &str,
        minutes: u32,
    ) -> Result<Observed<Request>, WorkflowMutationError> {
        self.engines
            .request_snooze(id, i64::from(minutes))
            .map_err(Into::into)
    }
}

#[cfg(test)]
#[path = "native_port.tests.rs"]
mod tests;
