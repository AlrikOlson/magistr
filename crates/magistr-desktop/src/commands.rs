//! Typed desktop commands delegate admission, drafts and session control to the existing application.
use crate::{
    projects::{ProjectPreview, ProjectReviews, SetupApplied},
    streams::Streams,
    wire::Frame,
};
use magistr::application::{RecentProject, SessionClient, WorkingCopies, WorkingCopy};
use magistr::config::inputs::ProjectInputs;
use magistr_cockpit::cockpit::Cmd;
use magistr_domain::identity::SessionAddress;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use tauri::{State, ipc::Channel};

pub(crate) struct DesktopState {
    pub(crate) client: SessionClient,
    pub(crate) streams: Streams,
    pub(crate) directory: Option<PathBuf>,
    pub(crate) projects: std::sync::Arc<ProjectReviews>,
    pub(crate) inputs: ProjectInputs,
    pub(crate) reduce_motion: bool,
}
#[derive(Serialize)]
pub(crate) struct Overview {
    directory: Option<PathBuf>,
    reduce_motion: bool,
    sessions: Vec<SessionInfo>,
    projects: Vec<RecentProject>,
}
#[derive(Serialize)]
struct SessionInfo {
    address: SessionAddress,
    name: String,
    status: String,
}
#[derive(Deserialize)]
#[serde(tag = "action", rename_all = "snake_case")]
pub(crate) enum Action {
    Send { text: String },
    Cancel,
    Answer { id: u64, option_id: String },
}

#[tauri::command]
pub(crate) async fn overview(state: State<'_, DesktopState>) -> Result<Overview, String> {
    let snapshots = state.client.list().await?;
    let active: Vec<_> = snapshots
        .iter()
        .map(|snapshot| PathBuf::from(&snapshot.address.workspace))
        .collect();
    let sessions = snapshots
        .iter()
        .map(|snapshot| {
            let frame = Frame::from_snapshot(0, snapshot, None);
            SessionInfo {
                address: snapshot.address.clone(),
                name: snapshot.seed.name.clone(),
                status: frame.status,
            }
        })
        .collect();
    let inputs = state.inputs.clone();
    let projects = tokio::task::spawn_blocking(move || {
        magistr::application::recent_projects(&inputs, &active)
    })
    .await
    .map_err(|error| error.to_string())?;
    Ok(Overview {
        directory: state.directory.clone(),
        reduce_motion: state.reduce_motion,
        sessions,
        projects,
    })
}
#[tauri::command]
pub(crate) async fn pick_folder(window: tauri::WebviewWindow) -> Result<Option<PathBuf>, String> {
    use tauri_plugin_dialog::DialogExt as _;
    let (reply, result) = tokio::sync::oneshot::channel();
    window
        .dialog()
        .file()
        .set_parent(&window)
        .set_title("Open a project")
        .pick_folder(move |path| {
            let _ = reply.send(
                path.map(|path| path.into_path().map_err(|error| error.to_string()))
                    .transpose(),
            );
        });
    result
        .await
        .map_err(|_| "The folder picker closed unexpectedly. Try again.".to_owned())?
}
#[tauri::command]
pub(crate) async fn preview_project(
    directory: PathBuf,
    review: bool,
    state: State<'_, DesktopState>,
) -> Result<ProjectPreview, String> {
    let projects = std::sync::Arc::clone(&state.projects);
    let inputs = state.inputs.clone();
    tokio::task::spawn_blocking(move || projects.preview(&directory, &inputs, review))
        .await
        .map_err(|error| error.to_string())?
}
#[tauri::command]
pub(crate) async fn apply_setup(
    review: u64,
    approved: Vec<usize>,
    state: State<'_, DesktopState>,
) -> Result<SetupApplied, String> {
    let projects = std::sync::Arc::clone(&state.projects);
    tokio::task::spawn_blocking(move || projects.apply(review, &approved))
        .await
        .map_err(|error| error.to_string())?
}
#[tauri::command]
pub(crate) fn discard_setup(review: u64, state: State<'_, DesktopState>) -> Result<(), String> {
    state.projects.discard(review)
}
#[tauri::command]
pub(crate) async fn working_copies(directory: PathBuf) -> Result<Vec<WorkingCopy>, String> {
    tokio::task::spawn_blocking(move || {
        WorkingCopies::at(&directory).and_then(|copies| copies.list())
    })
    .await
    .map_err(|error| error.to_string())?
    .map_err(|error| error.to_string())
}
#[tauri::command]
pub(crate) async fn open_session(
    directory: PathBuf,
    state: State<'_, DesktopState>,
) -> Result<SessionAddress, String> {
    state.client.open(directory).await
}
#[tauri::command]
pub(crate) async fn subscribe_session(
    address: SessionAddress,
    channel: Channel<Frame>,
    selection: u64,
    state: State<'_, DesktopState>,
) -> Result<u64, String> {
    state
        .streams
        .subscribe(&state.client, &address, channel, selection)
        .await
}
#[tauri::command]
pub(crate) async fn acknowledge(
    stream: u64,
    sequence: u64,
    state: State<'_, DesktopState>,
) -> Result<(), String> {
    state.streams.acknowledge(stream, sequence).await
}
#[tauri::command]
pub(crate) async fn session_action(
    address: SessionAddress,
    action: Action,
    state: State<'_, DesktopState>,
) -> Result<(), String> {
    let command = match action {
        Action::Send { text } => Cmd::Prompt(text),
        Action::Cancel => Cmd::Cancel,
        Action::Answer { id, option_id } => Cmd::Permission { id, option_id },
    };
    state.client.command(&address, command).await
}
#[tauri::command]
pub(crate) async fn save_draft(
    address: SessionAddress,
    text: String,
    state: State<'_, DesktopState>,
) -> Result<(), String> {
    state.client.draft(&address, text).await
}
#[tauri::command]
pub(crate) async fn close_session(
    address: SessionAddress,
    state: State<'_, DesktopState>,
) -> Result<(), String> {
    state.client.close(&address).await
}
