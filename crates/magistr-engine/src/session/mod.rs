//! Engine session — the core domain aggregate for agent session lifecycle.
//!
//! `EngineSession` owns the session state machine. It is backend-agnostic:
//! no ACP, no process spawning, no IO.

pub mod checkpoint_reason;
pub mod engine_session;
pub mod repo_state;
pub mod restored_session;
pub mod session_command;
pub mod session_cost;
pub mod session_event;
pub mod session_residency;
pub mod session_snapshot;
pub mod session_status;

pub use checkpoint_reason::CheckpointReason;
pub use engine_session::EngineSession;
pub use repo_state::RepoState;
pub use restored_session::RestoredSession;
pub use session_command::SessionCommand;
pub use session_cost::SessionCost;
pub use session_event::SessionEvent;
pub use session_residency::SessionResidency;
pub use session_snapshot::SessionSnapshot;
pub use session_status::SessionStatus;

#[cfg(test)]
#[path = "mod.tests.rs"]
mod tests;
