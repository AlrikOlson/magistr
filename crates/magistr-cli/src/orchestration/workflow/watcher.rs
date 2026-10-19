//! File system watcher for detecting changes during agent sessions.
//!
//! Uses polling-based approach (no `notify` crate dependency) to track
//! file creation, modification, and deletion within a project directory.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;

/// Type of file change detected.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FileChange {
    /// File was created (not previously tracked).
    Created,
    /// File was modified (mtime changed).
    Modified,
    /// File was deleted (previously tracked, now missing).
    Deleted,
}

/// A file change event.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileEvent {
    /// Path relative to the watched directory.
    pub path: String,
    /// Type of change.
    pub change: FileChange,
}

/// Directories to skip during scanning.
const SKIP_DIRS: &[&str] = &[
    ".git",
    "target",
    "node_modules",
    ".claude",
    // magistr's own state (journal.db-shm, workspace.db-shm, watcher.json)
    ".magistr/workspace",
    "dist",
    "build",
    "__pycache__",
    ".mypy_cache",
    ".pytest_cache",
    ".next",
    ".nuxt",
];

/// Scan a directory recursively and collect file paths with their modified times.
fn scan_dir(root: &Path) -> HashMap<PathBuf, SystemTime> {
    let mut files = HashMap::new();
    scan_dir_recursive(root, root, &mut files);
    files
}

fn scan_dir_recursive(root: &Path, dir: &Path, files: &mut HashMap<PathBuf, SystemTime>) {
    let entries = match std::fs::read_dir(dir) {
        Ok(entries) => entries,
        Err(_) => return,
    };

    for entry in entries.flatten() {
        let path = entry.path();

        if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
            if path.is_dir() {
                if SKIP_DIRS.contains(&name) {
                    continue;
                }
                scan_dir_recursive(root, &path, files);
            } else if let Ok(meta) = path.metadata()
                && let Ok(mtime) = meta.modified()
            {
                let relative = path.strip_prefix(root).unwrap_or(&path).to_path_buf();
                files.insert(relative, mtime);
            }
        }
    }
}

/// Compare two snapshots and produce change events.
fn diff_snapshots(
    old: &HashMap<PathBuf, SystemTime>,
    new: &HashMap<PathBuf, SystemTime>,
) -> Vec<FileEvent> {
    let mut events = Vec::new();

    // Check for created and modified files
    for (path, new_mtime) in new {
        match old.get(path) {
            None => events.push(FileEvent {
                path: path.display().to_string(),
                change: FileChange::Created,
            }),
            Some(old_mtime) if old_mtime != new_mtime => events.push(FileEvent {
                path: path.display().to_string(),
                change: FileChange::Modified,
            }),
            _ => {}
        }
    }

    // Check for deleted files
    for path in old.keys() {
        if !new.contains_key(path) {
            events.push(FileEvent {
                path: path.display().to_string(),
                change: FileChange::Deleted,
            });
        }
    }

    events
}

// ── FileWatcher trait ───────────────────────────────────────────────────────

/// Abstraction for file system watching.
///
/// Production uses `PollingFileWatcher` (spawns a background polling task).
/// Tests use `MockFileWatcher` to inject events without filesystem access.
pub trait FileWatcher: Send {
    /// Get the event receiver for file change events.
    fn event_rx(&mut self) -> &mut mpsc::UnboundedReceiver<FileEvent>;
}

/// Production file watcher using polling-based change detection.
pub struct PollingFileWatcher {
    /// Cancellation token — cancel to stop the watcher task.
    cancel: CancellationToken,
    /// Receiver for file change events from the background poller.
    rx: mpsc::UnboundedReceiver<FileEvent>,
    /// Handle to the background polling task.
    _task: tokio::task::JoinHandle<()>,
}

impl PollingFileWatcher {
    /// Start a new polling file watcher for the given directory.
    #[must_use]
    pub fn new(root: PathBuf) -> Self {
        let cancel = CancellationToken::new();
        let (event_tx, rx) = mpsc::unbounded_channel();
        let task = spawn_watcher(root, event_tx, cancel.clone());
        Self {
            cancel,
            rx,
            _task: task,
        }
    }
}

impl Drop for PollingFileWatcher {
    fn drop(&mut self) {
        self.cancel.cancel();
    }
}

impl FileWatcher for PollingFileWatcher {
    fn event_rx(&mut self) -> &mut mpsc::UnboundedReceiver<FileEvent> {
        &mut self.rx
    }
}

/// Test file watcher backed by a channel for injecting events.
pub struct MockFileWatcher {
    /// Receiver for injected file events.
    rx: mpsc::UnboundedReceiver<FileEvent>,
}

impl MockFileWatcher {
    /// Create a new mock file watcher.
    ///
    /// Returns `(watcher, event_sender)`. Push `FileEvent`s through the sender
    /// and they'll appear via `event_rx()`.
    pub fn new() -> (Self, mpsc::UnboundedSender<FileEvent>) {
        let (tx, rx) = mpsc::unbounded_channel();
        (Self { rx }, tx)
    }
}

impl FileWatcher for MockFileWatcher {
    fn event_rx(&mut self) -> &mut mpsc::UnboundedReceiver<FileEvent> {
        &mut self.rx
    }
}

/// Poll interval for the file watcher.
const POLL_INTERVAL: std::time::Duration = std::time::Duration::from_millis(500);

/// Spawn the polling loop with a `CancellationToken` for graceful shutdown.
fn spawn_watcher(
    root: PathBuf,
    event_tx: mpsc::UnboundedSender<FileEvent>,
    cancel: CancellationToken,
) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        let mut snapshot = scan_dir(&root);

        loop {
            tokio::select! {
                _ = tokio::time::sleep(POLL_INTERVAL) => {
                    let new_snapshot = scan_dir(&root);
                    let events = diff_snapshots(&snapshot, &new_snapshot);

                    for event in events {
                        if event_tx.send(event).is_err() {
                            return; // Receiver dropped
                        }
                    }

                    snapshot = new_snapshot;
                }
                _ = cancel.cancelled() => {
                    return;
                }
            }
        }
    })
}

/// Start a file watcher task that polls for changes and sends events through a channel.
///
/// Returns `(cancellation_token, event_receiver)`. Cancel the token to stop the watcher.
pub fn start_watcher(root: PathBuf) -> (CancellationToken, mpsc::UnboundedReceiver<FileEvent>) {
    let cancel = CancellationToken::new();
    let (event_tx, event_rx) = mpsc::unbounded_channel();
    spawn_watcher(root, event_tx, cancel.clone());

    (cancel, event_rx)
}

#[cfg(test)]
#[path = "watcher.tests.rs"]
mod tests;
