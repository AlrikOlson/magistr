use super::watch::WatchMode;
use std::time::Duration;
/// Hard limits apply before scanning, reading or admitting worker inputs.
#[derive(Debug, Clone)]
pub struct IndexOptions {
    pub watch_mode: WatchMode,
    pub poll_interval: Duration,
    pub max_file_bytes: usize,
    pub max_files: usize,
    pub worker_timeout: Duration,
    pub network_override: Option<String>,
}
impl Default for IndexOptions {
    fn default() -> Self {
        Self {
            watch_mode: WatchMode::Auto,
            poll_interval: Duration::from_millis(200),
            max_file_bytes: 1024 * 1024,
            max_files: 100_000,
            worker_timeout: Duration::from_secs(10),
            network_override: None,
        }
    }
}
