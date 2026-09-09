//! ⌘, settings: a searchable sidebar of scoped groups, one pane each.

use super::settings_group::SettingsGroup;

/// The ⌘, surface's state: which group's pane is open and what has
/// been typed into the sidebar's search. The *values* live in
/// [`SettingsData`](super::settings_data::SettingsData) on the model,
/// whether or not the surface is up; this is only where the reader is
/// in the book of settings.
///
/// Raised only on a native surface, the palette's rule: settings is
/// one of the window's floats. It holds still — no
/// motion, no clock.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Settings {
    group: SettingsGroup,
    query: String,
}

impl Settings {
    /// The group whose pane is open.
    #[must_use]
    pub const fn group(&self) -> SettingsGroup {
        self.group
    }

    /// What has been typed into `Search settings`.
    #[must_use]
    pub fn query(&self) -> &str {
        &self.query
    }

    /// The groups the sidebar shows: every group while nothing is
    /// typed, else the ones whose label carries the query (ASCII
    /// case-folded), in the sidebar's own order. A pane stays open
    /// even when its group is filtered out of the sidebar.
    #[must_use]
    pub fn visible(&self) -> Vec<SettingsGroup> {
        let needle = self.query.trim().to_ascii_lowercase();
        SettingsGroup::ALL
            .into_iter()
            .filter(|group| {
                needle.is_empty() || group.label().to_ascii_lowercase().contains(&needle)
            })
            .collect()
    }

    /// Open a group's pane.
    pub(crate) const fn set_group(&mut self, group: SettingsGroup) {
        self.group = group;
    }

    /// Type one character into the search.
    pub(crate) fn type_char(&mut self, c: char) {
        self.query.push(c);
    }

    /// Erase the last character.
    pub(crate) fn backspace(&mut self) {
        self.query.pop();
    }

    /// Walk to the visible group above the open one; the first holds.
    pub(crate) fn up(&mut self) {
        let visible = self.visible();
        let at = visible.iter().position(|g| *g == self.group);
        match at {
            Some(0) => {}
            Some(index) => self.group = visible[index - 1],
            None => {
                if let Some(first) = visible.first() {
                    self.group = *first;
                }
            }
        }
    }

    /// Walk to the visible group below the open one; the last holds.
    pub(crate) fn down(&mut self) {
        let visible = self.visible();
        let at = visible.iter().position(|g| *g == self.group);
        match at {
            Some(index) if index + 1 < visible.len() => self.group = visible[index + 1],
            Some(_) => {}
            None => {
                if let Some(first) = visible.first() {
                    self.group = *first;
                }
            }
        }
    }
}

#[cfg(test)]
#[path = "settings.tests.rs"]
mod tests;
