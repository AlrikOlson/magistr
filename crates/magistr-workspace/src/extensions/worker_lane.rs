use std::{sync::Arc, time::Duration};

use super::{
    ExtensionLane, LaneFuture, PluginGuestPolicy, PluginInvokeOutcome, RankingContributeOutcome,
    ResolverLaneOutcome, RunnerInput, RunnerLaneOutcome, WasiLaneOutcome,
};
use crate::{
    contracts::{ErrorKind, WorkspaceError},
    query::worker_error,
    worker::{
        WorkerRankingHit, WorkerRequest, WorkerResponse, WorkerSubprocessSpec, WorkerSupervisor,
    },
};

/// The production lane: every guest executes inside the supervised native
/// worker process, never in the host. Guest bytes travel with each request;
/// the worker's frame ceiling, deadlines and quarantine apply unchanged.
pub struct WorkerExtensionLane {
    worker: Arc<WorkerSupervisor>,
    timeout: Duration,
}

impl WorkerExtensionLane {
    #[must_use]
    pub fn new(worker: Arc<WorkerSupervisor>) -> Self {
        Self {
            worker,
            timeout: Duration::from_secs(60),
        }
    }

    #[must_use]
    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }

    async fn call(&self, request: WorkerRequest) -> Result<WorkerResponse, WorkspaceError> {
        self.worker
            .call(
                request,
                self.timeout,
                &tokio_util::sync::CancellationToken::new(),
            )
            .await
            .map_err(worker_error)
    }
}

fn mismatched<T>() -> Result<T, WorkspaceError> {
    Err(WorkspaceError::new(
        ErrorKind::EngineUnavailable,
        "worker returned a mismatched extension response",
    ))
}

impl ExtensionLane for WorkerExtensionLane {
    fn invoke_tool(
        &self,
        guest: Vec<u8>,
        export: String,
        payload: String,
        capabilities: Vec<String>,
        policy: PluginGuestPolicy,
    ) -> LaneFuture<'_, PluginInvokeOutcome> {
        Box::pin(async move {
            match self
                .call(WorkerRequest::InvokeComponent {
                    guest,
                    export,
                    payload,
                    capabilities,
                    policy: policy.into(),
                    subprocess_specs: Vec::new(),
                    subprocess_root: String::new(),
                })
                .await?
            {
                WorkerResponse::InvokeComponent {
                    end,
                    fuel_used,
                    result,
                } => Ok(PluginInvokeOutcome {
                    end,
                    fuel_used,
                    result,
                }),
                _ => mismatched(),
            }
        })
    }

    #[cfg(feature = "lang-pack")]
    fn load_lang_pack(
        &self,
        guest: Vec<u8>,
        capabilities: Vec<String>,
        policy: PluginGuestPolicy,
        blessed: bool,
    ) -> LaneFuture<'_, super::LangPackLoadOutcome> {
        Box::pin(async move {
            match self
                .call(WorkerRequest::LoadLanguagePack {
                    guest,
                    capabilities,
                    policy: policy.into(),
                    blessed,
                })
                .await?
            {
                WorkerResponse::LanguagePackLoaded {
                    end,
                    fuel_used,
                    registration,
                } => Ok(super::LangPackLoadOutcome {
                    end,
                    fuel_used,
                    registration,
                }),
                _ => mismatched(),
            }
        })
    }

    fn contribute_ranking(
        &self,
        guest: Vec<u8>,
        hits: Vec<WorkerRankingHit>,
        capabilities: Vec<String>,
        policy: PluginGuestPolicy,
    ) -> LaneFuture<'_, RankingContributeOutcome> {
        Box::pin(async move {
            match self
                .call(WorkerRequest::RankingContribute {
                    guest,
                    hits,
                    capabilities,
                    policy: policy.into(),
                })
                .await?
            {
                WorkerResponse::RankingContributed {
                    end,
                    fuel_used,
                    contributions,
                } => Ok(RankingContributeOutcome {
                    end,
                    fuel_used,
                    contributions,
                }),
                _ => mismatched(),
            }
        })
    }

    fn resolve_references(
        &self,
        guest: Vec<u8>,
        symbol: String,
        capabilities: Vec<String>,
        policy: PluginGuestPolicy,
        specs: Vec<WorkerSubprocessSpec>,
        root: String,
    ) -> LaneFuture<'_, ResolverLaneOutcome> {
        Box::pin(async move {
            match self
                .call(WorkerRequest::Resolver {
                    guest,
                    symbol,
                    capabilities,
                    policy: policy.into(),
                    subprocess_specs: specs,
                    subprocess_root: root,
                })
                .await?
            {
                WorkerResponse::Resolver {
                    end,
                    fuel_used,
                    locations,
                    sessions_opened,
                    sessions_reused,
                } => Ok(ResolverLaneOutcome {
                    end,
                    fuel_used,
                    locations,
                    sessions_opened,
                    sessions_reused,
                }),
                _ => mismatched(),
            }
        })
    }

    fn parse_run_output(
        &self,
        guest: Vec<u8>,
        input: RunnerInput,
        capabilities: Vec<String>,
        policy: PluginGuestPolicy,
    ) -> LaneFuture<'_, RunnerLaneOutcome> {
        Box::pin(async move {
            match self
                .call(WorkerRequest::RunnerParse {
                    guest,
                    command: input.command,
                    exit_code: input.exit_code,
                    stdout: input.stdout,
                    stderr: input.stderr,
                    capabilities,
                    policy: policy.into(),
                })
                .await?
            {
                WorkerResponse::RunnerParsed {
                    end,
                    fuel_used,
                    claimed,
                    report,
                } => Ok(RunnerLaneOutcome {
                    end,
                    fuel_used,
                    claimed,
                    report,
                }),
                _ => mismatched(),
            }
        })
    }

    fn run_wasi(
        &self,
        guest: Vec<u8>,
        argv: Vec<String>,
        policy: PluginGuestPolicy,
    ) -> LaneFuture<'_, WasiLaneOutcome> {
        Box::pin(async move {
            match self
                .call(WorkerRequest::Wasi {
                    guest,
                    argv,
                    policy: policy.into(),
                })
                .await?
            {
                WorkerResponse::Wasi {
                    end,
                    fuel_used,
                    exit_code,
                    stdout,
                    stderr,
                } => Ok(WasiLaneOutcome {
                    end,
                    fuel_used,
                    exit_code,
                    stdout,
                    stderr,
                }),
                _ => mismatched(),
            }
        })
    }
}
