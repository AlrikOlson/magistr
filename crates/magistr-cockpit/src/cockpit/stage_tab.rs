//! Which surface the stage shows.

/// The agent's turn, the session's work, and the roadmap.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum StageTab {
    Turn,
    Session,
    #[default]
    Roadmap,
}

impl StageTab {
    /// Every tab, in bar order.
    pub const ALL: [Self; 3] = [Self::Turn, Self::Session, Self::Roadmap];

    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Turn => "Turn",
            Self::Session => "Session",
            Self::Roadmap => "Roadmap",
        }
    }

    #[must_use]
    pub const fn next(self) -> Self {
        match self {
            Self::Turn => Self::Session,
            Self::Session => Self::Roadmap,
            Self::Roadmap => Self::Turn,
        }
    }

    #[must_use]
    pub const fn prev(self) -> Self {
        match self {
            Self::Turn => Self::Roadmap,
            Self::Session => Self::Turn,
            Self::Roadmap => Self::Session,
        }
    }
}

#[cfg(test)]
#[path = "stage_tab.tests.rs"]
mod tests;
