//! The process-wide shutdown request every owner observes.
//!
//! A plain `SIGINT` does not end the process by itself any more: the
//! signal handler *requests* shutdown here, and each owner winds down on
//! its own terms — the resident session owner stops admitting turns,
//! cancels the active one, waits for the provider's terminal response and
//! closes its transport; every owned subprocess is ended gracefully and
//! settled; the loop driver admits no further attempt; the command then
//! finishes its own settlement (the cost ledger, its receipts, its
//! checkpoint, the workspace's release) and the process exits
//! [`Shutdown::EXIT_CODE`]. A second `SIGINT`, or [`Shutdown::SETTLE_DEADLINE`]
//! passing first, is the distinct emergency exit: the terminal restored,
//! the registered process groups killed, the same exit code and a line on
//! stderr saying which way out was taken.

use std::sync::OnceLock;
use std::time::Duration;

use tokio_util::sync::CancellationToken;

pub use super::shutdown_bound::ShutdownBound;

static REQUEST: OnceLock<CancellationToken> = OnceLock::new();

/// The one shutdown request of this process.
pub struct Shutdown;

impl Shutdown {
    /// The exit code of an interrupted run, graceful or emergency:
    /// `128 + SIGINT`, so a shell reads it as the interrupt it was.
    pub const EXIT_CODE: i32 = 130;

    /// How long the graceful path gets before the emergency exit takes
    /// over: above a gate's `SIGTERM` grace and a process group's
    /// settlement deadline, so a cooperating child is joined rather than
    /// abandoned.
    pub const SETTLE_DEADLINE: Duration = Duration::from_secs(8);

    fn root() -> &'static CancellationToken {
        REQUEST.get_or_init(CancellationToken::new)
    }

    /// Request shutdown; `true` the first time, `false` for a repeat.
    pub fn request() -> bool {
        let root = Self::root();
        if root.is_cancelled() {
            return false;
        }
        root.cancel();
        true
    }

    /// Whether shutdown has been requested.
    #[must_use]
    pub fn is_requested() -> bool {
        Self::root().is_cancelled()
    }

    /// A token that fires with the request, for an owner to select on.
    #[must_use]
    pub fn token() -> CancellationToken {
        Self::root().child_token()
    }

    /// A token that fires when `cancel` fires *or* shutdown is requested,
    /// so one wait covers a caller's own cancellation and the process's.
    /// The guard keeps the bridge alive; drop it once the wait is over.
    #[must_use]
    pub fn bound(cancel: &CancellationToken) -> (CancellationToken, ShutdownBound) {
        let either = cancel.child_token();
        let bridge = either.clone();
        let task = tokio::spawn(async move {
            Self::root().cancelled().await;
            bridge.cancel();
        });
        (either, ShutdownBound::new(task))
    }

    /// Exit with [`Shutdown::EXIT_CODE`] when shutdown was requested, once a
    /// command's own settlement is done — the one place an interrupted
    /// run's exit code is decided, so a command that exits itself and one
    /// that returns to `main` read the same.
    pub fn exit_if_requested() {
        if Self::is_requested() {
            std::process::exit(Self::EXIT_CODE);
        }
    }
}

#[cfg(test)]
#[path = "shutdown.tests.rs"]
mod tests;
