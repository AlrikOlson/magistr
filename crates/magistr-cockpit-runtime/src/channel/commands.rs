//! Commands received by the existing provider and work coordinators.
use magistr_cockpit::cockpit::Cmd;
use tokio::sync::mpsc::{self, UnboundedReceiver, UnboundedSender};

#[derive(Debug)]
enum Receiver {
    Unbounded(UnboundedReceiver<Cmd>),
    Bounded(mpsc::Receiver<Cmd>),
}

#[derive(Debug)]
pub struct Commands {
    rx: Receiver,
}

impl Commands {
    #[must_use]
    pub fn channel() -> (UnboundedSender<Cmd>, Self) {
        let (tx, rx) = mpsc::unbounded_channel();
        (
            tx,
            Self {
                rx: Receiver::Unbounded(rx),
            },
        )
    }

    /// Admission queue used by resident application sessions.
    #[must_use]
    pub fn bounded(capacity: usize) -> (mpsc::Sender<Cmd>, Self) {
        let (tx, rx) = mpsc::channel(capacity.max(1));
        (
            tx,
            Self {
                rx: Receiver::Bounded(rx),
            },
        )
    }

    pub async fn next(&mut self) -> Option<Cmd> {
        match &mut self.rx {
            Receiver::Unbounded(rx) => rx.recv().await,
            Receiver::Bounded(rx) => rx.recv().await,
        }
    }

    pub fn try_next(&mut self) -> Option<Cmd> {
        match &mut self.rx {
            Receiver::Unbounded(rx) => rx.try_recv().ok(),
            Receiver::Bounded(rx) => rx.try_recv().ok(),
        }
    }
}
