//! Why the loop stopped.

use magistr_engine::workflow::StopReason;
use std::fmt;

/// How the iteration loop ended.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum LoopEnd {
    /// The roadmap has no ready chunk: everything left is done, blocked
    /// or backlog.
    NoReadyChunk,
    /// Every spec item is checked (a yaml project).
    SpecComplete,
    /// The iteration budget is spent.
    MaxIterations(u32),
    /// Too many iterations in a row produced no commit.
    NoCommits,
    /// The user aborted it.
    Aborted,
    /// Something failed that the loop could not go on from.
    Failed(String),
    /// The cockpit closed while the loop ran.
    CockpitClosed,
    /// The judge said the chunk's acceptance cannot be met as written:
    /// the chunk is blocked on a human with this reason.
    NeedsHuman(String),
    /// Recovery could not establish what happened to these attempts; the
    /// run is not stopped, only not admitting another attempt until they
    /// are reconciled or an explicit new run begins.
    Unreconciled(String),
}

impl LoopEnd {
    pub(super) fn from_stop(reason: StopReason, max: u32) -> Self {
        match reason {
            StopReason::NoProgress => Self::NoCommits,
            StopReason::IterationLimit | StopReason::OneChunkComplete => Self::MaxIterations(max),
            StopReason::SpecComplete => Self::SpecComplete,
            StopReason::NoReadyChunk => Self::NoReadyChunk,
            StopReason::Aborted => Self::Aborted,
            StopReason::Cancelled => Self::CockpitClosed,
            other => Self::Failed(other.to_string()),
        }
    }

    /// The terminal reason the run latches; `None` when the loop ended
    /// without the run reaching a terminal outcome.
    pub(super) fn stop_reason(&self) -> Option<StopReason> {
        Some(match self {
            Self::NoReadyChunk => StopReason::NoReadyChunk,
            Self::SpecComplete => StopReason::SpecComplete,
            Self::MaxIterations(_) => StopReason::IterationLimit,
            Self::NoCommits => StopReason::NoProgress,
            Self::Aborted => StopReason::Aborted,
            Self::CockpitClosed => StopReason::Cancelled,
            Self::Failed(_) | Self::NeedsHuman(_) => StopReason::Failed,
            Self::Unreconciled(_) => return None,
        })
    }
}

impl fmt::Display for LoopEnd {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NoReadyChunk => f.write_str(
                "Nothing on the roadmap is ready. Open Roadmap to review unfinished work.",
            ),
            Self::SpecComplete => f.write_str("All planned work is complete."),
            Self::MaxIterations(1) => f.write_str("Reached the limit of 1 attempt."),
            Self::MaxIterations(max) => write!(f, "Reached the limit of {max} attempts."),
            Self::NoCommits => f.write_str("The last several attempts produced no commits."),
            Self::Aborted => f.write_str("Cancelled."),
            Self::Failed(why) => f.write_str(why),
            Self::CockpitClosed => f.write_str("The application closed."),
            Self::NeedsHuman(why) => write!(f, "Your input is needed: {why}"),
            Self::Unreconciled(_) => f.write_str(
                "The previous run has work whose outcome is still unknown. Check whether its agent is still running before trying again.",
            ),
        }
    }
}
