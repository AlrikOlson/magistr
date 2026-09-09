//! Which surface the stage shows.

/// The stage's tabs: the launcher, the agent's turn, the iteration's
/// changes, the reasoning trace and the roadmap's tech-tree.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum StageTab {
    /// The launcher — project status, roadmap progress, recent
    /// sessions and the actions (what bare `magistr` opens).
    Home,
    /// The agent's turn — the transcript.
    #[default]
    Turn,
    /// What the iteration changed.
    Changes,
    /// The reasoning trace.
    Trace,
    /// The roadmap.
    Roadmap,
}

impl StageTab {
    /// Every tab, in bar order.
    pub const ALL: [Self; 5] = [
        Self::Home,
        Self::Turn,
        Self::Changes,
        Self::Trace,
        Self::Roadmap,
    ];

    /// The tab's label on the bar.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Home => "Home",
            Self::Turn => "Turn",
            Self::Changes => "Changes",
            Self::Trace => "Trace",
            Self::Roadmap => "Roadmap",
        }
    }

    /// The tab to the right, wrapping — what the Tab key selects.
    #[must_use]
    pub const fn next(self) -> Self {
        match self {
            Self::Home => Self::Turn,
            Self::Turn => Self::Changes,
            Self::Changes => Self::Trace,
            Self::Trace => Self::Roadmap,
            Self::Roadmap => Self::Home,
        }
    }

    /// The tab to the left, wrapping — what Shift-Tab selects.
    #[must_use]
    pub const fn prev(self) -> Self {
        match self {
            Self::Home => Self::Roadmap,
            Self::Turn => Self::Home,
            Self::Changes => Self::Turn,
            Self::Trace => Self::Changes,
            Self::Roadmap => Self::Trace,
        }
    }
}

#[cfg(test)]
#[path = "stage_tab.tests.rs"]
mod tests;
