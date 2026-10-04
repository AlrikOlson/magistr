//! The state a chunk is in, as every surface of the cockpit reads it.

use magistr_workflow_contracts::{Chunk, ChunkStatus};

/// A chunk's state as the Roadmap tab, the sky and every card read it:
/// the store's six statuses, plus the one the store spells as two fields
/// — [`Stuck`](Self::Stuck), work in progress that carries a blocker (the
/// star that breathes and wears the alert ring). Decided once, here, off
/// the store's own record, so no surface matches on the wire's spelling
/// and a status the store grows is a compile error at every reader
/// rather than a row that quietly reads as pending.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ChunkState {
    /// An idea that is not on the critical path yet.
    Backlog,
    /// Queued work; shared readiness determines whether it can start.
    Pending,
    /// A session is working it.
    InProgress,
    /// Working, but waiting on a person: in progress with a blocker.
    Stuck,
    /// Held by something outside the plan.
    Blocked,
    /// Shipped.
    Done,
    /// Overtaken by events; kept for history, never worked.
    Obsoleted,
}

impl ChunkState {
    /// Every state, in the roadmap's own order.
    pub const ALL: [Self; 7] = [
        Self::Backlog,
        Self::Pending,
        Self::InProgress,
        Self::Stuck,
        Self::Blocked,
        Self::Done,
        Self::Obsoleted,
    ];

    /// The state of the store's record: its status, or Stuck for work in
    /// progress that waits on a blocker.
    #[must_use]
    pub fn of(chunk: &Chunk) -> Self {
        match chunk.status {
            ChunkStatus::InProgress if chunk.blocked_by.is_some() => Self::Stuck,
            ChunkStatus::InProgress => Self::InProgress,
            ChunkStatus::Backlog => Self::Backlog,
            ChunkStatus::Pending => Self::Pending,
            ChunkStatus::Blocked => Self::Blocked,
            ChunkStatus::Done => Self::Done,
            ChunkStatus::Obsoleted => Self::Obsoleted,
        }
    }

    /// The canonical lifecycle status underlying this presentation state.
    #[must_use]
    pub const fn status(self) -> ChunkStatus {
        match self {
            Self::Backlog => ChunkStatus::Backlog,
            Self::Pending => ChunkStatus::Pending,
            Self::InProgress | Self::Stuck => ChunkStatus::InProgress,
            Self::Blocked => ChunkStatus::Blocked,
            Self::Done => ChunkStatus::Done,
            Self::Obsoleted => ChunkStatus::Obsoleted,
        }
    }

    /// Whether the chunk still takes part in the plan.
    #[must_use]
    pub const fn is_active(self) -> bool {
        !matches!(self, Self::Obsoleted)
    }

    /// Whether a session is on the chunk, blocked or not — what the
    /// list's *In progress* filter and the card's *Complete* verb read.
    #[must_use]
    pub const fn is_working(self) -> bool {
        matches!(self, Self::InProgress | Self::Stuck)
    }

    /// Whether the chunk shipped.
    #[must_use]
    pub const fn is_done(self) -> bool {
        matches!(self, Self::Done)
    }

    /// The projected state's spelling for diagnostics and structured data.
    #[must_use]
    pub const fn word(self) -> &'static str {
        match self {
            Self::Backlog => "backlog",
            Self::Pending => "pending",
            Self::InProgress => "in_progress",
            Self::Stuck => "stuck",
            Self::Blocked => "blocked",
            Self::Done => "done",
            Self::Obsoleted => "obsoleted",
        }
    }
}

#[cfg(test)]
#[path = "chunk_state.tests.rs"]
mod tests;
