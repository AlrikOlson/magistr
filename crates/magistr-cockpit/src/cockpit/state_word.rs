//! The one state vocabulary every chip, card and star reads from.

use crate::cockpit::chunk_state::ChunkState;
use crate::cockpit::star_state::StarState;

/// Shared labels for session strips, card headers, and roadmap state.
///
/// Drawing and copy use separate enums: [`StarState`] includes Pending and
/// Ghost for rendering, which map to Blocked and Idle in visible text.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum StateWord {
    /// Everything it depends on is shipped — the next thing to claim.
    Ready,
    /// A session is working it.
    Executing,
    /// The change is waiting to be looked at.
    Review,
    /// Working, but needs a decision.
    Stuck,
    /// Cannot start.
    Blocked,
    /// Shipped.
    Done,
    /// Nothing is happening.
    Idle,
}

impl StateWord {
    /// The seven labels in display order.
    pub const ALL: [Self; 7] = [
        Self::Ready,
        Self::Executing,
        Self::Review,
        Self::Stuck,
        Self::Blocked,
        Self::Done,
        Self::Idle,
    ];

    /// The words a state chip must never say — the synonyms a view
    /// reaches for when it spells a state by hand instead of through
    /// [`StateWord::word`]. A sweep refuses any run that is exactly one
    /// of these; the list is the vocabulary's own negative space, kept
    /// beside it so a new synonym is added here and nowhere else.
    pub const IMPOSTORS: [&'static str; 18] = [
        "Pending",
        "Planned",
        "In progress",
        "In Progress",
        "Working",
        "Running",
        "Someday",
        "Backlog",
        "Shipped",
        "Paused",
        "Waiting",
        "Failed",
        "Complete",
        "Completed",
        "Active",
        "Queued",
        "Obsoleted",
        "Finished",
    ];

    /// The word, as the design spells it.
    #[must_use]
    pub const fn word(self) -> &'static str {
        match self {
            Self::Ready => "Ready",
            Self::Executing => "Executing",
            Self::Review => "Review",
            Self::Stuck => "Stuck",
            Self::Blocked => "Blocked",
            Self::Done => "Done",
            Self::Idle => "Idle",
        }
    }

    /// The word a star's state is said with.
    #[must_use]
    pub const fn of_star(state: StarState) -> Self {
        match state {
            StarState::Ghost => Self::Idle,
            StarState::Pending | StarState::Blocked => Self::Blocked,
            StarState::Ready => Self::Ready,
            StarState::Executing => Self::Executing,
            StarState::Stuck => Self::Stuck,
            StarState::Done => Self::Done,
        }
    }

    /// The word for a chunk in `state`, given whether everything it
    /// depends on has shipped — [`StarState::of`]'s reading, plus the
    /// one state the sky never draws: an obsoleted chunk is nothing
    /// happening.
    #[must_use]
    pub const fn of_state(state: ChunkState, ready: bool) -> Self {
        if matches!(state, ChunkState::Obsoleted) {
            return Self::Idle;
        }
        Self::of_star(StarState::of(state, ready))
    }

    /// The state a word names, when it is one of the seven.
    #[must_use]
    pub fn parse(word: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|state| state.word() == word)
    }
}

#[cfg(test)]
#[path = "state_word.tests.rs"]
mod tests;
