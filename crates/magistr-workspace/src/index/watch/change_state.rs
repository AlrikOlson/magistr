use std::path::Path;
use std::sync::Mutex;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Instant;

use notify::{Event, EventKind};

use super::{WatchMode, WatchSnapshot};

/// A single coalescing dirty epoch, with no queued paths or retained events.
pub(super) struct ChangeState {
    epoch: AtomicU64,
    diagnostics: Mutex<Diagnostics>,
}

struct Diagnostics {
    requested_mode: WatchMode,
    mode: WatchMode,
    events: u64,
    rescans: u64,
    failures: u64,
    acknowledged: u64,
    pending_since: Option<Instant>,
    last_error: Option<String>,
    stopped: bool,
}

impl ChangeState {
    pub(super) fn new(requested_mode: WatchMode, mode: WatchMode) -> Self {
        let active = mode != WatchMode::Off;
        Self {
            epoch: AtomicU64::new(u64::from(active)),
            diagnostics: Mutex::new(Diagnostics {
                requested_mode,
                mode,
                events: 0,
                rescans: 0,
                failures: 0,
                acknowledged: 0,
                pending_since: active.then(Instant::now),
                last_error: None,
                stopped: !active,
            }),
        }
    }

    pub(super) fn epoch(&self) -> u64 {
        self.epoch.load(Ordering::Acquire)
    }

    pub(super) fn acknowledge(&self, observed_epoch: u64) {
        let mut state = self
            .diagnostics
            .lock()
            .unwrap_or_else(|poison| poison.into_inner());
        let current = self.epoch();
        state.acknowledged = state.acknowledged.max(observed_epoch.min(current));
        if state.acknowledged == current {
            state.pending_since = None;
        }
    }

    pub(super) fn changed(&self, rescan: bool, error: Option<&str>) {
        let mut state = self
            .diagnostics
            .lock()
            .unwrap_or_else(|poison| poison.into_inner());
        if state.stopped {
            return;
        }
        state.events = state.events.saturating_add(1);
        if rescan {
            state.rescans = state.rescans.saturating_add(1);
        }
        if let Some(error) = error {
            state.failures = state.failures.saturating_add(1);
            state.last_error = Some(error.chars().take(512).collect());
        } else {
            // A healthy native event or changed poll proves the adapter has
            // resumed. Historical failure counts remain visible.
            state.last_error = None;
        }
        state.pending_since.get_or_insert_with(Instant::now);
        // Every epoch writer holds diagnostics; readers only need the release store.
        self.epoch.store(
            self.epoch.load(Ordering::Relaxed).saturating_add(1),
            Ordering::Release,
        );
    }

    pub(super) fn recovered(&self) {
        let mut state = self
            .diagnostics
            .lock()
            .unwrap_or_else(|poison| poison.into_inner());
        if state.stopped || state.last_error.take().is_none() {
            return;
        }
        // Even an unchanged successful poll must notify the coordinator that
        // it can reconcile and leave its previous degraded freshness state.
        state.events = state.events.saturating_add(1);
        state.rescans = state.rescans.saturating_add(1);
        state.pending_since.get_or_insert_with(Instant::now);
        // Every epoch writer holds diagnostics; readers only need the release store.
        self.epoch.store(
            self.epoch.load(Ordering::Relaxed).saturating_add(1),
            Ordering::Release,
        );
    }

    pub(super) fn receive(&self, root: &Path, result: notify::Result<Event>) {
        match result {
            Err(error) => self.changed(true, Some(&error.to_string())),
            Ok(event) if event.need_rescan() => self.changed(true, None),
            Ok(event) if !matches!(event.kind, EventKind::Access(_)) => {
                if event.paths.is_empty() {
                    self.changed(true, None);
                } else if event.paths.iter().any(|path| Self::is_relevant(root, path)) {
                    self.changed(false, None);
                }
            }
            Ok(_) => {}
        }
    }

    pub(super) fn is_relevant(root: &Path, path: &Path) -> bool {
        let Ok(relative) = path.strip_prefix(root) else {
            return false;
        };
        !crate::paths::is_owner_path(relative)
            && !relative.file_name().is_some_and(|name| {
                let name = name.to_string_lossy();
                name.rsplit_once(".magistr-tmp-")
                    .is_some_and(|(_, suffix)| {
                        !suffix.is_empty() && suffix.bytes().all(|byte| byte.is_ascii_digit())
                    })
            })
    }

    pub(super) fn stop(&self) {
        self.diagnostics
            .lock()
            .unwrap_or_else(|poison| poison.into_inner())
            .stopped = true;
    }

    pub(super) fn snapshot(&self) -> WatchSnapshot {
        let state = self
            .diagnostics
            .lock()
            .unwrap_or_else(|poison| poison.into_inner());
        WatchSnapshot {
            requested_mode: state.requested_mode,
            mode: state.mode,
            epoch: self.epoch(),
            events: state.events,
            rescans: state.rescans,
            failures: state.failures,
            pending: self.epoch() > state.acknowledged,
            lag_ms: state.pending_since.map_or(0, |time| {
                u64::try_from(time.elapsed().as_millis()).unwrap_or(u64::MAX)
            }),
            last_error: state.last_error.clone(),
            stopped: state.stopped,
        }
    }
}

#[cfg(test)]
#[path = "change_state.tests.rs"]
mod tests;
