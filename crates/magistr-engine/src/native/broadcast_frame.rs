//! Compatibility export of the canonical workflow projection contract.
pub use magistr_workflow_contracts::NativeFrame;

#[cfg(test)]
#[path = "broadcast_frame.tests.rs"]
mod tests;
