//! The guard behind [`Shutdown::bound`](super::shutdown::Shutdown::bound).

/// Keeps a bound token's bridge alive; dropping it ends the bridge, so a
/// wait that is over leaves no task behind.
pub struct ShutdownBound {
    task: tokio::task::JoinHandle<()>,
}

impl ShutdownBound {
    pub(super) fn new(task: tokio::task::JoinHandle<()>) -> Self {
        Self { task }
    }
}

impl Drop for ShutdownBound {
    fn drop(&mut self) {
        self.task.abort();
    }
}
