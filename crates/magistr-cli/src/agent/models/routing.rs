//! Smart model routing — re-exports from split submodules.

pub use super::complexity_estimate::ComplexityEstimate;
pub use super::estimate_complexity::estimate_complexity;
pub use super::route::route;
pub use super::routing_decision::RoutingDecision;
pub use super::routing_log::RoutingLog;
pub use super::task_complexity::TaskComplexity;

#[cfg(test)]
#[path = "routing.tests.rs"]
mod tests;
