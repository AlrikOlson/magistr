//! Saved and resident draft commands delegate to application-owned history.
use crate::{commands::DesktopState, wire::Attachment};
use magistr::application::{ConversationTarget, DraftStatus};
use serde::Serialize;
use tauri::State;

#[derive(Serialize)]
pub(crate) struct SavedDraft {
    text: String,
    attachments: Vec<Attachment>,
    status: DraftStatus,
}
#[tauri::command]
pub(crate) async fn saved_draft(
    id: String,
    state: State<'_, DesktopState>,
) -> Result<SavedDraft, String> {
    let inputs = state.inputs.clone();
    let draft =
        tokio::task::spawn_blocking(move || magistr::application::saved_draft(&inputs, &id))
            .await
            .map_err(|error| {
                tracing::warn!(%error, "could not finish reading saved draft");
                "Your saved message could not be read. Try again.".to_owned()
            })??;
    Ok(SavedDraft {
        text: draft.text,
        attachments: draft
            .attachments
            .iter()
            .map(|file| Attachment::from_staged(file, &[]))
            .collect(),
        status: draft.status,
    })
}
#[tauri::command]
pub(crate) async fn discard_draft(
    target: ConversationTarget,
    expected_revision: u64,
    state: State<'_, DesktopState>,
) -> Result<DraftStatus, String> {
    state.client.discard_draft(&target, expected_revision).await
}
#[tauri::command]
pub(crate) async fn save_saved_draft(
    id: String,
    expected_revision: u64,
    text: String,
    state: State<'_, DesktopState>,
) -> Result<DraftStatus, String> {
    state
        .client
        .save_saved_draft(state.inputs.clone(), id, expected_revision, text)
        .await
}
#[tauri::command]
pub(crate) async fn discard_saved_draft(
    id: String,
    expected_revision: u64,
    state: State<'_, DesktopState>,
) -> Result<DraftStatus, String> {
    state
        .client
        .discard_saved_draft(state.inputs.clone(), id, expected_revision)
        .await
}
#[tauri::command]
pub(crate) async fn remove_saved_attachment(
    id: String,
    expected_revision: u64,
    attachment_id: String,
    state: State<'_, DesktopState>,
) -> Result<DraftStatus, String> {
    state
        .client
        .remove_saved_attachment(state.inputs.clone(), id, expected_revision, attachment_id)
        .await
}
