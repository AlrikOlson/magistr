use crate::{
    contracts::{ErrorKind, Freshness, WorkspaceError},
    index::{IndexQuery, WorkspaceIndex, cache::IndexedFile},
    worker::{WorkerRequest, WorkerResponse, WorkerSupervisor},
    workspace_policy::WorkspacePolicy,
};
use std::{path::PathBuf, time::Duration};

/// Typed source queries over admitted policy, optional derived index and native parser worker.
/// This borrowed facade creates no workspace owner and acquires no workspace lock.
pub struct WorkspaceQueries<'a> {
    pub(super) root: PathBuf,
    pub(super) policy: &'a WorkspacePolicy,
    pub(super) worker: &'a WorkerSupervisor,
    pub(super) index: Option<&'a WorkspaceIndex>,
    pub(super) timeout: Duration,
}

impl<'a> WorkspaceQueries<'a> {
    pub fn new(
        policy: &'a WorkspacePolicy,
        worker: &'a WorkerSupervisor,
        index: Option<&'a WorkspaceIndex>,
    ) -> Result<Self, WorkspaceError> {
        Ok(Self {
            root: policy.check_read(".")?,
            policy,
            worker,
            index,
            timeout: Duration::from_secs(30),
        })
    }

    pub async fn search(
        &self,
        input: &super::SearchInput,
    ) -> crate::contracts::WorkspaceResponse<super::SearchGroup> {
        super::search::run(self, input).await
    }

    pub(super) fn freshness(&self) -> Freshness {
        self.index
            .map_or(Freshness::Cold, |index| index.status().freshness)
    }

    pub(super) fn index_file(&self, path: &str) -> Result<IndexQuery<IndexedFile>, WorkspaceError> {
        self.policy.check_read(path)?;
        self.index
            .ok_or_else(|| {
                WorkspaceError::new(
                    ErrorKind::IndexUnavailable,
                    "index is unavailable; cold file and lexical queries remain available",
                )
            })?
            .file(path)
            .map_err(|error| WorkspaceError::new(ErrorKind::IndexUnavailable, error.to_string()))
    }

    pub(super) async fn call(
        &self,
        request: WorkerRequest,
    ) -> Result<WorkerResponse, WorkspaceError> {
        self.worker
            .call(
                request,
                self.timeout,
                &tokio_util::sync::CancellationToken::new(),
            )
            .await
            .map_err(super::worker_error::worker_error)
    }
}
