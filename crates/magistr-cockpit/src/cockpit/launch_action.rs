//! What the launcher asks the entry point to start.

/// The launcher's exit ask: recorded on the model beside the quit, and
/// dispatched by the entry point after the terminal is handed back —
/// the cockpit itself spawns nothing.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum LaunchAction {
    /// One `magistr next` iteration.
    Next,
    /// The autonomous iteration loop (`magistr next-loop`).
    Loop(super::LoopScope),
    /// The quality gates (`magistr validate`).
    Gates,
    /// A recent session, continued: the agent's session id as the
    /// session database recorded it, for the entry point to hand the
    /// ACP transport as its `session/load`.
    Resume(String),
}
