//! Share the original session's checkpoint history across agent purposes.

use magistr_cockpit::checkpoint::CheckpointPort;
use magistr_engine::CheckpointReason;
use std::sync::{Arc, Mutex};

pub(crate) struct SharedCheckpoint(pub(crate) Arc<Mutex<Box<dyn CheckpointPort>>>);

impl CheckpointPort for SharedCheckpoint {
    fn save(
        &mut self,
        id: &str,
        reason: CheckpointReason,
        label: &str,
    ) -> Result<(Option<String>, bool), String> {
        self.0
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .save(id, reason, label)
    }
    fn restore(&mut self, id: &str) -> Result<(u32, Vec<String>), String> {
        self.0
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .restore(id)
    }
    fn fork(&mut self, id: &str, branch: Option<&str>) -> Result<(String, String), String> {
        self.0
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .fork(id, branch)
    }
    fn merge(&mut self, id: &str) -> Result<(String, String), String> {
        self.0
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .merge(id)
    }
}
