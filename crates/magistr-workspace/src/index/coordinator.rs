use super::{
    IndexError,
    cache::IndexCache,
    selection::Selection,
    shared::Shared,
    watch::{IndexWatcher, NetworkDetection, WatchMode},
};
use crate::{
    contracts::Freshness,
    worker::{WorkerRequest, WorkerResponse},
};
use std::{
    sync::{Arc, atomic::Ordering},
    time::Duration,
};

use crate::diagnostics::now_unix_ms;

/// How the watcher is started: the root, the requested mode, whether the root
/// is on a network filesystem, and the polling interval.
pub(super) type StartWatcher = Box<
    dyn FnOnce(&std::path::Path, WatchMode, bool, Duration) -> notify::Result<IndexWatcher> + Send,
>;

pub(super) struct Coordinator;
impl Coordinator {
    /// The cache opens first and is usable at once. Starting the operating
    /// system's watcher is a separate blocking step beside the first scan: it
    /// can take half a second alone and seconds when many start together, and
    /// nothing here waits for it — not the first scan, not a stop. Once it is
    /// installed the tree is scanned again, so a change made while it was
    /// starting is not missed; until then the index never reads as warm.
    pub async fn run(shared: Arc<Shared>, watch: StartWatcher) {
        let init = shared.clone();
        let opened = tokio::task::spawn_blocking(move || {
            let network = NetworkDetection::detect(&init.root, init.options.network_override.as_deref());
            let cache = IndexCache::open(&init.state_dir, network.journal_mode())?;
            let mut notes = vec![format!("Filesystem detection: {}; requested journal mode {}.", network.method, network.journal_mode())];
            if let Some(reason) = &cache.rebuild_reason { notes.push(format!("Derived cache rebuilt: {reason}")); }
            notes.push(format!("Applied journal mode: {}.", cache.journal_mode));
            if init.options.watch_mode == WatchMode::Off { notes.push("Watcher disabled; queries revalidate candidates and may miss newly added files until refresh.".into()); }
            let generation = cache.generation()?;
            *init.cache.lock().unwrap_or_else(|e| e.into_inner()) = Some(cache);
            let mut status = init.status.lock().unwrap_or_else(|e| e.into_inner());
            status.generation = generation;
            status.notes.extend(notes);
            Ok::<_, IndexError>(network.network_fs)
        }).await;
        let network_fs = match opened {
            Ok(Ok(network_fs)) => network_fs,
            result => {
                let message = match result {
                    Ok(Err(e)) => e.to_string(),
                    Err(e) => e.to_string(),
                    _ => unreachable!(),
                };
                let mut status = shared.status.lock().unwrap_or_else(|e| e.into_inner());
                status.last_error = Some(message);
                return;
            }
        };
        let starting = shared.clone();
        let mut watcher_start = Some(tokio::task::spawn_blocking(move || {
            watch(
                &starting.root,
                starting.options.watch_mode,
                network_fs,
                starting.options.poll_interval,
            )
        }));
        let mut requested = 0;
        let mut epoch = u64::MAX;
        let mut retry = false;
        let mut last_attempt = tokio::time::Instant::now() - Duration::from_secs(60);
        let mut last_scan = last_attempt;
        loop {
            if shared.stopped.is_cancelled() {
                break;
            }
            let next_request = shared.requested.load(Ordering::Acquire);
            let next_epoch = shared
                .watcher
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .as_ref()
                .map_or(0, IndexWatcher::epoch);
            let periodic = shared.options.watch_mode != WatchMode::Off
                && last_scan.elapsed() >= Duration::from_secs(30);
            if (requested != next_request || epoch != next_epoch || retry || periodic)
                && last_attempt.elapsed() >= shared.options.poll_interval
            {
                requested = next_request;
                epoch = next_epoch;
                last_attempt = tokio::time::Instant::now();
                {
                    let mut status = shared.status.lock().unwrap_or_else(|e| e.into_inner());
                    status.building = true;
                    status.freshness = Freshness::Warming;
                    status.last_error = None;
                    status.parsed = 0;
                    status.reused = 0;
                    status.build_started_at_ms = Some(now_unix_ms());
                    status.build_updated_at_ms = status.build_started_at_ms;
                }
                let result = Self::build(shared.clone()).await;
                last_scan = tokio::time::Instant::now();
                retry = result.is_err();
                let watcher_guard = shared.watcher.lock().unwrap_or_else(|e| e.into_inner());
                if let Some(watcher) = watcher_guard.as_ref() {
                    watcher.acknowledge(epoch);
                }
                let watcher_ok = watcher_guard.as_ref().is_some_and(|w| {
                    let snapshot = w.snapshot();
                    snapshot.mode != WatchMode::Off
                        && !snapshot.pending
                        && snapshot.last_error.is_none()
                        && !snapshot.stopped
                });
                drop(watcher_guard);
                let mut status = shared.status.lock().unwrap_or_else(|e| e.into_inner());
                let current = shared.requested.load(Ordering::Acquire) == requested;
                status.refresh_pending = !current;
                status.building = false;
                status.build_updated_at_ms = Some(now_unix_ms());
                status.freshness = if result.is_ok() && watcher_ok && current {
                    Freshness::Warm
                } else {
                    Freshness::Warming
                };
                if let Err(error) = result {
                    status.last_error = Some(error.to_string());
                }
            }
            tokio::select! {
                _ = shared.stopped.cancelled() => break,
                started = async { watcher_start.as_mut().expect("guarded").await }, if watcher_start.is_some() => {
                    watcher_start = None;
                    Self::install(&shared, started);
                    // What changed while the watcher was starting is only
                    // found by looking again.
                    shared.request_refresh();
                }
                _ = tokio::time::sleep(if retry { Duration::from_secs(1) } else { shared.options.poll_interval }) => {}
            }
        }
        if let Some(start) = watcher_start.take() {
            // The operating system call cannot be interrupted. Its result is
            // torn down by its own step when it returns; a stop does not wait.
            tokio::spawn(async move {
                if let Ok(Ok(mut watcher)) = start.await {
                    let _ = tokio::task::spawn_blocking(move || watcher.shutdown()).await;
                }
            });
        }
        let watcher = shared
            .watcher
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .take();
        if let Some(mut watcher) = watcher {
            let _ = tokio::task::spawn_blocking(move || watcher.shutdown()).await;
        }
        // Close the cache after every owned scanner/worker call has settled.
        shared
            .cache
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .take();
        let mut status = shared.status.lock().unwrap_or_else(|e| e.into_inner());
        status.building = false;
        status.freshness = Freshness::Cold;
    }
    /// Take the started watcher, or say why there is none.
    fn install(
        shared: &Shared,
        started: Result<notify::Result<IndexWatcher>, tokio::task::JoinError>,
    ) {
        let failure = match started {
            Ok(Ok(watcher)) => {
                *shared.watcher.lock().unwrap_or_else(|e| e.into_inner()) = Some(watcher);
                return;
            }
            Ok(Err(error)) => error.to_string(),
            Err(error) => error.to_string(),
        };
        shared
            .status
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .notes
            .push(format!(
                "Watcher unavailable: {failure}; anchors require revalidation."
            ));
    }
    async fn build(shared: Arc<Shared>) -> Result<(), IndexError> {
        let scan = shared.clone();
        let selection = tokio::task::spawn_blocking(move || {
            Selection::scan(
                &scan.root,
                &scan.state_dir,
                scan.options.max_files,
                &scan.stopped,
            )
        })
        .await
        .map_err(|e| IndexError::Unavailable(e.to_string()))??;
        let mut failure = selection
            .truncated
            .then(|| "file admission limit reached; cache is incomplete".to_owned());
        for path in &selection.paths {
            if shared.stopped.is_cancelled() {
                return Err(IndexError::Unavailable("index stopped".into()));
            }
            let read = shared.clone();
            let path = path.clone();
            let loaded = tokio::task::spawn_blocking(move || {
                let observed = read.observe(&path)?;
                let cache = read.cache.lock().unwrap_or_else(|e| e.into_inner());
                let previous = cache
                    .as_ref()
                    .ok_or_else(|| IndexError::Unavailable("cache closed".into()))?
                    .metadata(&path)?;
                Ok::<_, IndexError>((observed, previous))
            })
            .await
            .map_err(|e| IndexError::Unavailable(e.to_string()))?;
            let (mut observed, previous) = match loaded {
                Ok(loaded) => loaded,
                Err(error) => {
                    failure = Some(error.to_string());
                    continue;
                }
            };
            if previous.as_ref().is_some_and(|old| observed.same(old)) {
                shared
                    .status
                    .lock()
                    .unwrap_or_else(|e| e.into_inner())
                    .reused += 1;
                continue;
            }
            if let (Some(language), Some(source)) =
                (&observed.file.language, &observed.file.content)
            {
                let response = shared
                    .worker
                    .call(
                        WorkerRequest::Extract {
                            path: observed.file.path.clone(),
                            language: language.clone(),
                            source: source.clone(),
                        },
                        shared.options.worker_timeout,
                        &shared.stopped,
                    )
                    .await;
                match response {
                    Ok(WorkerResponse::Extract {
                        symbols,
                        imports,
                        test_ranges,
                    }) => {
                        observed.file.symbols = symbols;
                        observed.file.imports = imports;
                        observed.file.test_ranges = test_ranges;
                        shared
                            .status
                            .lock()
                            .unwrap_or_else(|e| e.into_inner())
                            .parsed += 1;
                    }
                    result => {
                        failure = Some(format!(
                            "{}: extraction unavailable: {result:?}",
                            observed.file.path
                        ));
                        continue;
                    }
                }
            }
            let commit = shared.clone();
            tokio::task::spawn_blocking(move || {
                let live = commit.observe(&observed.file.path)?;
                if !observed.file.content_hash.is_empty() && !live.same(&observed.file) {
                    return Err(IndexError::Unavailable(
                        "file changed during worker extraction".into(),
                    ));
                }
                let mut cache = commit.cache.lock().unwrap_or_else(|e| e.into_inner());
                let cache = cache
                    .as_mut()
                    .ok_or_else(|| IndexError::Unavailable("cache closed".into()))?;
                cache.update(&observed.file)?;
                Ok::<_, IndexError>(())
            })
            .await
            .map_err(|e| IndexError::Unavailable(e.to_string()))??;
            tokio::task::yield_now().await;
        }
        let finish = shared.clone();
        tokio::task::spawn_blocking(move || {
            let mut cache = finish.cache.lock().unwrap_or_else(|e| e.into_inner());
            let cache = cache
                .as_mut()
                .ok_or_else(|| IndexError::Unavailable("cache closed".into()))?;
            // A partial walk cannot establish absence. Keep previous rows for
            // later reconciliation; queries still revalidate each candidate.
            if !selection.truncated {
                let mut after = None;
                loop {
                    let files = cache.files_page(after.as_deref(), 256)?;
                    if files.is_empty() {
                        break;
                    }
                    after = files.last().map(|f| f.path.clone());
                    for file in files {
                        if !selection.paths.contains(&file.path) {
                            cache.remove(&file.path)?;
                        }
                    }
                }
            }
            if failure.is_none() {
                cache.finish_build()?;
            }
            let generation = cache.generation()?;
            let files = cache.counts()?.files as usize;
            let mut status = finish.status.lock().unwrap_or_else(|e| e.into_inner());
            status.generation = generation;
            status.files = files;
            failure.map_or(Ok(()), |error| Err(IndexError::Unavailable(error)))
        })
        .await
        .map_err(|e| IndexError::Unavailable(e.to_string()))?
    }
}

#[cfg(test)]
#[path = "coordinator.tests.rs"]
mod tests;
