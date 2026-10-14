//! Preferences translate the captured configuration owner's typed read/write contract.
use crate::commands::DesktopState;
use magistr::application::{DesktopPreferences, DesktopPreferencesSnapshot};
use tauri::State;

#[tauri::command]
pub(crate) async fn preferences(
    state: State<'_, DesktopState>,
) -> Result<DesktopPreferencesSnapshot, String> {
    state
        .preferences
        .load()
        .await
        .map_err(|error| error.to_string())
}
#[tauri::command]
pub(crate) async fn save_preferences(
    expected_revision: u64,
    preferences: DesktopPreferences,
    state: State<'_, DesktopState>,
) -> Result<DesktopPreferencesSnapshot, String> {
    state
        .preferences
        .save(expected_revision, preferences)
        .await
        .map_err(|error| error.to_string())
}
