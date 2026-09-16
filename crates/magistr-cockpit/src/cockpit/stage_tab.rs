//! Which surface the stage shows.

/// The launcher, the agent's turn, the session's work, and the roadmap.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum StageTab {
    Home,
    #[default]
    Turn,
    Session,
    Roadmap,
}

impl StageTab {
    /// Every tab, in bar order.
    pub const ALL: [Self; 4] = [Self::Home, Self::Turn, Self::Session, Self::Roadmap];

    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Home => "Home",
            Self::Turn => "Turn",
            Self::Session => "Session",
            Self::Roadmap => "Roadmap",
        }
    }

    #[must_use]
    pub const fn next(self) -> Self {
        match self {
            Self::Home => Self::Turn,
            Self::Turn => Self::Session,
            Self::Session => Self::Roadmap,
            Self::Roadmap => Self::Home,
        }
    }

    #[must_use]
    pub const fn prev(self) -> Self {
        match self {
            Self::Home => Self::Roadmap,
            Self::Turn => Self::Home,
            Self::Session => Self::Turn,
            Self::Roadmap => Self::Session,
        }
    }
}

#[cfg(test)]
#[path = "stage_tab.tests.rs"]
mod tests;
