//! A ledger verb, run off the caller's task.

use std::sync::{Arc, Mutex, PoisonError};

use tokio::task::JoinHandle;

use magistr_cockpit::checkpoint::{CheckpointPort, apply};
use magistr_cockpit::cockpit::{CheckpointCmd, Msg};
use magistr_cockpit_runtime::channel::Inbox;

/// Carry `cmd` out on `port` on a blocking thread and post the answer
/// on `inbox` as [`Msg::Checkpoint`] — the one way a ledger verb runs.
///
/// A checkpoint reads the repository, resumes the engine or opens a
/// worktree: subprocesses and disk, never on the task that called for
/// it. The session task is the agent's relay, and one inline `git`
/// read stopped the stream for its duration; the loop driver is what
/// survives between iterations, and only its own task may wait — on
/// the handle this returns, at an iteration boundary, where nothing
/// else is going on. The mutex is because a spine grows (`&mut self`)
/// while the port is shared with the thread that runs it; there is one
/// holder, so it is never contended.
pub fn detach(
    port: &Arc<Mutex<Box<dyn CheckpointPort>>>,
    cmd: CheckpointCmd,
    inbox: &Inbox,
) -> JoinHandle<()> {
    let port = Arc::clone(port);
    let inbox = inbox.clone();
    tokio::task::spawn_blocking(move || {
        let mut port = port.lock().unwrap_or_else(PoisonError::into_inner);
        let event = apply(port.as_mut(), &cmd);
        drop(port);
        inbox.send(Msg::Checkpoint(event));
    })
}
