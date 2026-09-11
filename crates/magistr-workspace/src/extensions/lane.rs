use super::{
    LaneFuture, PluginGuestPolicy, PluginInvokeOutcome, RankingContributeOutcome,
    ResolverLaneOutcome, RunnerInput, RunnerLaneOutcome, WasiLaneOutcome,
};
use crate::{
    contracts::{ErrorKind, WorkspaceError},
    worker::{WorkerRankingHit, WorkerSubprocessSpec},
};

/// Every way a guest may execute. The production lane is the supervised
/// native worker ([`super::WorkerExtensionLane`]); tests script outcomes. A
/// method a lane does not provide answers `ENGINE_UNAVAILABLE`, the documented
/// degraded answer for an absent sandbox host.
pub trait ExtensionLane: Send + Sync {
    /// The `(string) -> string` plugin tool convention.
    fn invoke_tool(
        &self,
        guest: Vec<u8>,
        export: String,
        payload: String,
        capabilities: Vec<String>,
        policy: PluginGuestPolicy,
    ) -> LaneFuture<'_, PluginInvokeOutcome> {
        let _ = (guest, export, payload, capabilities, policy);
        unavailable("tool")
    }

    /// Read a lang-pack carrier and register it in the worker's registry.
    #[cfg(feature = "lang-pack")]
    fn load_lang_pack(
        &self,
        guest: Vec<u8>,
        capabilities: Vec<String>,
        policy: PluginGuestPolicy,
        blessed: bool,
    ) -> LaneFuture<'_, super::LangPackLoadOutcome> {
        let _ = (guest, capabilities, policy, blessed);
        unavailable("lang-pack")
    }

    /// Batched `contribute` over one ranking-provider instantiation.
    fn contribute_ranking(
        &self,
        guest: Vec<u8>,
        hits: Vec<WorkerRankingHit>,
        capabilities: Vec<String>,
        policy: PluginGuestPolicy,
    ) -> LaneFuture<'_, RankingContributeOutcome> {
        let _ = (guest, hits, capabilities, policy);
        unavailable("ranking")
    }

    /// `resolve-references` over a resolver-provider with the host-curated
    /// language-server table.
    fn resolve_references(
        &self,
        guest: Vec<u8>,
        symbol: String,
        capabilities: Vec<String>,
        policy: PluginGuestPolicy,
        specs: Vec<WorkerSubprocessSpec>,
        root: String,
    ) -> LaneFuture<'_, ResolverLaneOutcome> {
        let _ = (guest, symbol, capabilities, policy, specs, root);
        unavailable("resolver")
    }

    /// `can-parse` then `parse-output` over a runner-provider.
    fn parse_run_output(
        &self,
        guest: Vec<u8>,
        input: RunnerInput,
        capabilities: Vec<String>,
        policy: PluginGuestPolicy,
    ) -> LaneFuture<'_, RunnerLaneOutcome> {
        let _ = (guest, input, capabilities, policy);
        unavailable("runner")
    }

    /// One `wasm32-wasip1` command round (the script interpreter substrate).
    fn run_wasi(
        &self,
        guest: Vec<u8>,
        argv: Vec<String>,
        policy: PluginGuestPolicy,
    ) -> LaneFuture<'_, WasiLaneOutcome> {
        let _ = (guest, argv, policy);
        unavailable("wasi")
    }
}

fn unavailable<'a, T: Send + 'a>(lane: &'static str) -> LaneFuture<'a, T> {
    Box::pin(async move {
        Err(WorkspaceError::new(
            ErrorKind::EngineUnavailable,
            format!("the {lane} guest lane is not available in this composition"),
        ))
    })
}
