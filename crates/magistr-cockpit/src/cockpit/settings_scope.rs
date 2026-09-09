//! The three scopes of ⌘, — the sidebar's sections.

/// Where a setting lives, and so which file it is: the app (the user's
/// own config), the project (its `.magistr/project.json`) or the team
/// (its `.magistr/team.json`). Scopes are the sidebar's *sections*,
/// never tabs: named, so ownership
/// is visible before a click.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum SettingsScope {
    /// This app, all windows — the user's config.
    App,
    /// This project — its own config, under its sigil.
    Project,
    /// This team — read-only here; the team's file says who owns it.
    Team,
}

impl SettingsScope {
    /// Every scope, in the sidebar's order.
    pub const ALL: [Self; 3] = [Self::App, Self::Project, Self::Team];

    /// The section's cap, before any count.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::App => "This app",
            Self::Project => "This project",
            Self::Team => "This team",
        }
    }

    /// How a pane's sub-line names the scope: `Scope: <this>`.
    #[must_use]
    pub const fn reach(self) -> &'static str {
        match self {
            Self::App => "this app, all windows",
            Self::Project => "this project",
            Self::Team => "this team",
        }
    }

    /// How a pane's sub-line names the source: `source: <this>`.
    #[must_use]
    pub const fn source_name(self) -> &'static str {
        match self {
            Self::App => "user config",
            Self::Project => "project config",
            Self::Team => "team config",
        }
    }

    /// The file the scope's settings are — what every pane ends with
    /// and what `Open file` opens. Settings that are files stay files.
    #[must_use]
    pub const fn source(self) -> &'static str {
        match self {
            Self::App => "~/.config/magistr/config.toml",
            Self::Project => ".magistr/project.json",
            Self::Team => ".magistr/team.json",
        }
    }
}
