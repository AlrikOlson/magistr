use super::watch::IndexWatcher;
use super::{IndexOptions, IndexStatus, cache::IndexCache};
use crate::worker::WorkerSupervisor;
use std::{
    path::PathBuf,
    sync::{Arc, Mutex, atomic::AtomicU64},
};
use tokio_util::sync::CancellationToken;
pub(super) struct Shared {
    pub root: PathBuf,
    pub state_dir: PathBuf,
    pub options: IndexOptions,
    pub worker: Arc<WorkerSupervisor>,
    pub cache: Mutex<Option<IndexCache>>,
    pub watcher: Mutex<Option<IndexWatcher>>,
    pub status: Mutex<IndexStatus>,
    pub requested: AtomicU64,
    pub stopped: CancellationToken,
}
