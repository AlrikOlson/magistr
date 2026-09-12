//! The engine port over the in-process engines.

use magistr_engine::NativeEngines;
use magistr_engine::native::{Chunk, Signal};

use super::port::EnginePort;
use crate::cockpit::ProposalKind;

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
    fn pin(&mut self, step_number: u32, pinned: bool) -> Result<(), String> {
        self.engines.think_pin_step(step_number, pinned).map(|_| ())
    }

    fn checkpoint(&mut self) -> Result<serde_json::Value, String> {
        self.engines.think_checkpoint()
    }

    fn start_chunk(&mut self, id: &str) -> Result<Chunk, String> {
        self.engines.roadmap_start_chunk(id)
    }

    fn complete_chunk(&mut self, id: &str) -> Result<Chunk, String> {
        self.engines.roadmap_complete_chunk(id, None)
    }

    fn resolve_proposal(
        &mut self,
        id: &str,
        kind: ProposalKind,
        accept: bool,
    ) -> Result<Chunk, String> {
        match kind {
            ProposalKind::Title => self.engines.roadmap_resolve_title_proposal(id, accept),
            ProposalKind::Status => self.engines.roadmap_resolve_status_proposal(id, accept),
            ProposalKind::Reprioritize => self.engines.roadmap_resolve_reprioritize(id, accept),
        }
    }

    fn clear_blocker(&mut self, id: &str) -> Result<Chunk, String> {
        self.engines.roadmap_clear_blocked_by(id)
    }

    fn promote_signal(&mut self, id: &str) -> Result<Signal, String> {
        self.engines.signal_promote(id)
    }

    fn ignore_signal(&mut self, id: &str) -> Result<Signal, String> {
        self.engines.signal_ignore(id)
    }

    fn snooze_signal(&mut self, id: &str, minutes: u32) -> Result<Signal, String> {
        self.engines.signal_snooze(id, i64::from(minutes))
    }
}

#[cfg(test)]
#[path = "native_port.tests.rs"]
mod tests;
