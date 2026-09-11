//! Completion follows the handler, including rmcp's detached request tasks.

use tokio_util::sync::CancellationToken;

/// Shared by clones of one connection handler. Observers hold only the token,
/// so they do not extend this guard's lifetime. Its final drop acknowledges
/// that no request task can still reach that handler's workspace service.
#[derive(Debug, Default)]
pub(super) struct SessionLifetime {
    complete: CancellationToken,
}

impl SessionLifetime {
    pub(super) fn completion(&self) -> CancellationToken {
        self.complete.clone()
    }
}

impl Drop for SessionLifetime {
    fn drop(&mut self) {
        self.complete.cancel();
    }
}
