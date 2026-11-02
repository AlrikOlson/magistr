//! What the cockpit asks of the native workflow engines.

use magistr_workflow_contracts::signal::SignalPromotion;
use magistr_workflow_contracts::{Observed, Signal, Task, WorkflowMutationError};

use crate::cockpit::ProposalKind;

/// The engines the cockpit acts through without the agent: the decision
/// log's \[Pin\] and \[Review\], and the Plan tab's task,
/// proposal and signal actions. The in-process engines broadcast
/// nothing, so the consumer answers the cockpit itself with an
/// [`EngineEvent`](super::EngineEvent) built from the port's return —
/// mutations come back as the updated record, which
/// [`apply`](super::apply) folds into the typed event the engines would
/// have announced. Mutation outcomes retain source and effect until presentation. `Send +
/// Sync` because the session task holds the port across its awaits.
pub trait EnginePort: Send + Sync {
    /// Pin or unpin the note.
    ///
    /// # Errors
    /// The engine's reason the note was not marked — an unknown note
    /// number included.
    fn pin(
        &mut self,
        note_number: u32,
        pinned: bool,
    ) -> Result<Observed<bool>, WorkflowMutationError>;

    /// The log-wide metacognitive review report, as the engine
    /// renders it.
    ///
    /// # Errors
    /// The engine's reason no report was taken.
    fn review(&mut self) -> Result<serde_json::Value, String>;

    /// Mark the task in progress; the updated task comes back.
    ///
    /// # Errors
    /// The engine's reason — an unknown id or an illegal transition.
    fn start_task(&mut self, id: &str) -> Result<Observed<Task>, WorkflowMutationError>;

    /// Mark the task done; the updated task comes back.
    ///
    /// # Errors
    /// The engine's reason — an unknown id or an illegal transition.
    fn complete_task(&mut self, id: &str) -> Result<Observed<Task>, WorkflowMutationError>;

    /// Resolve the task's open proposal of `kind`: accept takes the
    /// suggestion, reject clears it. The updated task comes back.
    ///
    /// # Errors
    /// The engine's reason — no open proposal of that kind included.
    fn resolve_proposal(
        &mut self,
        id: &str,
        kind: ProposalKind,
        accept: bool,
    ) -> Result<Observed<Task>, WorkflowMutationError>;

    /// Lift the task's blocker; the updated task comes back.
    ///
    /// # Errors
    /// The engine's reason — an unknown id, or a task with no blocker
    /// to clear, which the engines refuse loudly rather than answer as
    /// a no-op.
    fn clear_blocker(&mut self, id: &str) -> Result<Observed<Task>, WorkflowMutationError>;

    /// Promote the signal to the plan; the updated signal comes back.
    ///
    /// # Errors
    /// The engine's reason — a signal not yet researched included.
    fn promote_signal(&mut self, id: &str) -> Result<SignalPromotion, WorkflowMutationError>;

    /// Dismiss the signal; the updated signal comes back.
    ///
    /// # Errors
    /// The engine's reason — an unknown id or a terminal signal.
    fn ignore_signal(&mut self, id: &str) -> Result<Observed<Signal>, WorkflowMutationError>;

    /// Snooze the signal for `minutes`; the updated signal comes back.
    ///
    /// # Errors
    /// The engine's reason — an unknown id.
    fn snooze_signal(
        &mut self,
        id: &str,
        minutes: u32,
    ) -> Result<Observed<Signal>, WorkflowMutationError>;
}
