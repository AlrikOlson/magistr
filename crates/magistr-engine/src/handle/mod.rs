//! Engine handle — the primary entry point for the magistr engine.
//!
//! `EngineHandle` composes all domain services (`EngineSession`,
//! `WorkflowEngine`, `CostGovernor`, `RoadmapManager`) behind a typed
//! command/query API with event broadcasting.
//!
//! Thread-safe (`Clone + Send + Sync`) via `Arc<Mutex<...>>`.
//! Event subscription via `tokio::sync::broadcast`.

pub mod engine_event;
pub mod engine_handle;
pub mod workflow_event;

pub use engine_event::EngineEvent;
pub use engine_handle::EngineHandle;
pub use workflow_event::WorkflowEvent;

#[cfg(test)]
#[path = "mod.tests.rs"]
mod tests;
