//! Where the window's camera is — altitudes.

use super::stage_tab::StageTab;

/// One of the places a window can be at: the
/// Galaxy at altitude 0; at 1 the two rooms of one project, the Sky and
/// the Signals inbox; the Session at 2 and, at 3, the records the
/// window shows — the Changes journal, the Trace feed, and the Report
/// of a shipped star (view 09).
///
/// A Moment is **not** a Place. The decision card comes to you over
/// whatever place you are at — [`Model::moment`](super::Model::moment)
/// is the dock's own front — and that is what makes "an answer never
/// moves the camera" a property of the data rather than of every
/// answer site: answering pops the dock and the place is untouched.
/// Birth is a place — a project being made, altitude 1 beside the
/// Sky — whose five moments are [`Model::birth`](super::Model::birth)'s
/// own sequence, not altitudes.
///
/// Native-only state, the Brief's pattern: a terminal navigates by
/// [`StageTab`] and has no Place at all
/// ([`Model::place`](super::Model::place) is `None` there).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Place {
    /// Altitude 0 — every project.
    Galaxy,
    /// Altitude 1 — one project: the roadmap as a constellation.
    Sky,
    /// Altitude 1 — one project: the stakeholder inbox (view 12), the
    /// room beside the Sky where a signal waits to become a star.
    Signals,
    /// Altitude 1 — a project being made:
    /// the five moments of [`Birth`](super::Birth), from the ground to
    /// first light, reached by `＋ New project` on the Galaxy or a
    /// folder dropped on the window.
    Birth,
    /// Altitude 2 — one session: the turn and its rail.
    #[default]
    Session,
    /// Altitude 3 — the session's Changes journal.
    Changes,
    /// Altitude 3 — the session's reasoning trace.
    Trace,
    /// Altitude 3 — a shipped star opened: the chapter and its
    /// deliverables shelf (view 09). The record of the selected Done
    /// chunk, reached through the open-chunk fold.
    Report,
}

impl Place {
    /// Every place, in altitude order.
    pub const ALL: [Self; 8] = [
        Self::Galaxy,
        Self::Sky,
        Self::Signals,
        Self::Birth,
        Self::Session,
        Self::Changes,
        Self::Trace,
        Self::Report,
    ];

    /// The navigation depth.
    #[must_use]
    pub const fn altitude(self) -> u8 {
        match self {
            Self::Galaxy => 0,
            Self::Sky | Self::Signals | Self::Birth => 1,
            Self::Session => 2,
            Self::Changes | Self::Trace | Self::Report => 3,
        }
    }

    /// One altitude up — what Esc climbs to. `None` at the Galaxy,
    /// which is the top: its Esc is spent and holds. The Signals inbox
    /// climbs to the Sky: it is the project's second room, and leaving
    /// it lands you back in the first. The Report climbs to the Sky
    /// too: it is a star opened, and there is no session under a
    /// shipped star to land on — leaving the chapter puts you back on
    /// the constellation the star is on. Birth climbs to the Galaxy:
    /// it was opened from there, and a project not yet born has no
    /// Sky of its own to land on (the flow's own Esc stands its later
    /// moments down first — [`nav::esc`](super::nav::esc)).
    #[must_use]
    pub const fn up(self) -> Option<Self> {
        match self {
            Self::Galaxy => None,
            Self::Sky | Self::Birth => Some(Self::Galaxy),
            Self::Signals => Some(Self::Sky),
            Self::Session => Some(Self::Sky),
            Self::Changes | Self::Trace => Some(Self::Session),
            Self::Report => Some(Self::Sky),
        }
    }

    /// The next place along [`Place::ALL`], wrapping — what the Tab key
    /// walks on a window, the way it walks a terminal's tabs.
    #[must_use]
    pub const fn next(self) -> Self {
        match self {
            Self::Galaxy => Self::Sky,
            Self::Sky => Self::Signals,
            Self::Signals => Self::Birth,
            Self::Birth => Self::Session,
            Self::Session => Self::Changes,
            Self::Changes => Self::Trace,
            Self::Trace => Self::Report,
            Self::Report => Self::Galaxy,
        }
    }

    /// The place before, wrapping — what Shift-Tab walks.
    #[must_use]
    pub const fn prev(self) -> Self {
        match self {
            Self::Galaxy => Self::Report,
            Self::Sky => Self::Galaxy,
            Self::Signals => Self::Sky,
            Self::Birth => Self::Signals,
            Self::Session => Self::Birth,
            Self::Changes => Self::Session,
            Self::Trace => Self::Changes,
            Self::Report => Self::Trace,
        }
    }

    /// The stage surface this place draws. The Galaxy takes the
    /// launcher's slot — the window has no launcher, and altitude 0 is
    /// what stands where a terminal's Home does — which also keeps the
    /// Roadmap tab's zoom keys off it: the Galaxy zooms its own field.
    /// The Signals inbox draws the Roadmap tab's surface too: on a
    /// terminal the inbox is a section of that tab, and the window's
    /// composition root routes the place itself. So does the Report:
    /// on a terminal a shipped chunk is its row on the Roadmap tab.
    /// And so does Birth: first light is a sky, and the moments before
    /// it are the window's own.
    #[must_use]
    pub const fn tab(self) -> StageTab {
        match self {
            Self::Galaxy => StageTab::Home,
            Self::Sky | Self::Signals | Self::Report | Self::Birth => StageTab::Roadmap,
            Self::Session => StageTab::Turn,
            Self::Changes | Self::Trace => StageTab::Session,
        }
    }

    /// The place a terminal tab names on a window: the launcher's slot
    /// is the Sky (the window has no launcher — bare `magistr --native`
    /// opens the Sky), and the rest are their altitudes. The Roadmap
    /// tab names the Sky and never the Signals or the Report: a window
    /// opened on the roadmap opens the constellation, and the inbox and
    /// the chapters are reached from it.
    #[must_use]
    pub const fn of_tab(tab: StageTab) -> Self {
        match tab {
            StageTab::Home | StageTab::Roadmap => Self::Sky,
            StageTab::Turn => Self::Session,
            StageTab::Session => Self::Trace,
        }
    }
}

#[cfg(test)]
#[path = "place.tests.rs"]
mod tests;
