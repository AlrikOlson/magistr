//! One acknowledged observation at a time bounds native IPC independently of WebView speed.
use std::sync::{Mutex, PoisonError};
use tokio::sync::Notify;

#[derive(Default)]
pub(crate) struct Credit {
    offered: Mutex<Option<u64>>,
    pub(crate) ready: Notify,
}
impl Credit {
    pub(crate) fn offer(&self, sequence: u64) -> bool {
        let mut offered = self.offered.lock().unwrap_or_else(PoisonError::into_inner);
        if offered.is_some() {
            return false;
        }
        *offered = Some(sequence);
        true
    }
    pub(crate) fn acknowledge(&self, sequence: u64) -> Result<(), String> {
        let mut offered = self.offered.lock().unwrap_or_else(PoisonError::into_inner);
        if *offered != Some(sequence) {
            return Err("This update is no longer waiting for a response.".into());
        }
        *offered = None;
        self.ready.notify_one();
        Ok(())
    }
}

#[cfg(test)]
#[path = "credit.tests.rs"]
mod tests;
