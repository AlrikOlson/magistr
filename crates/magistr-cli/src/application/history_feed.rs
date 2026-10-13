//! Current recent-project projections over the session owner's history store.
//! SQLite data versions detect commits from every process. Only changed data is
//! reread, off the UI thread; a refused bounded delivery retains the latest value.
use crate::config::inputs::ProjectInputs;
use magistr_cockpit::{
    cockpit::{GalaxyData, Msg},
    job::{JobId, JobOutput},
};
use magistr_cockpit_runtime::channel::Inbox;
use std::{path::PathBuf, time::Duration};
use tokio::task::JoinHandle;

#[derive(Default)]
struct Projection {
    conn: Option<rusqlite::Connection>,
    version: Option<i64>,
    initialized: bool,
}

impl Projection {
    fn read(&mut self, root: &std::path::Path, inputs: &ProjectInputs) -> Option<GalaxyData> {
        let first = !std::mem::replace(&mut self.initialized, true);
        if self.conn.is_none() {
            self.conn = crate::session::storage::sessions_db_path(inputs)
                .filter(|path| path.is_file())
                .and_then(|path| crate::session::storage::open_db(&path).ok());
        }
        let version = self.conn.as_ref().and_then(|conn| {
            conn.pragma_query_value(None, "data_version", |row| row.get::<_, i64>(0))
                .ok()
        });
        if !first && version == self.version {
            return None;
        }
        self.version = version;
        Some(crate::commands::galaxy_data::collect(root, inputs))
    }
}

pub(super) fn follow(root: PathBuf, inputs: ProjectInputs, inbox: Inbox) -> JoinHandle<()> {
    tokio::spawn(async move {
        let mut tick = tokio::time::interval(Duration::from_secs(1));
        tick.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
        let mut projection = Projection::default();
        let mut pending = None;
        let mut delivered = None;
        loop {
            tokio::select! {
                () = inbox.closed() => return,
                _ = tick.tick() => {}
            }
            let root = root.clone();
            let inputs = inputs.clone();
            let Ok((reader, update)) = tokio::task::spawn_blocking(move || {
                let update = projection.read(&root, &inputs);
                (projection, update)
            })
            .await
            else {
                return;
            };
            projection = reader;
            if let Some(update) = update {
                pending = Some(update);
            }
            if let Some(data) = pending.as_ref() {
                if delivered.as_ref() == Some(data) {
                    pending = None;
                } else if inbox.send(Msg::Job {
                    id: JobId::new(u64::MAX),
                    output: JobOutput::Galaxy(Box::new(data.clone())),
                }) {
                    delivered = pending.take();
                }
            }
        }
    })
}

#[cfg(test)]
#[path = "history_feed.tests.rs"]
mod tests;
