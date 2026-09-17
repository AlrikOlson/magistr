//! The channel every producer posts on.

use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender, unbounded_channel};

use crate::channel::Arrivals;
use magistr_cockpit::cockpit::Msg;

/// A sender into the runtime's event loop, cloned out to every producer
/// — jobs, the engine feed, the agent relay, the harness's fakes. What is
/// sent here is folded into the model with the same coalescing as the
/// terminal's own events: a burst costs one paint.
///
/// A tapped inbox ([`Inbox::tap`]) also hands a copy of everything sent
/// through it to an observer — how a driver outside the loop (the
/// iteration loop) sees exactly what the cockpit sees from a producer,
/// without a second event path. A stamped inbox ([`Inbox::stamped`])
/// notes the instant of every send synchronously, ahead of both
/// channels — how a test pins "the first frame after this event".
#[derive(Clone, Debug)]
pub struct Inbox {
    tx: UnboundedSender<Msg>,
    tap: Option<UnboundedSender<Msg>>,
    stamp: Option<Arrivals>,
}

impl Inbox {
    /// A fresh inbox and the receiver the loop drains.
    #[must_use]
    pub fn channel() -> (Self, UnboundedReceiver<Msg>) {
        let (tx, rx) = unbounded_channel();
        (
            Self {
                tx,
                tap: None,
                stamp: None,
            },
            rx,
        )
    }

    /// A clone of this inbox whose sends are also delivered to
    /// `observer`, replacing any earlier tap. An observer that went away
    /// costs nothing; only the loop's own side decides
    /// [`Inbox::send`]'s answer.
    #[must_use]
    pub fn tap(&self, observer: UnboundedSender<Msg>) -> Self {
        Self {
            tx: self.tx.clone(),
            tap: Some(observer),
            stamp: self.stamp.clone(),
        }
    }

    /// A clone of this inbox that records the instant of every send into
    /// `arrivals`, replacing any earlier stamp. The record is taken
    /// before the message goes to the tap or the loop, so a stamp is
    /// never later than the loop's first chance to see the message.
    #[must_use]
    pub fn stamped(&self, arrivals: Arrivals) -> Self {
        Self {
            tx: self.tx.clone(),
            tap: self.tap.clone(),
            stamp: Some(arrivals),
        }
    }

    /// Post `msg`; `false` once the loop is gone.
    pub fn send(&self, msg: Msg) -> bool {
        if let Some(stamp) = &self.stamp {
            stamp.record(&msg);
        }
        if let Some(tap) = &self.tap {
            let _ = tap.send(msg.clone());
        }
        self.tx.send(msg).is_ok()
    }

    /// Whether the loop is gone.
    #[must_use]
    pub fn is_closed(&self) -> bool {
        self.tx.is_closed()
    }

    /// Resolves once the loop is gone.
    pub async fn closed(&self) {
        self.tx.closed().await;
    }
}
