//! The wire key of the stamp a publisher that declared an owner epoch
//! puts on every broadcast frame. Read by the subscriber that decodes the
//! line; written by the broadcaster.

/// The wire key of the publisher's declared epoch.
pub use magistr_workflow_contracts::EPOCH_KEY;
