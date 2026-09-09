//! The groups of ⌘, — one pane each, under their scope.

use super::settings_scope::SettingsScope;

/// One group in the settings sidebar: a place the pane can show, under
/// one of the three scopes. The order of [`SettingsGroup::ALL`] is the
/// sidebar's: the app's four, the project's four, the team's two.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum SettingsGroup {
    /// Theme, interface switches, density.
    #[default]
    Appearance,
    /// Whether anything moves.
    Motion,
    /// When the window rests.
    Rest,
    /// The chords.
    Shortcuts,
    /// The project's quality gates.
    Gates,
    /// Budget & limits.
    Budget,
    /// The charter.
    Charter,
    /// The agent backend.
    Backend,
    /// The team's roster — read at the nav level.
    Members,
    /// The team's policy — read-only, labelled at the nav level.
    Policy,
}

impl SettingsGroup {
    /// Every group, in the sidebar's order.
    pub const ALL: [Self; 10] = [
        Self::Appearance,
        Self::Motion,
        Self::Rest,
        Self::Shortcuts,
        Self::Gates,
        Self::Budget,
        Self::Charter,
        Self::Backend,
        Self::Members,
        Self::Policy,
    ];

    /// The group's word in the sidebar and at the head of its pane.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Appearance => "Appearance",
            Self::Motion => "Motion",
            Self::Rest => "Rest",
            Self::Shortcuts => "Shortcuts",
            Self::Gates => "Gates",
            Self::Budget => "Budget & limits",
            Self::Charter => "Charter",
            Self::Backend => "Agent backend",
            Self::Members => "Members",
            Self::Policy => "Policy",
        }
    }

    /// The scope the group sits under.
    #[must_use]
    pub const fn scope(self) -> SettingsScope {
        match self {
            Self::Appearance | Self::Motion | Self::Rest | Self::Shortcuts => SettingsScope::App,
            Self::Gates | Self::Budget | Self::Charter | Self::Backend => SettingsScope::Project,
            Self::Members | Self::Policy => SettingsScope::Team,
        }
    }

    /// The file the group's pane ends with.
    #[must_use]
    pub const fn source(self) -> &'static str {
        self.scope().source()
    }

    /// Whether the group is read-only here — a team-owned value the
    /// nav labels before you click.
    #[must_use]
    pub const fn is_read_only(self) -> bool {
        matches!(self, Self::Policy)
    }

    /// The group at `index` in [`SettingsGroup::ALL`], when there is one.
    #[must_use]
    pub fn at(index: usize) -> Option<Self> {
        Self::ALL.get(index).copied()
    }

    /// The group's index in [`SettingsGroup::ALL`].
    #[must_use]
    pub fn index(self) -> usize {
        Self::ALL
            .iter()
            .position(|group| *group == self)
            .expect("every group is in ALL")
    }
}

#[cfg(test)]
#[path = "settings_group.tests.rs"]
mod tests;
