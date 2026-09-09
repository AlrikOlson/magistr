//! What the iteration-loop driver reports.

use std::time::Duration;

use super::loop_stage::LoopStage;
use crate::job::JobId;

/// One thing the loop driver said, as [`Msg::Loop`](super::Msg::Loop)
/// carries it. The driver runs in the entry point's crate and owns every
/// effect of the loop; the cockpit only folds these in.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum LoopEvent {
    /// An iteration started: the stage is cleared for it.
    Started {
        /// Which iteration (1-based).
        iteration: u32,
        /// Of how many.
        max: u32,
    },
    /// How long the iteration has been running.
    Elapsed(Duration),
    /// A gate started running as a job; its result arrives as
    /// [`Msg::Job`](super::Msg::Job) with that id.
    GateStarted {
        /// The gate's name.
        name: String,
        /// The job running it.
        job: JobId,
    },
    /// Something worth a line on the stage.
    Note(String),
    /// Something went wrong; the loop may or may not go on.
    Failed(String),
    /// The driver moved to `stage` within the iteration — what the
    /// window's cap says while the feed is expectedly gone between
    /// agents.
    Stage(LoopStage),
    /// The driver paused the loop itself (failed gates, by configuration).
    Paused(String),
    /// The loop ended, and why.
    Ended(String),
    /// The judge's verdict on the iteration's chunk — already reconciled
    /// by the driver against the real gates, so what follows it (a
    /// completion, another iteration, a blocker card, the loop's end)
    /// is the driver's doing; the cockpit only shows the verdict.
    Verdict {
        /// The iteration it judged (1-based).
        iteration: u32,
        /// The judge said the acceptance is met.
        ok: bool,
        /// The judge said it cannot be met as written.
        impossible: bool,
        /// The judge's reason.
        reason: String,
    },
    /// An iteration ended: its gates ran, and a boundary checkpoint is
    /// due. The cockpit mints the row (`cp-N · iter N`, wearing the
    /// verdicts) and asks the port for it under that name; the driver
    /// serves the ask on its own thread — *after* the iteration's
    /// session is gone and *before* the next handshake begins, never at
    /// [`Started`](Self::Started), where one git read once sat on a
    /// delegated goal's echo deadline.
    Boundary {
        /// The iteration that ended (1-based).
        iteration: u32,
        /// Each gate that ran, in order, and whether it passed.
        gates: Vec<(String, bool)>,
    },
}
