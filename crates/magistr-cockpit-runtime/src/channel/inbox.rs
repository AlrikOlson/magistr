//! One observation destination for every session producer and its optional tap.
use crate::channel::{Arrivals, ObservationSink};
use magistr_application::sessions::AgentEvent;
use magistr_cockpit::cockpit::{Cmd, Msg};
use std::sync::Arc;
use tokio::sync::mpsc::{self, UnboundedReceiver, UnboundedSender};

#[derive(Clone, Debug)]
enum Delivery {
    Unbounded(UnboundedSender<Msg>),
    Bounded(mpsc::Sender<Msg>),
    Observed(Arc<dyn ObservationSink>),
}

/// Producer port independent of the surface and its observation lifetime.
#[derive(Clone, Debug)]
pub struct Inbox {
    delivery: Delivery,
    tap: Option<UnboundedSender<Msg>>,
    stamp: Option<Arrivals>,
    conversation: Option<Arc<str>>,
}

impl Inbox {
    #[must_use]
    pub fn channel() -> (Self, UnboundedReceiver<Msg>) {
        let (tx, rx) = mpsc::unbounded_channel();
        (Self::new(Delivery::Unbounded(tx)), rx)
    }

    /// Bounded surface delivery. A full queue refuses the message without closing.
    #[must_use]
    pub fn bounded(capacity: usize) -> (Self, mpsc::Receiver<Msg>) {
        let (tx, rx) = mpsc::channel(capacity.max(1));
        (Self::new(Delivery::Bounded(tx)), rx)
    }

    /// Fold observations directly into their application-owned projection.
    #[must_use]
    pub fn observed(sink: Arc<dyn ObservationSink>) -> Self {
        Self::new(Delivery::Observed(sink))
    }

    fn new(delivery: Delivery) -> Self {
        Self {
            delivery,
            tap: None,
            stamp: None,
            conversation: None,
        }
    }

    #[must_use]
    pub fn tap(&self, observer: UnboundedSender<Msg>) -> Self {
        Self {
            delivery: self.delivery.clone(),
            tap: Some(observer),
            stamp: self.stamp.clone(),
            conversation: self.conversation.clone(),
        }
    }

    #[must_use]
    pub fn stamped(&self, arrivals: Arrivals) -> Self {
        Self {
            delivery: self.delivery.clone(),
            tap: self.tap.clone(),
            stamp: Some(arrivals),
            conversation: self.conversation.clone(),
        }
    }

    /// Bind this producer to one logical conversation for its entire lifetime.
    #[must_use]
    pub fn for_conversation(&self, id: &str) -> Self {
        Self {
            conversation: Some(Arc::from(id)),
            ..self.clone()
        }
    }

    /// Ask the observation owner to select an already bound conversation.
    pub fn activate(&self, id: &str) -> Result<(), String> {
        match &self.delivery {
            Delivery::Observed(sink) => sink.activate(id),
            Delivery::Unbounded(_) | Delivery::Bounded(_) => Ok(()),
        }
    }

    /// Validate the captured command target at its dispatch owner, including after awaits.
    pub fn command_for(&self, mut command: Cmd, active: Option<&str>) -> Option<Cmd> {
        while let Cmd::ForConversation { id, command: inner } = command {
            if active != Some(id.as_str()) {
                let inbox = self.for_conversation(&id);
                let error =
                    "This conversation is no longer selected. Select it and try again.".to_owned();
                match *inner {
                    Cmd::Prompt(input) | Cmd::RoadmapPrompt(input) => {
                        inbox.send(Msg::Agent(AgentEvent::PromptRejected { input, error }));
                    }
                    cmd => {
                        inbox.send(Msg::CommandFailed { cmd, error });
                    }
                }
                return None;
            }
            command = *inner;
        }
        Some(command)
    }

    /// Post once to the owner; false means the destination refused delivery.
    pub fn send(&self, msg: Msg) -> bool {
        // A bounded relay may return a command before the runtime unwraps it.
        // Its refusal belongs to the original conversation even after a switch.
        if let Msg::CommandFailed {
            cmd: Cmd::ForConversation { id, command },
            error,
        } = msg
        {
            return self.for_conversation(&id).send(Msg::CommandFailed {
                cmd: *command,
                error,
            });
        }
        if let Some(stamp) = &self.stamp {
            stamp.record(&msg);
        }
        if let Some(tap) = &self.tap {
            let _ = tap.send(msg.clone());
        }
        match &self.delivery {
            Delivery::Unbounded(tx) => tx.send(msg).is_ok(),
            Delivery::Bounded(tx) => tx.try_send(msg).is_ok(),
            Delivery::Observed(sink) => match &self.conversation {
                Some(id) => sink.send_for(id, msg),
                None => sink.send(msg),
            },
        }
    }

    #[must_use]
    pub fn is_closed(&self) -> bool {
        match &self.delivery {
            Delivery::Unbounded(tx) => tx.is_closed(),
            Delivery::Bounded(tx) => tx.is_closed(),
            Delivery::Observed(sink) => sink.is_closed(),
        }
    }

    pub async fn closed(&self) {
        match &self.delivery {
            Delivery::Unbounded(tx) => tx.closed().await,
            Delivery::Bounded(tx) => tx.closed().await,
            Delivery::Observed(sink) => sink.closed().await,
        }
    }
}
