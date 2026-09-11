use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, SyncSender};
use std::thread::{self, JoinHandle};
use std::time::Duration;

use notify::{RecommendedWatcher, RecursiveMode, Watcher};

use super::change_state::ChangeState;
use super::polling_digest::PollingDigest;
use super::{WatchMode, WatchSnapshot};

/// Native events or bounded content polling feeding one coalesced dirty epoch.
/// Call `acknowledge` only after consuming the captured epoch. A concurrent
/// event remains pending. Dropping the adapter stops and joins its own poller.
pub struct IndexWatcher {
    state: Arc<ChangeState>,
    native: Option<RecommendedWatcher>,
    stop: Arc<AtomicBool>,
    wake: Option<SyncSender<()>>,
    poller: Option<JoinHandle<()>>,
}

impl IndexWatcher {
    pub fn start(
        root: &Path,
        mode: WatchMode,
        network: bool,
        interval: Duration,
    ) -> notify::Result<Self> {
        let mut watcher = Self {
            state: Arc::new(ChangeState::new(mode, mode.effective(network))),
            native: None,
            stop: Arc::new(AtomicBool::new(false)),
            wake: None,
            poller: None,
        };
        let effective = mode.effective(network);
        if effective == WatchMode::Off {
            return Ok(watcher);
        }
        let root = root.canonicalize().map_err(notify::Error::io)?;
        if !root.is_dir() {
            return Err(notify::Error::generic("watch root is not a directory"));
        }
        if effective == WatchMode::Native {
            let state = Arc::clone(&watcher.state);
            let callback_root = root.clone();
            let mut native = RecommendedWatcher::new(
                move |event| state.receive(&callback_root, event),
                notify::Config::default().with_follow_symlinks(false),
            )?;
            native.watch(&root, RecursiveMode::Recursive)?;
            watcher.native = Some(native);
        } else {
            // notify's PollWatcher does not track permissions and does not join
            // its detached thread. The owned poller gives both contracts here.
            let mut previous =
                PollingDigest::capture(&root, &watcher.stop).map_err(notify::Error::io)?;
            let state = Arc::clone(&watcher.state);
            let stop = Arc::clone(&watcher.stop);
            let (wake, receiver) = mpsc::sync_channel(1);
            watcher.wake = Some(wake);
            watcher.poller = Some(
                thread::Builder::new()
                    .name("magistr-index-poll".into())
                    .spawn(move || {
                        let interval = interval.max(Duration::from_millis(10));
                        while receiver.recv_timeout(interval)
                            == Err(mpsc::RecvTimeoutError::Timeout)
                        {
                            if stop.load(Ordering::Acquire) {
                                break;
                            }
                            match PollingDigest::capture(&root, &stop) {
                                Ok(next) if next != previous => {
                                    previous = next;
                                    state.changed(false, None);
                                }
                                Ok(_) => state.recovered(),
                                Err(error) if !stop.load(Ordering::Acquire) => {
                                    state.changed(true, Some(&error.to_string()));
                                }
                                _ => {}
                            }
                        }
                    })
                    .map_err(notify::Error::io)?,
            );
        }
        Ok(watcher)
    }

    #[must_use]
    pub fn epoch(&self) -> u64 {
        self.state.epoch()
    }

    pub fn acknowledge(&self, epoch: u64) {
        self.state.acknowledge(epoch);
    }

    #[must_use]
    pub fn snapshot(&self) -> WatchSnapshot {
        self.state.snapshot()
    }

    pub fn shutdown(&mut self) {
        self.state.stop();
        self.stop.store(true, Ordering::Release);
        self.native.take();
        if let Some(wake) = self.wake.take() {
            let _ = wake.try_send(());
        }
        if let Some(poller) = self.poller.take() {
            let _ = poller.join();
        }
    }
}

impl Drop for IndexWatcher {
    fn drop(&mut self) {
        self.shutdown();
    }
}

#[cfg(test)]
#[path = "index_watcher.tests.rs"]
mod tests;
