use std::{ffi::OsString, time::Duration};

/// Owner-selected bounds; request data cannot widen these limits.
#[derive(Debug, Clone)]
pub struct WorkerOptions {
    pub max_pending: usize,
    pub ready_timeout: Duration,
    pub backoff: Vec<Duration>,
    pub max_strikes: u32,
    pub max_quarantine_entries: usize,
    /// Internal role arguments. Override only for another admitted native host.
    pub launch_args: Vec<OsString>,
}

impl Default for WorkerOptions {
    fn default() -> Self {
        Self {
            max_pending: 128,
            ready_timeout: Duration::from_secs(15),
            backoff: [0, 50, 100, 200, 400, 800]
                .map(Duration::from_millis)
                .to_vec(),
            max_strikes: 3,
            max_quarantine_entries: 1024,
            launch_args: vec!["--workspace-worker".into()],
        }
    }
}
