//! Run controls address the shared owner, independent of conversation changes.
#[tauri::command]
pub(crate) async fn control_work(
    target: magistr::application::WorkTarget,
    action: magistr_cockpit::cockpit::LoopCmd,
    state: tauri::State<'_, crate::commands::DesktopState>,
) -> Result<(), String> {
    state.client.control_work(&target, action).await
}
