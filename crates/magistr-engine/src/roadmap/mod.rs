//! Roadmap management — domain service for spec progress tracking.
//!
//! `RoadmapManager` tracks roadmap progress and emits domain events when
//! tasks or phases change state. It is backend-agnostic: no YAML, no
//! filesystem, no markdown generation.

pub mod roadmap_event;
pub mod roadmap_manager;
pub mod roadmap_progress;

pub use roadmap_event::RoadmapEvent;
pub use roadmap_manager::RoadmapManager;
pub use roadmap_progress::RoadmapProgress;

#[cfg(test)]
#[path = "mod.tests.rs"]
mod tests;
