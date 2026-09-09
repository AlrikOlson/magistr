//! The core's commands on their way to the session.

use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender, unbounded_channel};

use crate::cockpit::Cmd;

/// The receiver of every command `update` answers with except
/// [`Cmd::Quit`] — prompts, cancels, permission answers, loop decisions
/// — for whoever took it to act on. [`Runtime::commands`] hands it out
/// once, before the run; it ends when the loop is gone.
///
/// A driver that sits between the runtime and a session (the iteration
/// loop, which runs one session per iteration) makes its own pair with
/// [`Commands::channel`], hands the receiver to the session and forwards
/// what the runtime's receiver yields onto the sender.
///
/// [`Runtime::commands`]: super::Runtime::commands
#[derive(Debug)]
pub struct Commands {
    rx: UnboundedReceiver<Cmd>,
}

impl Commands {
    /// A sender and the receiver fed by it; the receiver ends when every
    /// sender is dropped.
    #[must_use]
    pub fn channel() -> (UnboundedSender<Cmd>, Self) {
        let (tx, rx) = unbounded_channel();
        (tx, Self { rx })
    }

    /// The next command; `None` once the loop is gone and the queue is
    /// drained.
    pub async fn next(&mut self) -> Option<Cmd> {
        self.rx.recv().await
    }

    /// The next command already queued, without waiting; `None` when
    /// nothing is queued *or* the loop is gone — a synchronous consumer
    /// (the native window's tests) cannot tell the two apart here and
    /// does not need to.
    pub fn try_next(&mut self) -> Option<Cmd> {
        self.rx.try_recv().ok()
    }
}
