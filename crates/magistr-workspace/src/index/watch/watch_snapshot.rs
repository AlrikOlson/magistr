use serde::Serialize;

use super::WatchMode;

/// Copy of bounded watcher diagnostics; `pending` counts one coalesced scan.
#[derive(Debug, Clone, Serialize)]
pub struct WatchSnapshot {
    pub requested_mode: WatchMode,
    pub mode: WatchMode,
    pub epoch: u64,
    pub events: u64,
    pub rescans: u64,
    pub failures: u64,
    pub pending: bool,
    pub lag_ms: u64,
    /// Active adapter error; clears on recovery. `failures` retains history.
    pub last_error: Option<String>,
    pub stopped: bool,
}
