use serde::{Deserialize, Serialize};

use super::batch_row::BatchRow;
use super::batch_state::BatchState;

/// What a whole batch run amounts to, in one answer.
///
/// The acceptance's fourth criterion is that batch progress is one
/// rollup rather than N unrelated session streams, and the shape of that
/// guarantee is here: the counts are folded from the same
/// [`BatchState`]s the rows carry, and those are derived from the work
/// queue rather than tallied as things happen. A caller cannot read a
/// count that disagrees with the rows beside it, and neither can
/// disagree with the queue.
///
/// The per-project event streams still exist and are still per-project —
/// `work.events` reads each one by its own cursor. What this replaces is
/// having to *poll* N of them to learn whether the run is done.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BatchStatus {
    /// The batch this describes.
    pub batch_id: String,
    /// How many projects the batch covers.
    pub total: usize,
    /// Held back by a dependency or a busy working tree.
    pub waiting: usize,
    /// Released, waiting for a node.
    pub queued: usize,
    /// Being run by a node.
    pub running: usize,
    /// Finished successfully.
    pub succeeded: usize,
    /// Finished unsuccessfully.
    pub failed: usize,
    /// Cancelled by an upstream failure.
    pub skipped: usize,
    /// Whether every project has settled — nothing left to release and
    /// nothing still running.
    pub settled: bool,
    /// The projects, in dependency order.
    pub rows: Vec<BatchRow>,
}

impl BatchStatus {
    /// Fold the rows into the counts. The one construction site, so a
    /// count can never be written by hand.
    #[must_use]
    pub fn from_rows(batch_id: impl Into<String>, rows: Vec<BatchRow>) -> Self {
        let count = |wanted: BatchState| rows.iter().filter(|row| row.state == wanted).count();
        Self {
            batch_id: batch_id.into(),
            total: rows.len(),
            waiting: count(BatchState::Waiting),
            queued: count(BatchState::Queued),
            running: count(BatchState::Running),
            succeeded: count(BatchState::Succeeded),
            failed: count(BatchState::Failed),
            skipped: count(BatchState::Skipped),
            settled: rows.iter().all(|row| row.state.is_terminal()),
            rows,
        }
    }

    /// Whether anything in the batch went wrong — a failure of its own,
    /// or a project cancelled by one.
    #[must_use]
    pub fn has_losses(&self) -> bool {
        self.failed > 0 || self.skipped > 0
    }
}

#[cfg(test)]
#[path = "batch_status.tests.rs"]
mod tests;
