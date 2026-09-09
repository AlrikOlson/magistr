//! Which of the two ways a dependency edge is drawn.

/// A dependency edge's kind: the
/// dependency is either behind you or ahead of you, and nothing else.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum EdgeKind {
    /// The chunk this edge comes from has shipped — the path is walked,
    /// and the line is solid.
    Done,
    /// It has not — the path is still ahead, and the line is dashed.
    Ahead,
}

impl EdgeKind {
    /// Both kinds, in the order specimen II.b lays them out.
    pub const ALL: [Self; 2] = [Self::Done, Self::Ahead];

    /// The kind of an edge out of a chunk with this wire `status`.
    #[must_use]
    pub fn of(status: &str) -> Self {
        if status == "done" {
            Self::Done
        } else {
            Self::Ahead
        }
    }

    /// Whether the line is drawn as dashes rather than as one run.
    #[must_use]
    pub const fn is_dashed(self) -> bool {
        matches!(self, Self::Ahead)
    }
}

#[cfg(test)]
#[path = "edge_kind.tests.rs"]
mod tests;
