//! The five moments a project is born through.

/// One of Birth's five moments: the ground is chosen, the survey
/// reads it, the interview asks three questions, the charting draws
/// the proposal, and first light is the accepted sky. The moments are
/// a sequence, not altitudes: Esc from the first rises to the Galaxy,
/// from any later one stands the flow down to the first with what the
/// survey found kept.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum BirthMoment {
    /// F1 — choose the ground: a folder, a repository, or nothing.
    #[default]
    Ground,
    /// F2 — the survey: `magistr init`'s autodetect and scaffold, drawn
    /// as a sweep.
    Survey,
    /// F3 — the interview: three questions as a card stack.
    Interview,
    /// F4 — the charting: the proposal drawn star by star, editable.
    Charting,
    /// F5 — first light: the accepted sky and its first claim.
    FirstLight,
}

impl BirthMoment {
    /// Every moment, in the order the flow walks them.
    pub const ALL: [Self; 5] = [
        Self::Ground,
        Self::Survey,
        Self::Interview,
        Self::Charting,
        Self::FirstLight,
    ];

    /// The setup stage ID: `F1`…`F5`.
    #[must_use]
    pub const fn view(self) -> &'static str {
        match self {
            Self::Ground => "F1",
            Self::Survey => "F2",
            Self::Interview => "F3",
            Self::Charting => "F4",
            Self::FirstLight => "F5",
        }
    }

    /// The moment's name, as shown on screen.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Ground => "Choose the ground",
            Self::Survey => "The survey",
            Self::Interview => "The interview",
            Self::Charting => "The charting",
            Self::FirstLight => "First light",
        }
    }
}

#[cfg(test)]
#[path = "birth_moment.tests.rs"]
mod tests;
