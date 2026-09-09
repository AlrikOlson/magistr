//! What the cockpit asks for a project being born.

use super::ground::Ground;
use super::proposal::Proposal;
use super::proposed_chunk::ProposedChunk;

/// A Birth request from the cockpit, carried by
/// [`Cmd::Birth`](super::Cmd::Birth): the entry point that owns the
/// file system serves it as a job — reading the ground, writing the
/// scaffold, drafting the proposal, writing the store — and posts the
/// answer back as a [`JobOutput`](crate::job::JobOutput). The cockpit
/// never touches a folder itself: it knows no file system and opens no
/// store.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum BirthCmd {
    /// F1 → F2: the ground was chosen; read it with `magistr init`'s
    /// autodetect, write the scaffold, and answer with the survey. A
    /// clone is cloned first, an empty ground is created first.
    Survey {
        /// Where the project stands.
        ground: Ground,
    },
    /// F3 → F4: the interview is answered; draft the proposal from the
    /// survey and the answers, charting around what is pinned.
    Chart {
        /// The project's folder.
        dir: String,
        /// The project's name.
        name: String,
        /// The three answers: the goal, what must never break, how
        /// free the agent is.
        answers: [String; 3],
        /// The chunks the reader pinned — kept as they are.
        pinned: Vec<ProposedChunk>,
    },
    /// F4 → F5: the proposal is accepted; write it as the project's
    /// roadmap — `roadmap_add_chunk` per star, deps and priorities
    /// included — with the gates and protected paths the interview
    /// configured, and answer with what was written.
    Accept {
        /// The project's folder.
        dir: String,
        /// The project's name.
        name: String,
        /// The proposal as the list shows it.
        proposal: Proposal,
        /// What must never break — required gates and protected paths.
        never_break: Vec<String>,
        /// The charter dial's starting notch, when one was picked.
        freedom: Option<usize>,
    },
}
