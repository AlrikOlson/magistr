//! Persisted desktop appearance over the personal configuration owner.
//! Blocking reads and writes stay off the event loop; one retained revision
//! connects an editable snapshot with the exact document the user observed.

use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use color_eyre::eyre;
use serde::{Deserialize, Serialize};

use crate::config::inputs::ProjectInputs;
use crate::config::user_config::{
    ThemePreference, UserConfig, UserConfigDocument, UserConfigError,
};

/// Preferences supported by the desktop.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DesktopPreferences {
    pub theme: ThemePreference,
    pub reduce_motion: bool,
}

impl From<&UserConfig> for DesktopPreferences {
    fn from(config: &UserConfig) -> Self {
        Self {
            theme: config.theme.default,
            reduce_motion: config.accessibility.reduce_motion.unwrap_or(false),
        }
    }
}

/// The current preferences and the service revision required to save them.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct DesktopPreferencesSnapshot {
    pub preferences: DesktopPreferences,
    pub revision: u64,
}

/// Application owner shared by desktop settings commands.
#[derive(Debug, Clone)]
pub struct DesktopPreferencesService {
    path: Option<PathBuf>,
    state: Arc<Mutex<PreferencesState>>,
}

#[derive(Debug, Default)]
struct PreferencesState {
    revision: u64,
    document: Option<UserConfigDocument>,
}

impl DesktopPreferencesService {
    /// Capture the personal-settings location once, without reading the filesystem.
    #[must_use]
    pub fn new(inputs: ProjectInputs) -> Self {
        Self {
            path: inputs.user_config_path(),
            state: Arc::default(),
        }
    }

    /// Refresh the saved values. An invalid or unreadable file remains an error.
    pub async fn load(&self) -> eyre::Result<DesktopPreferencesSnapshot> {
        let service = self.clone();
        tokio::task::spawn_blocking(move || service.load_blocking())
            .await
            .map_err(|error| unavailable("read preferences", error))?
    }

    /// Save only the displayed revision and return the newly accepted values.
    /// Refusal leaves the retained document and revision unchanged.
    pub async fn save(
        &self,
        expected_revision: u64,
        preferences: DesktopPreferences,
    ) -> eyre::Result<DesktopPreferencesSnapshot> {
        let service = self.clone();
        tokio::task::spawn_blocking(move || service.save_blocking(expected_revision, preferences))
            .await
            .map_err(|error| unavailable("save preferences", error))?
    }

    fn load_blocking(&self) -> eyre::Result<DesktopPreferencesSnapshot> {
        let mut state = self
            .state
            .lock()
            .map_err(|error| unavailable("lock preferences", error))?;
        let document = self
            .path
            .as_deref()
            .map(UserConfigDocument::load)
            .transpose()
            .map_err(|error| config_error(PreferenceOperation::Read, error))?;
        let revision = next_revision(state.revision)?;
        state.document = document;
        state.revision = revision;
        Ok(state.snapshot())
    }

    fn save_blocking(
        &self,
        expected_revision: u64,
        preferences: DesktopPreferences,
    ) -> eyre::Result<DesktopPreferencesSnapshot> {
        let mut state = self
            .state
            .lock()
            .map_err(|error| unavailable("lock preferences", error))?;
        if state.revision == 0 {
            tracing::warn!("desktop preferences save refused before loading");
            eyre::bail!("Open preferences before saving.");
        }
        if expected_revision != state.revision {
            tracing::warn!(
                expected_revision,
                current_revision = state.revision,
                "desktop preferences save refused for a stale revision"
            );
            eyre::bail!(SETTINGS_CHANGED);
        }
        let document = state.document.as_ref().ok_or_else(|| {
            tracing::warn!("desktop preferences have no captured personal settings location");
            eyre::eyre!(SETTINGS_LOCATION_UNAVAILABLE)
        })?;
        let revision = next_revision(state.revision)?;
        let saved = document
            .save(&[
                ("theme", "default", preferences.theme.as_str().into()),
                (
                    "accessibility",
                    "reduce_motion",
                    preferences.reduce_motion.into(),
                ),
            ])
            .map_err(|error| config_error(PreferenceOperation::Save, error))?;
        state.document = Some(saved);
        state.revision = revision;
        Ok(state.snapshot())
    }
}

impl PreferencesState {
    fn snapshot(&self) -> DesktopPreferencesSnapshot {
        DesktopPreferencesSnapshot {
            preferences: self
                .document
                .as_ref()
                .map(|document| DesktopPreferences::from(document.config()))
                .unwrap_or_default(),
            revision: self.revision,
        }
    }
}

const SETTINGS_CHANGED: &str =
    "Settings changed elsewhere. Choose Retry preferences to save your choices.";
const SETTINGS_LOCATION_UNAVAILABLE: &str =
    "Magistr could not find your settings folder. Restart Magistr and try again.";

#[derive(Debug, Clone, Copy)]
enum PreferenceOperation {
    Read,
    Save,
}

/// Diagnostics stay in tracing; even an expanded report contains only public copy.
fn config_error(operation: PreferenceOperation, error: UserConfigError) -> eyre::Report {
    tracing::warn!(?operation, %error, "desktop preferences failed");
    let message = match error {
        UserConfigError::Conflict(_) => SETTINGS_CHANGED,
        UserConfigError::Busy(_) => "Settings are being saved. Try again in a moment.",
        UserConfigError::Parse(_, _) => {
            "Your settings file contains an error. Correct it, then choose Retry preferences."
        }
        UserConfigError::NoHome(_) => SETTINGS_LOCATION_UNAVAILABLE,
        UserConfigError::Io(_, _) => match operation {
            PreferenceOperation::Read => {
                "Could not read your saved preferences. Check that your settings folder is available, then try again."
            }
            PreferenceOperation::Save => {
                "Could not save your preferences. Check that you can save files in your settings folder, then try again."
            }
        },
    };
    eyre::eyre!(message)
}

fn unavailable(operation: &str, error: impl std::fmt::Display) -> eyre::Report {
    tracing::error!(operation, %error, "desktop preferences unavailable");
    eyre::eyre!("Preferences are unavailable. Restart Magistr and try again.")
}

fn next_revision(revision: u64) -> eyre::Result<u64> {
    revision
        .checked_add(1)
        .ok_or_else(|| unavailable("advance preferences revision", "revision exhausted"))
}

#[cfg(test)]
#[path = "desktop_preferences.tests.rs"]
mod tests;
