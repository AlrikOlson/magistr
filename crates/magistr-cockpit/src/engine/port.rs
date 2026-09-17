//! What the cockpit asks of the native workflow engines.

use magistr_workflow_contracts::{Chunk, Signal};

use crate::cockpit::ProposalKind;

/// The engines the cockpit acts through without the agent: the Trace
/// tab's \[Pin\] and \[Checkpoint\], and the Roadmap tab's chunk,
/// proposal and signal actions. The in-process engines broadcast
/// nothing, so the consumer answers the cockpit itself with an
/// [`EngineEvent`](super::EngineEvent) built from the port's return —
/// mutations come back as the updated record, which
/// [`apply`](super::apply) folds into the typed event the engines would
/// have announced. Errors are words for the stage, not codes. `Send +
/// Sync` because the session task holds the port across its awaits.
pub trait EnginePort: Send + Sync {
    /// Pin or unpin the step.
    ///
    /// # Errors
    /// The engine's reason the step was not marked — an unknown step
    /// number included.
    fn pin(&mut self, step_number: u32, pinned: bool) -> Result<(), String>;

    /// The trace-wide metacognitive checkpoint report, as the engine
    /// renders it.
    ///
    /// # Errors
    /// The engine's reason no report was taken.
    fn checkpoint(&mut self) -> Result<serde_json::Value, String>;

    /// Mark the chunk in progress; the updated chunk comes back.
    ///
    /// # Errors
    /// The engine's reason — an unknown id or an illegal transition.
    fn start_chunk(&mut self, id: &str) -> Result<Chunk, String>;

    /// Mark the chunk done; the updated chunk comes back.
    ///
    /// # Errors
    /// The engine's reason — an unknown id or an illegal transition.
    fn complete_chunk(&mut self, id: &str) -> Result<Chunk, String>;

    /// Resolve the chunk's open proposal of `kind`: accept takes the
    /// suggestion, reject clears it. The updated chunk comes back.
    ///
    /// # Errors
    /// The engine's reason — no open proposal of that kind included.
    fn resolve_proposal(
        &mut self,
        id: &str,
        kind: ProposalKind,
        accept: bool,
    ) -> Result<Chunk, String>;

    /// Lift the chunk's blocker; the updated chunk comes back.
    ///
    /// # Errors
    /// The engine's reason — an unknown id, or a chunk with no blocker
    /// to clear, which the engines refuse loudly rather than answer as
    /// a no-op.
    fn clear_blocker(&mut self, id: &str) -> Result<Chunk, String>;

    /// Promote the signal to the roadmap; the updated signal comes back.
    ///
    /// # Errors
    /// The engine's reason — a signal not yet researched included.
    fn promote_signal(&mut self, id: &str) -> Result<Signal, String>;

    /// Dismiss the signal; the updated signal comes back.
    ///
    /// # Errors
    /// The engine's reason — an unknown id or a terminal signal.
    fn ignore_signal(&mut self, id: &str) -> Result<Signal, String>;

    /// Snooze the signal for `minutes`; the updated signal comes back.
    ///
    /// # Errors
    /// The engine's reason — an unknown id.
    fn snooze_signal(&mut self, id: &str, minutes: u32) -> Result<Signal, String>;
}
