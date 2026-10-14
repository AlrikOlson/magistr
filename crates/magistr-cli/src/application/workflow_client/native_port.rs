//! The engine port over the in-process engines.

use magistr_engine::NativeEngines;
use magistr_engine::Observed;
use magistr_engine::native::{Chunk, Signal, SignalPromotion, WorkflowMutationError};

use magistr_cockpit::cockpit::ProposalKind;
use magistr_cockpit::engine::EnginePort;

/// The production [`EnginePort`]: the same admitted in-process
/// [`NativeEngines`] the entry point opened for the store snapshot,
/// acting on the shared store the way the MCP server's verbs do — a pin
/// is `think_pin_step`, a checkpoint is `think_checkpoint`. The store
/// merges on write, so the agent's `magistr mcp-serve` on the same
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
        step_number: u32,
        pinned: bool,
    ) -> Result<Observed<bool>, WorkflowMutationError> {
        self.engines
            .think_pin_step(step_number, pinned)
            .map_err(Into::into)
    }

    fn checkpoint(&mut self) -> Result<serde_json::Value, String> {
        self.engines.think_checkpoint()
    }

    fn start_chunk(&mut self, id: &str) -> Result<Observed<Chunk>, WorkflowMutationError> {
        self.engines.roadmap_start_chunk(id).map_err(Into::into)
    }

    fn complete_chunk(&mut self, id: &str) -> Result<Observed<Chunk>, WorkflowMutationError> {
        self.engines
            .roadmap_complete_chunk(id, None)
            .map_err(Into::into)
    }

    fn resolve_proposal(
        &mut self,
        id: &str,
        kind: ProposalKind,
        accept: bool,
    ) -> Result<Observed<Chunk>, WorkflowMutationError> {
        match kind {
            ProposalKind::Title => self.engines.roadmap_resolve_title_proposal(id, accept),
            ProposalKind::Status => self.engines.roadmap_resolve_status_proposal(id, accept),
            ProposalKind::Reprioritize => self.engines.roadmap_resolve_reprioritize(id, accept),
        }
        .map_err(Into::into)
    }

    fn clear_blocker(&mut self, id: &str) -> Result<Observed<Chunk>, WorkflowMutationError> {
        self.engines
            .roadmap_clear_blocked_by(id)
            .map_err(Into::into)
    }

    fn promote_signal(&mut self, id: &str) -> Result<SignalPromotion, WorkflowMutationError> {
        self.engines.signal_promote(id).map_err(Into::into)
    }

    fn ignore_signal(&mut self, id: &str) -> Result<Observed<Signal>, WorkflowMutationError> {
        self.engines.signal_ignore(id).map_err(Into::into)
    }

    fn snooze_signal(
        &mut self,
        id: &str,
        minutes: u32,
    ) -> Result<Observed<Signal>, WorkflowMutationError> {
        self.engines
            .signal_snooze(id, i64::from(minutes))
            .map_err(Into::into)
    }
}

#[cfg(test)]
#[path = "native_port.tests.rs"]
mod tests;
