//! Native exit awaits editable views before the session owner settles.
use std::sync::{
    Mutex,
    atomic::{AtomicBool, Ordering},
};
use tauri::ipc::Channel;

#[derive(Default)]
pub(crate) struct Exit {
    channel: Mutex<Option<Channel<()>>>,
    ready: AtomicBool,
}
impl Exit {
    pub(crate) fn request(&self) -> bool {
        if self.ready.swap(false, Ordering::AcqRel) {
            return false;
        }
        let channel = self.channel.lock().expect("exit channel lock");
        if let Some(channel) = channel.as_ref() {
            return channel.send(()).is_ok();
        }
        false
    }
}
#[tauri::command]
pub(crate) fn subscribe_exit(
    channel: Channel<()>,
    state: tauri::State<'_, crate::commands::DesktopState>,
) {
    *state.exit.channel.lock().expect("exit channel lock") = Some(channel);
}
#[tauri::command]
pub(crate) fn finish_exit(
    app: tauri::AppHandle,
    state: tauri::State<'_, crate::commands::DesktopState>,
) {
    state.exit.ready.store(true, Ordering::Release);
    app.exit(0);
}
