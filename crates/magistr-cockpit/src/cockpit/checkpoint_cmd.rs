//! What the cockpit asks of the checkpoint spine.

use magistr_engine::CheckpointReason;

/// A checkpoint request from the cockpit, carried by
/// [`Cmd::Checkpoint`](super::Cmd::Checkpoint): the consumer runs it
/// through a [`CheckpointPort`](crate::checkpoint::CheckpointPort) and
/// posts the outcome back as a
/// [`CheckpointEvent`](super::CheckpointEvent).
///
/// Not to be confused with [`EngineCmd::Checkpoint`], which is
/// think-and-ship's *trace* checkpoint — a metacognitive report over
/// the reasoning. This one is a state of the session and its tree.
///
/// [`EngineCmd::Checkpoint`]: super::EngineCmd::Checkpoint
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum CheckpointCmd {
    /// Take a checkpoint under the name the spine already minted.
    ///
    /// The id travels *down* rather than coming back up, because the
    /// permission card that triggered the save has already printed it.
    Save {
        /// The name the spine minted and the card printed.
        id: String,
        /// Why it is being taken.
        reason: CheckpointReason,
        /// What it precedes, in the reader's words.
        label: String,
    },
    /// `/restore <id>`: resume the session from that state.
    Restore {
        /// The checkpoint to go back to.
        id: String,
    },
    /// `/fork <id> [branch]`: open a worktree branch at that state,
    /// leaving head where it is.
    Fork {
        /// The checkpoint to branch from.
        id: String,
        /// The branch to open, or `None` for the port's default.
        branch: Option<String>,
    },
    /// Bring a forked worktree home — the second half of a review's
    /// *Approve & merge*.
    ///
    /// It names the checkpoint rather than the branch for the same
    /// reason [`Fork`](Self::Fork) does: the checkpoint is what the
    /// reader picked and what the review is scoped since, and the
    /// branch a fork of it opened is derivable from it. A review
    /// scoped to the journal has no checkpoint to name and therefore
    /// asks for no merge.
    Merge {
        /// The checkpoint whose worktree is being brought home.
        id: String,
    },
}
