//! The engine port over the in-process engines.

use magistr_engine::NativeEngines;

use super::port::EnginePort;
use crate::cockpit::ProposalKind;

/// The production [`EnginePort`]: the same admitted in-process
/// [`NativeEngines`] the entry point opened for the store seed, acting
/// on the shared store the way the MCP server's verbs do — a pin is
/// `think_pin_step`, a checkpoint is `think_checkpoint`. The store
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

    fn start_chunk(&mut self, id: &str) -> Result<serde_json::Value, String> {
        self.engines.roadmap_start_chunk(id).map(chunk_value)
    }

    fn complete_chunk(&mut self, id: &str) -> Result<serde_json::Value, String> {
        self.engines
            .roadmap_complete_chunk(id, None)
            .map(chunk_value)
    }

    fn resolve_proposal(
        &mut self,
        id: &str,
        kind: ProposalKind,
        accept: bool,
    ) -> Result<serde_json::Value, String> {
        match kind {
            ProposalKind::Title => self.engines.roadmap_resolve_title_proposal(id, accept),
            ProposalKind::Status => self.engines.roadmap_resolve_status_proposal(id, accept),
            ProposalKind::Reprioritize => self.engines.roadmap_resolve_reprioritize(id, accept),
        }
        .map(chunk_value)
    }

    fn clear_blocker(&mut self, id: &str) -> Result<serde_json::Value, String> {
        self.engines.roadmap_clear_blocked_by(id).map(chunk_value)
    }

    fn promote_signal(&mut self, id: &str) -> Result<serde_json::Value, String> {
        self.engines.signal_promote(id).map(signal_value)
    }

    fn ignore_signal(&mut self, id: &str) -> Result<serde_json::Value, String> {
        self.engines.signal_ignore(id).map(signal_value)
    }

    fn snooze_signal(&mut self, id: &str, minutes: u32) -> Result<serde_json::Value, String> {
        self.engines
            .signal_snooze(id, i64::from(minutes))
            .map(signal_value)
    }
}

/// A chunk as the wire would carry it; serialization cannot fail.
pub(super) fn chunk_value(chunk: magistr_engine::native::Chunk) -> serde_json::Value {
    serde_json::to_value(chunk).unwrap_or(serde_json::Value::Null)
}

/// A signal as the wire would carry it; serialization cannot fail.
pub(super) fn signal_value(signal: magistr_engine::native::Signal) -> serde_json::Value {
    serde_json::to_value(signal).unwrap_or(serde_json::Value::Null)
}

#[cfg(test)]
#[path = "native_port.tests.rs"]
mod tests;
