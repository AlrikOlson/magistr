use super::{
    IndexError, IndexOptions, IndexQuery, IndexStatus,
    cache::{CachedSymbol, IndexCache, IndexedFile},
    shared::Shared,
};
use crate::{contracts::Freshness, worker::WorkerSupervisor};
use std::{
    path::PathBuf,
    sync::{
        Arc, Mutex,
        atomic::{AtomicU64, Ordering},
    },
};
use tokio::task::JoinHandle;
use tokio_util::sync::CancellationToken;

/// One admitted application's derived index. Start is lazy and never opens a cache
/// or waits for a worker on the calling thread. Await shutdown before runtime exit.
pub struct WorkspaceIndex {
    pub(super) shared: Arc<Shared>,
    task: Option<JoinHandle<()>>,
}
impl WorkspaceIndex {
    pub fn start(
        root: PathBuf,
        state_dir: PathBuf,
        worker: Arc<WorkerSupervisor>,
        options: IndexOptions,
    ) -> Result<Self, IndexError> {
        Self::start_with(
            root,
            state_dir,
            worker,
            options,
            Box::new(super::watch::IndexWatcher::start),
        )
    }

    /// [`Self::start`] with the watcher's start given: the seam a test holds
    /// the operating system call through.
    pub(super) fn start_with(
        root: PathBuf,
        state_dir: PathBuf,
        worker: Arc<WorkerSupervisor>,
        options: IndexOptions,
        watch: super::coordinator::StartWatcher,
    ) -> Result<Self, IndexError> {
        if options.max_files == 0
            || options.max_files > 1_000_000
            || options.max_file_bytes == 0
            || options.max_file_bytes > 1024 * 1024
            || options.poll_interval.is_zero()
            || options.worker_timeout.is_zero()
        {
            return Err(IndexError::Unavailable("invalid index limits".into()));
        }
        let root = root.canonicalize()?;
        if !root.is_dir() {
            return Err(IndexError::Unavailable(
                "workspace root is not a directory".into(),
            ));
        }
        // The composing owner supplies an admitted state directory. Existing
        // ancestor aliases are normalized before checking watcher exclusions.
        let state_dir = match state_dir.canonicalize() {
            Ok(path) => path,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                let parent = state_dir.parent().ok_or_else(|| {
                    IndexError::Unavailable("state directory needs a parent".into())
                })?;
                let name = state_dir.file_name().ok_or_else(|| {
                    IndexError::Unavailable("state directory needs a name".into())
                })?;
                parent.canonicalize()?.join(name)
            }
            Err(error) => return Err(error.into()),
        };
        if state_dir.starts_with(&root) && !state_dir.starts_with(root.join(".workbench")) {
            return Err(IndexError::Unavailable("in-workspace index state must be inside .workbench so watcher events exclude cache writes".into()));
        }
        let shared = Arc::new(Shared {
            root,
            state_dir,
            options,
            worker,
            cache: Mutex::new(None),
            watcher: Mutex::new(None),
            status: Mutex::new(IndexStatus::default()),
            requested: AtomicU64::new(1),
            stopped: CancellationToken::new(),
        });
        let runtime = tokio::runtime::Handle::try_current()
            .map_err(|e| IndexError::Unavailable(e.to_string()))?;
        let task_shared = shared.clone();
        let task = runtime.spawn(async move {
            super::coordinator::Coordinator::run(task_shared, watch).await;
        });
        Ok(Self {
            shared,
            task: Some(task),
        })
    }
    pub fn status(&self) -> IndexStatus {
        let mut status = self
            .shared
            .status
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .clone();
        if let Ok(watcher) = self.shared.watcher.try_lock() {
            if let Some(watcher) = watcher.as_ref() {
                let snapshot = watcher.snapshot();
                if snapshot.pending
                    || snapshot.mode == super::watch::WatchMode::Off
                    || snapshot.stopped
                    || snapshot.last_error.is_some()
                {
                    if status.freshness == Freshness::Warm {
                        status.freshness = Freshness::Warming;
                    }
                    status.notes.push(format!(
                        "Watcher {}: pending={}, lag={}ms.",
                        snapshot.mode.as_str(),
                        snapshot.pending,
                        snapshot.lag_ms
                    ));
                    if let Some(error) = snapshot.last_error {
                        status.notes.push(error);
                    }
                }
            }
        } else if status.freshness == Freshness::Warm {
            status.freshness = Freshness::Warming;
        }
        status
    }
    pub fn request_refresh(&self) {
        self.shared.requested.fetch_add(1, Ordering::AcqRel);
        self.shared
            .status
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .freshness = Freshness::Warming;
    }
    pub fn symbols(
        &self,
        prefix: &str,
        limit: usize,
    ) -> Result<IndexQuery<CachedSymbol>, IndexError> {
        self.query(
            limit,
            |db, bound| db.symbols(prefix, bound),
            |row| &row.file,
        )
    }
    /// A hydrated file outline whose source bytes, mode and selection are verified live.
    pub fn file(&self, path: &str) -> Result<IndexQuery<IndexedFile>, IndexError> {
        self.query(
            1,
            |db, _| Ok(db.file(path)?.into_iter().collect()),
            |row| row,
        )
    }
    pub fn fts(&self, literal: &str, limit: usize) -> Result<IndexQuery<IndexedFile>, IndexError> {
        self.query(limit, |db, bound| db.fts(literal, bound), |row| row)
    }
    /// Live paths the index does not know at all: a membership probe with
    /// no row validation, so a references answer can name what it could
    /// not see without paying for every file in scope.
    pub fn unknown_paths(&self, paths: &[&str]) -> Result<Vec<String>, IndexError> {
        let guard = self.cache_for_probe()?;
        let cache = guard
            .as_ref()
            .ok_or_else(|| IndexError::Unavailable("Index is opening".into()))?;
        Ok(cache.unknown_paths(paths)?)
    }
    /// The first `limit` content-index (FTS) files mentioning `literal`, as
    /// paths — a bounded candidate probe with no row validation, because a
    /// candidate is verified against its live bytes when it is scanned, not
    /// when it is named. `true` when more candidates exist beyond the bound.
    pub fn fts_paths(
        &self,
        literal: &str,
        limit: usize,
    ) -> Result<(Vec<String>, bool), IndexError> {
        let guard = self.cache_for_probe()?;
        let cache = guard
            .as_ref()
            .ok_or_else(|| IndexError::Unavailable("Index is opening".into()))?;
        let bound = limit.min(1000);
        let mut paths: Vec<String> = cache
            .fts(literal, (bound + 1).min(1000))?
            .into_iter()
            .map(|file| file.path)
            .collect();
        let truncated = paths.len() > bound;
        paths.truncate(bound);
        Ok((paths, truncated))
    }
    /// The cache for a probe that hydrates nothing; never queued behind a
    /// build or a cache write.
    fn cache_for_probe(&self) -> Result<std::sync::MutexGuard<'_, Option<IndexCache>>, IndexError> {
        if self.shared.stopped.is_cancelled() {
            return Err(IndexError::Unavailable("Index is stopped".into()));
        }
        self.shared
            .cache
            .try_lock()
            .map_err(|_| IndexError::Unavailable("Index is updating".into()))
    }
    pub fn files(
        &self,
        after: Option<&str>,
        limit: usize,
    ) -> Result<IndexQuery<IndexedFile>, IndexError> {
        self.query(limit, |db, bound| db.files_page(after, bound), |row| row)
    }
    fn query<T>(
        &self,
        limit: usize,
        fetch: impl FnOnce(&IndexCache, usize) -> Result<Vec<T>, super::cache::CacheError>,
        file: impl Fn(&T) -> &IndexedFile,
    ) -> Result<IndexQuery<T>, IndexError> {
        let status = self.status();
        let error = status.last_error.clone();
        let mut query = IndexQuery {
            rows: Vec::new(),
            freshness: status.freshness,
            generation: status.generation,
            truncated: limit > 1000,
            notes: status.notes,
        };
        if let Some(error) = error {
            query.notes.push(error);
        }
        if self.shared.stopped.is_cancelled() {
            query.freshness = Freshness::Cold;
            query
                .notes
                .push("Index is stopped; use cold source queries.".into());
            return Ok(query);
        }
        // Never queue behind a cache write or a build. Cold lexical work is a
        // separate native capability and does not require this derived cache.
        let Ok(guard) = self.shared.cache.try_lock() else {
            query.freshness = Freshness::Warming;
            query
                .notes
                .push("Index is updating; retry or use cold source queries.".into());
            return Ok(query);
        };
        let Some(cache) = guard.as_ref() else {
            query.freshness = Freshness::Cold;
            query
                .notes
                .push("Index is opening; cold source queries remain available.".into());
            return Ok(query);
        };
        let bound = limit.min(1000);
        let mut rows = fetch(cache, if bound == 0 { 0 } else { (bound + 1).min(1000) })?;
        query.generation = cache.generation()?;
        query.truncated |= rows.len() > bound || (bound == 1000 && rows.len() == 1000);
        rows.truncate(bound);
        drop(guard);
        let mut observed = std::collections::HashMap::new();
        let mut budget = super::validation_budget::ValidationBudget::new(
            8 * 1024 * 1024,
            self.shared.options.max_file_bytes,
        );
        let mut admitted = super::selection::Selection::admission(&self.shared.root);
        for row in rows {
            let indexed = file(&row);
            let valid = if let Some(valid) = observed.get(&indexed.path) {
                *valid
            } else {
                let valid =
                    budget.validate(&self.shared.root, &self.shared.options.languages, indexed)
                        && admitted(&indexed.path);
                observed.insert(indexed.path.clone(), valid);
                valid
            };
            if valid {
                query.rows.push(row);
            } else {
                query.freshness = Freshness::Warming;
                query.truncated = true;
            }
        }
        if observed.values().any(|valid| !valid) {
            query.notes.push("Stale or unvalidated anchors omitted; refresh requested. Validation is optimistic, not an immutable source snapshot.".into());
            self.request_refresh();
        }
        Ok(query)
    }
    pub async fn shutdown(&mut self) -> Result<(), IndexError> {
        self.shared.stopped.cancel();
        if let Some(task) = self.task.take() {
            task.await
                .map_err(|e| IndexError::Unavailable(e.to_string()))?;
        }
        Ok(())
    }
}
impl Drop for WorkspaceIndex {
    fn drop(&mut self) {
        self.shared.stopped.cancel();
    }
}
