use serde::{Deserialize, Serialize};

/// Where one project stands inside a batch run.
///
/// Only two of these are facts the batch keeps: [`Waiting`] (nothing has
/// been released for the project yet) and [`Skipped`] (an upstream
/// failure cancelled it). The other four are read straight off the
/// project's job in the [`WorkQueue`] — which is why a job reclaimed
/// from a dead node shows up here as [`Queued`] again with nothing to
/// update, and why there is no second ledger for the batch and the queue
/// to disagree through.
///
/// [`Waiting`]: BatchState::Waiting
/// [`Skipped`]: BatchState::Skipped
/// [`Queued`]: BatchState::Queued
/// [`WorkQueue`]: crate::fleet::WorkQueue
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BatchState {
    /// Held back: a dependency has not succeeded, or a project sharing
    /// its working tree is still in flight.
    Waiting,
    /// Released to the queue, waiting for a node.
    Queued,
    /// A node is running it.
    Running,
    /// The node reported success.
    Succeeded,
    /// The node reported failure.
    Failed,
    /// Cancelled without being run, because something it depends on
    /// failed.
    Skipped,
}

impl BatchState {
    /// Whether the project has reached a state the batch will not move
    /// it out of.
    #[must_use]
    pub fn is_terminal(self) -> bool {
        matches!(self, Self::Succeeded | Self::Failed | Self::Skipped)
    }

    /// Whether the project is holding its working tree right now.
    ///
    /// This is what serializes a monorepo: a project is released only
    /// when no sibling sharing its tree answers true here.
    #[must_use]
    pub fn is_in_flight(self) -> bool {
        matches!(self, Self::Queued | Self::Running)
    }

    /// The wire spelling, for status rows and protocol results.
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Waiting => "waiting",
            Self::Queued => "queued",
            Self::Running => "running",
            Self::Succeeded => "succeeded",
            Self::Failed => "failed",
            Self::Skipped => "skipped",
        }
    }
}

#[cfg(test)]
#[path = "batch_state.tests.rs"]
mod tests;
