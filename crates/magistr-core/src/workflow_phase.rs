//! Compatibility path for the canonical workflow phase.

pub use magistr_domain::workflow::WorkflowPhase;

#[cfg(test)]
#[path = "workflow_phase.tests.rs"]
mod tests;
