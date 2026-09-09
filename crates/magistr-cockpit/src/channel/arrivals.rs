//! When a message reached the runtime's inbox.

use std::sync::{Arc, Mutex};
use std::time::Instant;

use crate::agent::AgentEvent;
use crate::cockpit::Msg;

/// A shared log of the instant every message posted on a stamped
/// [`Inbox`](crate::channel::Inbox) reached it — the "event arrived" end
/// of an event-to-paint measurement, the way [`Deliveries`](super::Deliveries)
/// is the "input arrived" end. The stamp is taken inside `send`, ahead
/// of the loop's own channel, so it is at or before the instant the loop
/// can see the message — which is what lets a test say "the first write
/// after this stamp" and mean the very next frame.
#[derive(Clone, Debug, Default)]
pub struct Arrivals {
    log: Arc<Mutex<Vec<(Instant, Msg)>>>,
}

impl Arrivals {
    /// An empty log.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Record that `msg` arrived now.
    pub fn record(&self, msg: &Msg) {
        self.lock().push((Instant::now(), msg.clone()));
    }

    /// Every arrival so far, in order.
    #[must_use]
    pub fn instants(&self) -> Vec<Instant> {
        self.lock().iter().map(|(when, _)| *when).collect()
    }

    /// The agent's text deltas that arrived, each with its instant, in
    /// order — what a test samples to say every one of them was in the
    /// very next frame.
    #[must_use]
    pub fn texts(&self) -> Vec<(Instant, String)> {
        self.lock()
            .iter()
            .filter_map(|(when, msg)| match msg {
                Msg::Agent(AgentEvent::Text(text)) => Some((*when, text.clone())),
                _ => None,
            })
            .collect()
    }

    /// The instants at which a permission request arrived, in order.
    #[must_use]
    pub fn permission_requests(&self) -> Vec<Instant> {
        self.lock()
            .iter()
            .filter(|(_, msg)| matches!(msg, Msg::Agent(AgentEvent::PermissionRequest { .. })))
            .map(|(when, _)| *when)
            .collect()
    }

    /// Arrivals so far.
    #[must_use]
    pub fn len(&self) -> usize {
        self.lock().len()
    }

    /// Whether nothing arrived.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.lock().is_empty()
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, Vec<(Instant, Msg)>> {
        self.log
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }
}

#[cfg(test)]
#[path = "arrivals.tests.rs"]
mod tests;
