/// A coherent snapshot acquired between requests; pending() remains nonblocking.
#[derive(Debug, Clone)]
pub struct WorkerStatus {
    pub pid: Option<u32>,
    pub restarts: u64,
    pub consecutive_failures: u32,
    pub quarantined: usize,
    pub stopped: bool,
    pub pending: usize,
}
