//! The theme picker's three cards.

use crate::theme::ThemeMode;

/// The theme choice: forced dark, forced light, or following the OS.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum ThemeChoice {
    /// Night, forced.
    Sky,
    /// Day, forced.
    Chart,
    /// Whatever the OS says.
    #[default]
    FollowOs,
}

impl ThemeChoice {
    /// The three cards, in the picker's order.
    pub const ALL: [Self; 3] = [Self::Sky, Self::Chart, Self::FollowOs];

    /// The card's label.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Sky => "The sky",
            Self::Chart => "The chart",
            Self::FollowOs => "Follow OS",
        }
    }

    /// The mode a card forces; `None` when it follows the OS.
    #[must_use]
    pub const fn forced(self) -> Option<ThemeMode> {
        match self {
            Self::Sky => Some(ThemeMode::Dark),
            Self::Chart => Some(ThemeMode::Light),
            Self::FollowOs => None,
        }
    }

    /// The card at `index` in [`ThemeChoice::ALL`], when there is one.
    #[must_use]
    pub fn at(index: usize) -> Option<Self> {
        Self::ALL.get(index).copied()
    }
}
