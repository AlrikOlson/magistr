//! The values the settings panes render.

use super::density::Density;
use super::theme_choice::ThemeChoice;

/// What the ⌘, panes show and edit: the app's choices, and the
/// identity the project and team sections carry. The entry point
/// fills it from the real files the panes name (user config, project
/// config, team config); the defaults are what a fresh install shows.
/// The model keeps the working copy — a switch toggles here at once —
/// and writing it back is the source file's own business.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SettingsData {
    /// Which theme card is chosen — follows the OS until forced.
    pub theme: ThemeChoice,
    /// Show the activity heatmap on the project view.
    pub heatmap: bool,
    /// The sky's film grain, night only.
    pub grain: bool,
    /// Row height and padding.
    pub density: Density,
    /// The project's path as the identity row prints it
    /// (`~/Code/magistr`); empty until the entry point says.
    pub project_path: String,
    /// How many people the team file lists; `None` without a team.
    pub team_size: Option<usize>,
    /// The rows the terminal dock was last dragged to on this project;
    /// `None` until anyone drags, when the first rise is a
    /// third of the stage.
    pub dock_rows: Option<u16>,
}

impl Default for SettingsData {
    fn default() -> Self {
        Self {
            theme: ThemeChoice::FollowOs,
            heatmap: true,
            grain: true,
            density: Density::Comfortable,
            project_path: String::new(),
            team_size: None,
            dock_rows: None,
        }
    }
}
