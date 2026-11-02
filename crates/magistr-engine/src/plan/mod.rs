//! Plan management — domain service for spec progress tracking.
//!
//! `PlanManager` tracks plan progress and emits domain events when
//! tasks or phases change state. It is backend-agnostic: no YAML, no
//! filesystem, no markdown generation.

pub mod plan_event;
pub mod plan_manager;
pub mod plan_progress;

pub use plan_event::PlanEvent;
pub use plan_manager::PlanManager;
pub use plan_progress::PlanProgress;

#[cfg(test)]
#[path = "mod.tests.rs"]
mod tests;
