//! Why the loop stopped.

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
