//! The chain of checkpoints, oldest first.

use chrono::{DateTime, Utc};
use magistr_engine::SessionSnapshot;

use magistr_engine::CheckpointReason;

use super::checkpoint::Checkpoint;

/// The session's chain of verified states.
///
/// Append-only within a session: a restore does not truncate what came
/// after it, because what came after it happened, and a spine that
/// rewrote itself on every restore would lose the state a user wants to
/// come *back* to after looking at an older one. The same reason
/// [`StreamRelay::delivered`] is append-only across attempts.
///
/// Ids are *given*, not minted here, and [`next_id`] is the one mint.
/// That split is load-bearing: a permission card has to print "cp-4
/// saved" in the same fold that raises it, before the checkpoint
/// exists, so whoever prints the name must also be the one that chose
/// it. A spine that minted a second id of its own would give the rail
/// and the card two different names for one state. The counter never
/// goes backwards, so `cp-4` names one state for the life of the
/// session even if the spine is restored to `cp-2` and grows again.
///
/// [`next_id`]: CheckpointSpine::next_id
///
/// [`StreamRelay::delivered`]: magistr_engine::fleet::StreamRelay::delivered
#[derive(Debug, Clone, Default, PartialEq)]
pub struct CheckpointSpine {
    entries: Vec<Checkpoint>,
    minted: u32,
}

impl CheckpointSpine {
    /// An empty spine.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Append a checkpoint over `snapshot` under the name `id`.
    ///
    /// Refuses an id already on the spine rather than shadowing it: two
    /// states under one name is the one thing a chain a user restores
    /// from must not have.
    pub fn save(
        &mut self,
        id: impl Into<String>,
        reason: CheckpointReason,
        label: impl Into<String>,
        snapshot: SessionSnapshot,
        taken_at: DateTime<Utc>,
    ) -> Result<&Checkpoint, String> {
        let id = id.into();
        if self.get(&id).is_some() {
            return Err(format!("{id} is already on the spine"));
        }
        self.minted = self.minted.max(counter_of(&id).unwrap_or(0));
        self.entries.push(Checkpoint {
            id,
            reason,
            label: label.into(),
            taken_at,
            snapshot,
        });
        Ok(self.entries.last().expect("just pushed"))
    }

    /// The next name to give a checkpoint — the mint, for a caller that
    /// has no cockpit numbering them.
    #[must_use]
    pub fn next_id(&self) -> String {
        format!("cp-{}", self.minted + 1)
    }

    /// The checkpoint `id` names.
    #[must_use]
    pub fn get(&self, id: &str) -> Option<&Checkpoint> {
        self.entries.iter().find(|entry| entry.id == id)
    }

    /// The newest checkpoint — the ledger head.
    #[must_use]
    pub fn head(&self) -> Option<&Checkpoint> {
        self.entries.last()
    }

    /// Every checkpoint, oldest first.
    #[must_use]
    pub fn entries(&self) -> &[Checkpoint] {
        &self.entries
    }

    /// How many states the spine holds.
    #[must_use]
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Whether nothing has been checkpointed yet.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

/// The counter inside a `cp-<n>` name, so a spine told an id keeps its
/// own mint ahead of it rather than proposing a name already taken.
fn counter_of(id: &str) -> Option<u32> {
    id.strip_prefix("cp-")?.parse().ok()
}

#[cfg(test)]
#[path = "checkpoint_spine.tests.rs"]
mod tests;
