//! The store, read whole.

use crate::roadmap::Chunk;
use crate::ship::Objective;
use crate::ship::Task;
use crate::signal::Signal;
use crate::think::ThinkStep;

use super::snapshot_cursor::SnapshotCursor;
use crate::RoadmapProgress;

/// Everything a surface shows before the feed speaks, read off the
/// engines by the host's native snapshot reader:
/// the ship objective and its tasks with their actions and checks whole,
/// the roadmap's chunks and progress, every signal, and the persisted
/// reasoning steps with their branch metadata.
///
/// What it guarantees, and what it does not. Each family is read whole
/// under its own engine lock, so within a family the snapshot is one
/// consistent state — a task and the checks on it agree. Across families
/// there is no such guarantee: the four families are four files with four
/// locks, and a write landing between two reads is visible in one and not
/// the other. The [`SnapshotCursor`] is taken *before* the families are
/// read, so a write that lands during the read moves the next cursor and
/// the timer re-reads. No global revision is fabricated over the four:
/// the store has none, and a projection that claimed one would be lying.
/// The reasoning steps and branch metadata come from one persisted archive
/// read rather than the live engine (the live load path can print, which
/// a raw-mode terminal must never see), so they may trail the trace by
/// one unflushed step.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct NativeSnapshot {
    /// The store stamp read before the families; `None` when the store is
    /// not persisted or has no files yet.
    pub cursor: Option<SnapshotCursor>,
    /// The ship objective, once one is set.
    pub objective: Option<Objective>,
    /// The planned tasks in plan order, actions and checks included.
    pub tasks: Vec<Task>,
    /// The roadmap's progress as the engine counts it.
    pub progress: RoadmapProgress,
    /// Every chunk, in stored order.
    pub chunks: Vec<Chunk>,
    /// Every signal, in capture order.
    pub signals: Vec<Signal>,
    /// The persisted reasoning steps, in stored order.
    pub steps: Vec<ThinkStep>,
    /// Branch names, origins, statuses and explicit merge destinations from the same archive.
    pub branches: Vec<super::ThinkBranch>,
}
