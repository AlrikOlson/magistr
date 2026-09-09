use serde::{Deserialize, Serialize};

/// Where a unit of work is in its lifecycle.
///
/// There is no `Reclaimed` state, and that is the point: a job whose
/// node went unreachable — or whose claim lease ran out — goes back to
/// [`Queued`], because "waiting for a node" is exactly what it now is.
/// The fact that it has been tried before lives in [`Job::attempt`],
/// which is a count rather than a state, so a job can be reclaimed any
/// number of times without the state machine growing a branch.
///
/// [`Queued`]: JobState::Queued
/// [`Job::attempt`]: super::job::Job::attempt
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum JobState {
    /// Waiting for a node to claim it.
    Queued,
    /// Held by a node under a claim lease.
    Claimed,
    /// The node reported success.
    Completed,
    /// The node reported failure.
    Failed,
}

impl JobState {
    /// Whether the job has reached a state nothing moves it out of.
    #[must_use]
    pub fn is_terminal(self) -> bool {
        matches!(self, Self::Completed | Self::Failed)
    }

    /// The wire spelling, for status rows and protocol results.
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Queued => "queued",
            Self::Claimed => "claimed",
            Self::Completed => "completed",
            Self::Failed => "failed",
        }
    }
}

#[cfg(test)]
#[path = "job_state.tests.rs"]
mod tests;
