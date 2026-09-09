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

    pub(super) fn stop_reason(&self) -> StopReason {
        match self {
            Self::NoReadyChunk => StopReason::NoReadyChunk,
            Self::SpecComplete => StopReason::SpecComplete,
            Self::MaxIterations(_) => StopReason::IterationLimit,
            Self::NoCommits => StopReason::NoProgress,
            Self::Aborted => StopReason::Aborted,
            Self::CockpitClosed => StopReason::Cancelled,
            Self::Failed(_) | Self::NeedsHuman(_) => StopReason::Failed,
        }
    }
}

impl fmt::Display for LoopEnd {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NoReadyChunk => {
                f.write_str("no ready roadmap chunk — nothing left for the loop to take")
            }
            Self::SpecComplete => f.write_str("all spec items complete"),
            Self::MaxIterations(max) => write!(f, "max iterations ({max}) reached"),
            Self::NoCommits => f.write_str("too many iterations without commits"),
            Self::Aborted => f.write_str("aborted"),
            Self::Failed(why) => f.write_str(why),
            Self::CockpitClosed => f.write_str("the cockpit closed"),
            Self::NeedsHuman(why) => write!(f, "the judge needs a human: {why}"),
        }
    }
}
