use crate::{
    contracts::{ErrorKind, Freshness, WorkspaceError},
    index::{IndexQuery, WorkspaceIndex, cache::IndexedFile},
    worker::{WorkerRequest, WorkerResponse, WorkerSupervisor},
    workspace_policy::WorkspacePolicy,
};
use std::{path::PathBuf, time::Duration};
use tokio_util::sync::CancellationToken;

/// Typed source queries over admitted policy, optional derived index and native parser worker.
/// This borrowed facade creates no workspace owner and acquires no workspace lock.
pub struct WorkspaceQueries<'a> {
    pub(super) root: PathBuf,
    pub(super) policy: &'a WorkspacePolicy,
    pub(super) worker: &'a WorkerSupervisor,
    pub(super) index: Option<&'a WorkspaceIndex>,
    pub(super) timeout: Duration,
    /// The request's own cancellation: a caller that goes away stops the
    /// worker call instead of letting the query burn on (chunk arch-14c10).
    pub(super) cancel: CancellationToken,
    /// The admitted state directory whose `plugins.json` the map inventory
    /// reads; absent, the map row says the inventory is unavailable.
    pub(super) state_dir: Option<PathBuf>,
    /// Promoted ranking signals and the shadow lane (chunk arch-14c8).
    #[cfg(feature = "sandbox")]
    pub(super) ranking: Option<&'a crate::extensions::RankingSignals>,
    /// The resolver upgrade lane for references (chunk arch-14c8).
    #[cfg(feature = "sandbox")]
    pub(super) resolver: Option<&'a crate::extensions::PluginResolver<'a>>,
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
            cancel: CancellationToken::new(),
            state_dir: None,
            #[cfg(feature = "sandbox")]
            ranking: None,
            #[cfg(feature = "sandbox")]
            resolver: None,
        })
    }

    /// Tie every worker call to the request's cancellation token.
    #[must_use]
    pub fn with_cancel(mut self, cancel: CancellationToken) -> Self {
        self.cancel = cancel;
        self
    }

    /// Let map read the installed-plugin inventory under this state directory.
    #[must_use]
    pub fn with_state_dir(mut self, state_dir: PathBuf) -> Self {
        self.state_dir = Some(state_dir);
        self
    }

    /// Fold promoted ranking features into search and feed the shadow lane
    /// after every search, off the response path.
    #[cfg(feature = "sandbox")]
    #[must_use]
    pub fn with_ranking(mut self, ranking: &'a crate::extensions::RankingSignals) -> Self {
        self.ranking = Some(ranking);
        self
    }

    /// Merge resolver-confirmed edges into references searches.
    #[cfg(feature = "sandbox")]
    #[must_use]
    pub fn with_resolver(mut self, resolver: &'a crate::extensions::PluginResolver<'a>) -> Self {
        self.resolver = Some(resolver);
        self
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
            .call(request, self.timeout, &self.cancel)
            .await
            .map_err(super::worker_error::worker_error)
    }
}
