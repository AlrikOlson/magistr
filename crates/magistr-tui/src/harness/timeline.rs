//! Messages that arrive at chosen moments.

use std::time::Duration;

use tokio::task::JoinHandle;
use tokio::time::Instant;

use magistr_cockpit::channel::Inbox;
use magistr_cockpit::cockpit::Msg;

/// A script for a producer: [`Msg`]s due at times measured from when
/// [`Timeline::play`] starts. Steps are delivered in time order, ties in
/// insertion order; everything due is sent without yielding in between,
/// so a hundred steps a few microseconds apart land as one burst.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Timeline {
    steps: Vec<(Duration, Msg)>,
}

impl Timeline {
    /// Nothing scheduled.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Post `msg` at `at`.
    #[must_use]
    pub fn at(mut self, at: Duration, msg: Msg) -> Self {
        self.steps.push((at, msg));
        self
    }

    /// Number of steps.
    #[must_use]
    pub fn len(&self) -> usize {
        self.steps.len()
    }

    /// Whether nothing is scheduled.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.steps.is_empty()
    }

    /// The steps in delivery order.
    #[must_use]
    pub fn into_steps(mut self) -> Vec<(Duration, Msg)> {
        self.steps.sort_by_key(|(at, _)| *at);
        self.steps
    }

    /// Play the script onto `inbox` from now; the task ends after the last
    /// step, or as soon as the loop is gone.
    pub fn play(self, inbox: Inbox) -> JoinHandle<()> {
        tokio::spawn(play(self.into_steps(), inbox))
    }
}

async fn play(steps: Vec<(Duration, Msg)>, inbox: Inbox) {
    let start = Instant::now();
    for (at, msg) in steps {
        let due = start + at;
        if due > Instant::now() {
            tokio::time::sleep_until(due).await;
        }
        if !inbox.send(msg) {
            return;
        }
    }
}

#[cfg(test)]
#[path = "timeline.tests.rs"]
mod tests;
