//! Native file gestures grant only the selected conversation access to chosen paths.
use crate::commands::DesktopState;
use magistr_application::sessions::{
    MAX_ATTACHMENT_BYTES, MAX_ATTACHMENT_TOTAL_BYTES, MAX_ATTACHMENTS,
};
use magistr_domain::identity::SessionAddress;
use serde::Serialize;
use std::{path::PathBuf, sync::Mutex};
use tauri::{State, ipc::Channel};

#[derive(Clone, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub(crate) enum DropNotice {
    Selected {
        selection: u64,
        address: SessionAddress,
        stream: u64,
        count: usize,
    },
    Error {
        message: String,
    },
}

struct Grant {
    selection: u64,
    address: SessionAddress,
    stream: u64,
    paths: Vec<PathBuf>,
}

#[derive(Default)]
struct Drops {
    next: u64,
    pending: Option<Grant>,
    channel: Option<Channel<DropNotice>>,
}

#[derive(Default)]
pub(crate) struct NativeAttachments {
    drops: Mutex<Drops>,
}

impl NativeAttachments {
    pub(crate) fn subscribe(&self, channel: Channel<DropNotice>) -> Result<(), String> {
        let mut drops = self.drops.lock().map_err(|_| unavailable())?;
        drops.pending = None;
        drops.channel = Some(channel);
        Ok(())
    }

    pub(crate) fn dropped(&self, target: Option<(u64, SessionAddress)>, paths: Vec<PathBuf>) {
        let Ok(mut drops) = self.drops.lock() else {
            return;
        };
        drops.pending = None;
        let notice = match target {
            Some((stream, address)) if !paths.is_empty() && paths.len() <= MAX_ATTACHMENTS => {
                drops.next += 1;
                let selection = drops.next;
                let notice = DropNotice::Selected {
                    selection,
                    address: address.clone(),
                    stream,
                    count: paths.len(),
                };
                drops.pending = Some(Grant {
                    selection,
                    address,
                    stream,
                    paths,
                });
                notice
            }
            Some(_) => DropNotice::Error {
                message: format!("Add up to {MAX_ATTACHMENTS} files to one message."),
            },
            None => DropNotice::Error {
                message: "Open a conversation before adding files.".into(),
            },
        };
        let delivered = drops
            .channel
            .as_ref()
            .is_some_and(|channel| channel.send(notice).is_ok());
        tracing::debug!(
            stream = ?drops.pending.as_ref().map(|grant| grant.stream),
            delivered,
            "native file drop grant delivery"
        );
        if !delivered {
            drops.pending = None;
        }
    }

    fn take(
        &self,
        selection: u64,
        address: &SessionAddress,
        stream: u64,
    ) -> Result<Vec<PathBuf>, String> {
        let mut drops = self.drops.lock().map_err(|_| unavailable())?;
        let grant = drops.pending.as_ref().ok_or_else(expired)?;
        if grant.selection != selection || &grant.address != address || grant.stream != stream {
            return Err(expired());
        }
        Ok(drops.pending.take().ok_or_else(expired)?.paths)
    }
}

fn expired() -> String {
    "The selected conversation changed. Drop the files again.".into()
}
fn unavailable() -> String {
    "File selection is unavailable. Try again.".into()
}

#[derive(Serialize)]
pub(crate) struct Limits {
    file_bytes: usize,
    total_bytes: usize,
    count: usize,
}

impl Default for Limits {
    fn default() -> Self {
        Self {
            file_bytes: MAX_ATTACHMENT_BYTES,
            total_bytes: MAX_ATTACHMENT_TOTAL_BYTES,
            count: MAX_ATTACHMENTS,
        }
    }
}

#[tauri::command]
pub(crate) fn subscribe_file_drops(
    channel: Channel<DropNotice>,
    state: State<'_, DesktopState>,
) -> Result<(), String> {
    state.attachments.subscribe(channel)
}

#[tauri::command]
pub(crate) async fn pick_attachments(
    window: tauri::WebviewWindow,
    address: SessionAddress,
    state: State<'_, DesktopState>,
) -> Result<(), String> {
    use tauri_plugin_dialog::DialogExt as _;
    let (reply, result) = tokio::sync::oneshot::channel();
    window
        .dialog()
        .file()
        .set_parent(&window)
        .set_title("Add files")
        .set_directory(&address.workspace)
        .pick_files(move |paths| {
            let paths = paths
                .map(|paths| {
                    paths
                        .into_iter()
                        .map(|path| path.into_path().map_err(|error| error.to_string()))
                        .collect::<Result<Vec<_>, _>>()
                })
                .transpose();
            let _ = reply.send(paths);
        });
    let paths = result.await.map_err(|_| unavailable())??;
    if let Some(paths) = paths {
        state.client.attach_files(&address, &paths).await?;
    }
    Ok(())
}

#[tauri::command]
pub(crate) async fn attach_dropped_files(
    address: SessionAddress,
    stream: u64,
    selection: u64,
    state: State<'_, DesktopState>,
) -> Result<(), String> {
    let paths = state.attachments.take(selection, &address, stream)?;
    let count = paths.len();
    tracing::debug!(count, stream, "native file drop grant consumed");
    let result = state.client.attach_files(&address, &paths).await;
    tracing::debug!(
        count,
        stream,
        accepted = result.is_ok(),
        "native file drop staging finished"
    );
    result
}

#[tauri::command]
pub(crate) async fn attach_clipboard_file(
    address: SessionAddress,
    name: String,
    bytes: Vec<u8>,
    state: State<'_, DesktopState>,
) -> Result<(), String> {
    state.client.attach_bytes(&address, name, bytes).await
}

#[tauri::command]
pub(crate) async fn remove_attachment(
    address: SessionAddress,
    id: String,
    state: State<'_, DesktopState>,
) -> Result<(), String> {
    state.client.remove_attachment(&address, id).await
}

#[cfg(test)]
#[path = "attachments.tests.rs"]
mod tests;
