//! A think-and-ship feed made of a script.

use std::time::Duration;

use magistr_engine::{FeedStatus, NativeFrame};
use tokio::task::JoinHandle;

use super::timeline::Timeline;
use magistr_cockpit::channel::Inbox;
use magistr_cockpit::cockpit::Msg;
use magistr_cockpit::engine::EngineEvent;

/// Scripted feed events at scripted times, posted as `Msg::Engine`
/// exactly as the live feed would post them.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct FakeEngines {
    timeline: Timeline,
}

impl FakeEngines {
    /// A feed that reports nothing.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Any event at `at`.
    #[must_use]
    pub fn event(mut self, at: Duration, event: EngineEvent) -> Self {
        self.timeline = self.timeline.at(at, Msg::Engine(event));
        self
    }

    /// The feed's attachment becomes `status` at `at`.
    #[must_use]
    pub fn feed(self, at: Duration, status: FeedStatus) -> Self {
        self.event(at, EngineEvent::Feed(status))
    }

    /// The feed attaches for the first time at `at`.
    #[must_use]
    pub fn connected(self, at: Duration) -> Self {
        self.feed(at, FeedStatus::Connected { reconnect: false })
    }

    /// A frame from `family` of kind `kind` carrying `payload` at `at`.
    #[must_use]
    pub fn frame(self, at: Duration, family: &str, kind: &str, payload: serde_json::Value) -> Self {
        self.event(
            at,
            EngineEvent::Frame(NativeFrame {
                family: family.to_owned(),
                kind: kind.to_owned(),
                payload,
            }),
        )
    }

    /// The events scheduled so far.
    #[must_use]
    pub fn len(&self) -> usize {
        self.timeline.len()
    }

    /// Whether nothing is scheduled.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.timeline.is_empty()
    }

    /// The script as a timeline.
    #[must_use]
    pub fn into_timeline(self) -> Timeline {
        self.timeline
    }

    /// Start reporting onto `inbox`.
    pub fn play(self, inbox: Inbox) -> JoinHandle<()> {
        self.timeline.play(inbox)
    }
}

#[cfg(test)]
#[path = "fake_engines.tests.rs"]
mod tests;
