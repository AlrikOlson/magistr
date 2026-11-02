//! One typed change announced by the native workflow store or the host.

use crate::Family;

use super::magistr_frame::MagistrFrame;
use super::{DecisionFrame, PlanFrame, RequestFrame, RunFrame};

/// One thing that happened, as the engines say it: the workflow's own
/// typed frame for each of its four families — the very values the
/// engines hand their broadcaster — plus the host's `magistr` family.
///
/// This is the one vocabulary every surface folds. The socket carries the
/// same values as JSON lines; [`NativeFrame::decode`](super::NativeFrame::decode)
/// turns a line back into one of these at the edge, and nothing past that
/// edge reads a family or a kind as a string. An in-process writer's own
/// answer (a task the cockpit started, a gate the loop ran) is built as
/// one of these directly and never serialized on the way.
///
/// Plan and request frames carry the whole record, so folding one is
/// idempotent; decision frames carry the whole step; run frames are deltas
/// over the goal (a task started, a check recorded), so a gap in the
/// feed is repaired by a fresh [`NativeSnapshot`](super::NativeSnapshot),
/// not by replaying.
// The plan variant carries the whole `Task` on purpose — a fold over
// it is idempotent because the record is complete — and an event is moved
// from the decoder to the fold and dropped, never kept in bulk, so boxing
// the record would buy an allocation per frame and no memory.
#[allow(clippy::large_enum_variant)]
#[derive(Debug, Clone, PartialEq)]
pub enum NativeEvent {
    /// The decision log changed.
    Decision(DecisionFrame),
    /// The run history changed.
    Run(RunFrame),
    /// A plan task changed, or a refresh was recorded.
    Plan(PlanFrame),
    /// A stakeholder request changed.
    Request(RequestFrame),
    /// The host said something about the run itself.
    Magistr(MagistrFrame),
}

impl NativeEvent {
    /// Which family this event belongs to.
    #[must_use]
    pub const fn family(&self) -> Family {
        match self {
            Self::Decision(_) => Family::Decision,
            Self::Run(_) => Family::Run,
            Self::Plan(_) => Family::Plan,
            Self::Request(_) => Family::Request,
            Self::Magistr(_) => Family::Magistr,
        }
    }
}
