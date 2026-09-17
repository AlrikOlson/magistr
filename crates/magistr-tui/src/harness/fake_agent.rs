//! An agent made of a script.

use std::time::Duration;

use tokio::task::JoinHandle;

use super::timeline::Timeline;
use magistr_application::sessions::{AgentEvent, ToolStatus, ToolVerb, TurnEnd};
use magistr_cockpit::cockpit::Msg;
use magistr_cockpit_runtime::channel::Inbox;

/// Scripted [`AgentEvent`]s at scripted times, posted as `Msg::Agent`
/// exactly as the live relay would post them.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct FakeAgent {
    timeline: Timeline,
}

impl FakeAgent {
    /// An agent that says nothing.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Any event at `at`.
    #[must_use]
    pub fn event(mut self, at: Duration, event: AgentEvent) -> Self {
        self.timeline = self.timeline.at(at, Msg::Agent(event));
        self
    }

    /// One text delta at `at`.
    #[must_use]
    pub fn text(self, at: Duration, text: &str) -> Self {
        self.event(at, AgentEvent::Text(text.to_owned()))
    }

    /// `count` text deltas of `text`, the first at `at` and each `gap`
    /// after the previous.
    #[must_use]
    pub fn deltas(mut self, at: Duration, count: u32, gap: Duration, text: &str) -> Self {
        for i in 0..count {
            self = self.text(at + gap * i, text);
        }
        self
    }

    /// A thought delta at `at`.
    #[must_use]
    pub fn thought(self, at: Duration, text: &str) -> Self {
        self.event(at, AgentEvent::Thought(text.to_owned()))
    }

    /// A tool call announced in progress at `at`, a plain `tool` verb.
    #[must_use]
    pub fn tool_call(self, at: Duration, id: &str, title: &str) -> Self {
        self.event(
            at,
            AgentEvent::ToolCall {
                id: id.to_owned(),
                title: title.to_owned(),
                verb: ToolVerb::default(),
                status: ToolStatus::InProgress,
                diff: None,
                images: Vec::new(),
                record: Default::default(),
            },
        )
    }

    /// The turn ends normally at `at`.
    #[must_use]
    pub fn turn_ended(self, at: Duration) -> Self {
        self.event(at, AgentEvent::TurnEnded(TurnEnd::EndTurn))
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

    /// Start talking onto `inbox`.
    pub fn play(self, inbox: Inbox) -> JoinHandle<()> {
        self.timeline.play(inbox)
    }
}

#[cfg(test)]
#[path = "fake_agent.tests.rs"]
mod tests;
