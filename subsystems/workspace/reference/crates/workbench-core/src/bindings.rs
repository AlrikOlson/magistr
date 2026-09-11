//! The napi boundary: thin, synchronous wrappers over [`crate::engine`].
//! Nothing here contains logic — errors map to `napi::Error` reasons and the
//! structs mirror the engine types 1:1 (napi camelCases field names for JS).

use napi::bindgen_prelude::*;
use napi_derive::napi;

use crate::engine;
use crate::indexer;

fn reason(e: engine::CoreError) -> Error {
    Error::from_reason(e.to_string())
}

#[napi]
pub fn version() -> String {
    crate::VERSION.to_string()
}

#[napi]
pub fn languages() -> Vec<String> {
    let mut out: Vec<String> = crate::langs::language_ids()
        .into_iter()
        .map(str::to_string)
        .collect();
    // Registered pack languages ride the same list so the engine child's
    // language checks (edit parse-verify included) need no second lookup.
    // Bundled ids come first; registration refuses shadowing (FR-005).
    #[cfg(feature = "lang-pack")]
    out.extend(crate::lang_registry::registered_ids());
    out
}

#[napi(object)]
pub struct ParseSummary {
    pub language: String,
    pub root_kind: String,
    pub node_count: u32,
    pub error_count: u32,
    pub has_error: bool,
}

#[napi]
pub fn parse(language: String, source: String) -> Result<ParseSummary> {
    let s = engine::parse(&language, &source).map_err(reason)?;
    Ok(ParseSummary {
        language: s.language,
        root_kind: s.root_kind,
        node_count: s.node_count,
        error_count: s.error_count,
        has_error: s.has_error,
    })
}

#[napi(object)]
pub struct SearchHit {
    /// 1-based line number.
    pub line: u32,
    pub start: u32,
    pub end: u32,
    pub line_text: String,
}

#[napi]
pub fn search(
    pattern: String,
    haystack: String,
    case_insensitive: Option<bool>,
) -> Result<Vec<SearchHit>> {
    let hits =
        engine::search(&pattern, &haystack, case_insensitive.unwrap_or(false)).map_err(reason)?;
    Ok(hits
        .into_iter()
        .map(|h| SearchHit {
            line: u32::try_from(h.line).unwrap_or(u32::MAX),
            start: h.start,
            end: h.end,
            line_text: h.line_text,
        })
        .collect())
}

#[napi(object)]
pub struct StructuralHit {
    /// 1-based line of the match start.
    pub line: u32,
    /// 1-based byte column of the match start within its line.
    pub column: u32,
    /// 1-based line of the match end.
    pub end_line: u32,
    /// tree-sitter node kind of the matched node.
    pub kind: String,
    /// The full source line containing the match start.
    pub line_text: String,
}

#[napi]
pub fn structural_search(
    language: String,
    source: String,
    pattern: String,
) -> Result<Vec<StructuralHit>> {
    let hits = engine::structural_search(&language, &source, &pattern).map_err(reason)?;
    Ok(hits
        .into_iter()
        .map(|h| StructuralHit {
            line: u32::try_from(h.line).unwrap_or(u32::MAX),
            column: h.column,
            end_line: u32::try_from(h.end_line).unwrap_or(u32::MAX),
            kind: h.kind,
            line_text: h.line_text,
        })
        .collect())
}

#[napi(object)]
pub struct RewriteOutcome {
    pub output: String,
    pub match_count: u32,
}

#[napi]
pub fn rewrite(
    language: String,
    source: String,
    pattern: String,
    replacement: String,
) -> Result<RewriteOutcome> {
    let o = engine::rewrite(&language, &source, &pattern, &replacement).map_err(reason)?;
    Ok(RewriteOutcome {
        output: o.output,
        match_count: o.match_count,
    })
}

#[napi(object)]
pub struct SymbolRec {
    pub name: String,
    pub kind: String,
    /// 1-based.
    pub start_line: u32,
    /// 1-based.
    pub start_col: u32,
    pub end_line: u32,
    /// Qualification the address dialect does not carry — the trait a Rust
    /// impl member came from (specs/009). Absent for everything else.
    pub detail: Option<String>,
    /// First line of the item's doc comment (specs/009). Absent when none.
    pub doc: Option<String>,
}

/// One inclusive 1-based line range holding test-only code — a Rust
/// `#[cfg(test)]` module (specs/009 R4).
#[napi(object)]
pub struct TestRangeRec {
    pub start_line: u32,
    pub end_line: u32,
}

#[napi(object)]
pub struct Extraction {
    pub symbols: Vec<SymbolRec>,
    pub imports: Vec<String>,
    pub test_ranges: Vec<TestRangeRec>,
}

fn extraction_out(e: indexer::Extraction) -> Extraction {
    Extraction {
        symbols: e
            .symbols
            .into_iter()
            .map(|s| SymbolRec {
                name: s.name,
                kind: s.kind.to_string(),
                start_line: s.start_line,
                start_col: s.start_col,
                end_line: s.end_line,
                detail: s.detail,
                doc: s.doc,
            })
            .collect(),
        imports: e.imports,
        test_ranges: e
            .test_ranges
            .into_iter()
            .map(|r| TestRangeRec {
                start_line: r.start_line,
                end_line: r.end_line,
            })
            .collect(),
    }
}

/// Tier-1 extraction as an async task: each task runs on a worker thread with
/// its own parser arena, so the engine child's frame loop never blocks on a
/// parse (SDD §7.3.2).
pub struct ExtractTask {
    language: String,
    source: String,
}

#[napi]
impl Task for ExtractTask {
    type Output = indexer::Extraction;
    type JsValue = Extraction;

    fn compute(&mut self) -> Result<Self::Output> {
        indexer::extract(&self.language, &self.source).map_err(reason)
    }

    fn resolve(&mut self, _env: Env, output: Self::Output) -> Result<Self::JsValue> {
        Ok(extraction_out(output))
    }
}

#[napi(ts_return_type = "Promise<Extraction>")]
pub fn extract(language: String, source: String) -> AsyncTask<ExtractTask> {
    AsyncTask::new(ExtractTask { language, source })
}

#[napi(object)]
pub struct ScanEntry {
    /// Workspace-relative, `/`-separated.
    pub path: String,
    pub size: i64,
    pub mtime_ms: i64,
    /// Vendor/generated: demoted in ranking, never excluded.
    pub junk: bool,
}

/// Workspace scan as an async task (gitignore semantics, junk flagging).
pub struct ScanTask {
    root: String,
}

#[napi]
impl Task for ScanTask {
    type Output = Vec<indexer::ScanEntry>;
    type JsValue = Vec<ScanEntry>;

    fn compute(&mut self) -> Result<Self::Output> {
        indexer::scan_workspace(&self.root).map_err(reason)
    }

    fn resolve(&mut self, _env: Env, output: Self::Output) -> Result<Self::JsValue> {
        Ok(output
            .into_iter()
            .map(|e| ScanEntry {
                path: e.path,
                size: i64::try_from(e.size).unwrap_or(i64::MAX),
                mtime_ms: e.mtime_ms,
                junk: e.junk,
            })
            .collect())
    }
}

#[napi(ts_return_type = "Promise<Array<ScanEntry>>")]
pub fn scan_workspace(root: String) -> AsyncTask<ScanTask> {
    AsyncTask::new(ScanTask { root })
}

#[napi(object)]
pub struct SelfTestReport {
    pub parse_ok: bool,
    pub search_ok: bool,
    pub rewrite_ok: bool,
    pub all_ok: bool,
    pub details: Vec<String>,
}

// ---- Wasmtime extension host (SDD §8.1, native builds only) ---------------

#[cfg(feature = "sandbox")]
use crate::sandbox;

/// The one guest resource policy (SDD §8.1) — mirrors `sandbox::SandboxPolicy`.
#[cfg(feature = "sandbox")]
#[napi(object)]
pub struct SandboxPolicyArg {
    /// Fuel budget; exhaustion is a deterministic kill.
    pub fuel: i64,
    /// Linear-memory cap in bytes; a grow past it traps the guest.
    pub memory_cap_bytes: i64,
}

#[cfg(feature = "sandbox")]
#[napi(object)]
pub struct SandboxComponentOutcome {
    /// ok | out-of-fuel | memory-cap | cap-denied | guest-trap | invalid-guest
    pub outcome: String,
    /// Denied import name, trap message, or invalid-guest reason.
    pub detail: String,
    pub fuel_used: i64,
    /// Debug-rendered component values returned by the export.
    pub results: Vec<String>,
    /// Values the guest passed to the granted `observe` capability.
    pub observations: Vec<u32>,
}

#[cfg(feature = "sandbox")]
#[napi(object)]
pub struct SandboxWasiOutcome {
    /// ok | out-of-fuel | memory-cap | guest-trap | guest-exit | invalid-guest
    pub outcome: String,
    pub detail: String,
    pub fuel_used: i64,
    pub exit_code: Option<i32>,
    pub stdout: String,
    pub stderr: String,
}

#[cfg(feature = "sandbox")]
fn sandbox_policy(arg: &SandboxPolicyArg) -> sandbox::SandboxPolicy {
    // Negative inputs clamp to zero: zero fuel / zero memory is a
    // deterministic immediate kill, never undefined behavior.
    sandbox::SandboxPolicy {
        fuel: u64::try_from(arg.fuel).unwrap_or(0),
        memory_cap_bytes: usize::try_from(arg.memory_cap_bytes).unwrap_or(0),
    }
}

#[cfg(feature = "sandbox")]
fn sandbox_reason(e: sandbox::SandboxError) -> Error {
    Error::from_reason(e.to_string())
}

/// Capability objects this host can grant (default deny; SDD §8.1).
#[cfg(feature = "sandbox")]
#[napi]
pub fn sandbox_capabilities() -> Vec<String> {
    sandbox::CAPABILITIES
        .iter()
        .map(|s| s.to_string())
        .collect()
}

/// Component-lane run as an async task: a fuel-burning guest occupies a
/// worker thread, never the engine child's frame loop.
#[cfg(feature = "sandbox")]
pub struct SandboxComponentTask {
    guest: Vec<u8>,
    export: String,
    args: Vec<i64>,
    capabilities: Vec<String>,
    policy: sandbox::SandboxPolicy,
}

#[cfg(feature = "sandbox")]
#[napi]
impl Task for SandboxComponentTask {
    type Output = sandbox::ComponentOutcome;
    type JsValue = SandboxComponentOutcome;

    fn compute(&mut self) -> Result<Self::Output> {
        sandbox::run_component(
            &self.guest,
            &self.export,
            &self.args,
            &self.capabilities,
            self.policy,
        )
        .map_err(sandbox_reason)
    }

    fn resolve(&mut self, _env: Env, o: Self::Output) -> Result<Self::JsValue> {
        Ok(SandboxComponentOutcome {
            outcome: o.end.kind().to_string(),
            detail: o.end.detail(),
            fuel_used: i64::try_from(o.fuel_used).unwrap_or(i64::MAX),
            results: o.results,
            observations: o.observations,
        })
    }
}

#[cfg(feature = "sandbox")]
#[napi(ts_return_type = "Promise<SandboxComponentOutcome>")]
pub fn sandbox_run_component(
    guest: Buffer,
    export_name: String,
    args: Vec<i64>,
    capabilities: Vec<String>,
    policy: SandboxPolicyArg,
) -> AsyncTask<SandboxComponentTask> {
    AsyncTask::new(SandboxComponentTask {
        guest: guest.to_vec(),
        export: export_name,
        args,
        capabilities,
        policy: sandbox_policy(&policy),
    })
}

/// Component-lane JSON invoke result — mirrors `sandbox::InvokeOutcome`.
#[cfg(feature = "sandbox")]
#[napi(object)]
pub struct SandboxInvokeOutcome {
    /// ok | out-of-fuel | memory-cap | cap-denied | guest-trap | invalid-guest
    pub outcome: String,
    /// Denied import name, trap message, or invalid-guest reason.
    pub detail: String,
    pub fuel_used: i64,
    /// The export's string result, present iff outcome is "ok".
    pub result: Option<String>,
}

/// One host-curated language-server spec crossing the FFI boundary — mirrors
/// `sandbox::SubprocessSpec` (chunk:m5-lsp-resolver-plugin). Only the host
/// constructs these; a guest addresses a `language_id`, never a binary.
#[cfg(feature = "sandbox")]
#[napi(object)]
#[derive(Clone)]
pub struct SubprocessSpecArg {
    pub language_id: String,
    pub bin: String,
    pub args: Vec<String>,
}

#[cfg(feature = "sandbox")]
fn subprocess_specs(specs: Option<Vec<SubprocessSpecArg>>) -> Vec<sandbox::SubprocessSpec> {
    specs
        .unwrap_or_default()
        .into_iter()
        .map(|s| sandbox::SubprocessSpec {
            language_id: s.language_id,
            bin: s.bin,
            args: s.args,
        })
        .collect()
}

/// Plugin-tool invoke as an async task (SDD §8.3, chunk:m4-capability-manifests).
#[cfg(feature = "sandbox")]
pub struct SandboxInvokeTask {
    guest: Vec<u8>,
    export: String,
    payload: String,
    capabilities: Vec<String>,
    policy: sandbox::SandboxPolicy,
    specs: Vec<sandbox::SubprocessSpec>,
    root: String,
}

#[cfg(feature = "sandbox")]
#[napi]
impl Task for SandboxInvokeTask {
    type Output = sandbox::InvokeOutcome;
    type JsValue = SandboxInvokeOutcome;

    fn compute(&mut self) -> Result<Self::Output> {
        sandbox::invoke_component_json(
            &self.guest,
            &self.export,
            &self.payload,
            &self.capabilities,
            self.policy,
            &self.specs,
            &self.root,
        )
        .map_err(sandbox_reason)
    }

    fn resolve(&mut self, _env: Env, o: Self::Output) -> Result<Self::JsValue> {
        Ok(SandboxInvokeOutcome {
            outcome: o.end.kind().to_string(),
            detail: o.end.detail(),
            fuel_used: i64::try_from(o.fuel_used).unwrap_or(i64::MAX),
            result: o.result,
        })
    }
}

#[cfg(feature = "sandbox")]
#[napi(ts_return_type = "Promise<SandboxInvokeOutcome>")]
pub fn sandbox_invoke_component(
    guest: Buffer,
    export_name: String,
    payload: String,
    capabilities: Vec<String>,
    policy: SandboxPolicyArg,
    subprocess_specs_arg: Option<Vec<SubprocessSpecArg>>,
    subprocess_root: Option<String>,
) -> AsyncTask<SandboxInvokeTask> {
    AsyncTask::new(SandboxInvokeTask {
        guest: guest.to_vec(),
        export: export_name,
        payload,
        capabilities,
        policy: sandbox_policy(&policy),
        specs: subprocess_specs(subprocess_specs_arg),
        root: subprocess_root.unwrap_or_default(),
    })
}

/// One lifted `source-location` crossing the FFI boundary.
#[cfg(feature = "sandbox")]
#[napi(object)]
pub struct ResolvedLocationOut {
    pub path: String,
    pub line: u32,
    pub column: u32,
}

/// Typed resolver-lane outcome — mirrors `sandbox::ResolverOutcome`.
#[cfg(feature = "sandbox")]
#[napi(object)]
pub struct SandboxResolverOutcome {
    /// ok | out-of-fuel | memory-cap | cap-denied | guest-trap | invalid-guest
    pub outcome: String,
    /// Denied import name, trap message, or invalid-guest reason.
    pub detail: String,
    pub fuel_used: i64,
    /// Lifted locations, present (possibly empty) iff outcome is "ok".
    pub locations: Vec<ResolvedLocationOut>,
    /// Language-server sessions this invoke spawned fresh
    /// (specs/007-resolver-server-pool FR-013; additive).
    pub sessions_opened: u32,
    /// Warm pooled sessions this invoke reused instead of spawning.
    pub sessions_reused: u32,
}

/// Typed resolver invoke as an async task (chunk:m5-lsp-resolver-plugin).
#[cfg(feature = "sandbox")]
pub struct SandboxResolverTask {
    guest: Vec<u8>,
    symbol: String,
    capabilities: Vec<String>,
    policy: sandbox::SandboxPolicy,
    specs: Vec<sandbox::SubprocessSpec>,
    root: String,
}

#[cfg(feature = "sandbox")]
#[napi]
impl Task for SandboxResolverTask {
    type Output = sandbox::ResolverOutcome;
    type JsValue = SandboxResolverOutcome;

    fn compute(&mut self) -> Result<Self::Output> {
        sandbox::invoke_resolver_references(
            &self.guest,
            &self.symbol,
            &self.capabilities,
            self.policy,
            &self.specs,
            &self.root,
        )
        .map_err(sandbox_reason)
    }

    fn resolve(&mut self, _env: Env, o: Self::Output) -> Result<Self::JsValue> {
        Ok(SandboxResolverOutcome {
            outcome: o.end.kind().to_string(),
            detail: o.end.detail(),
            fuel_used: i64::try_from(o.fuel_used).unwrap_or(i64::MAX),
            locations: o
                .locations
                .into_iter()
                .map(|l| ResolvedLocationOut {
                    path: l.path,
                    line: l.line,
                    column: l.column,
                })
                .collect(),
            sessions_opened: o.sessions_opened,
            sessions_reused: o.sessions_reused,
        })
    }
}

/// One pooled language-server session (specs/007-resolver-server-pool) —
/// mirrors `sandbox::ServerPoolSessionStat`. Tests and diagnostics only.
#[cfg(feature = "sandbox")]
#[napi(object)]
pub struct ServerPoolSessionOut {
    pub language_id: String,
    pub root: String,
    pub pid: u32,
    pub leased: bool,
    pub idle_ms: i64,
}

/// Kill every warm pooled language server. The engine child calls this at
/// its normal exit (Rust statics never drop at process exit). Returns how
/// many were killed. Idempotent.
#[cfg(feature = "sandbox")]
#[napi]
pub fn sandbox_server_pool_shutdown() -> u32 {
    u32::try_from(sandbox::server_pool_shutdown()).unwrap_or(u32::MAX)
}

/// Snapshot of the pooled language-server sessions.
#[cfg(feature = "sandbox")]
#[napi]
pub fn sandbox_server_pool_stats() -> Vec<ServerPoolSessionOut> {
    sandbox::server_pool_stats()
        .into_iter()
        .map(|s| ServerPoolSessionOut {
            language_id: s.language_id,
            root: s.root,
            pid: s.pid,
            leased: s.leased,
            idle_ms: i64::try_from(s.idle_ms).unwrap_or(i64::MAX),
        })
        .collect()
}

#[cfg(feature = "sandbox")]
#[napi(ts_return_type = "Promise<SandboxResolverOutcome>")]
pub fn sandbox_resolve_references(
    guest: Buffer,
    symbol: String,
    capabilities: Vec<String>,
    policy: SandboxPolicyArg,
    subprocess_specs_arg: Option<Vec<SubprocessSpecArg>>,
    subprocess_root: Option<String>,
) -> AsyncTask<SandboxResolverTask> {
    AsyncTask::new(SandboxResolverTask {
        guest: guest.to_vec(),
        symbol,
        capabilities,
        policy: sandbox_policy(&policy),
        specs: subprocess_specs(subprocess_specs_arg),
        root: subprocess_root.unwrap_or_default(),
    })
}

/// One lifted structured failure crossing the FFI boundary.
#[cfg(feature = "sandbox")]
#[napi(object)]
pub struct StructuredFailureOut {
    pub test_name: String,
    pub file: Option<String>,
    pub assertion: String,
    pub trace: String,
}

/// One lifted run-report crossing the FFI boundary.
#[cfg(feature = "sandbox")]
#[napi(object)]
pub struct RunReportOut {
    pub failures: Vec<StructuredFailureOut>,
    pub summary: Option<String>,
}

/// Typed runner-lane outcome — mirrors `sandbox::RunnerOutcome`.
#[cfg(feature = "sandbox")]
#[napi(object)]
pub struct SandboxRunnerOutcome {
    /// ok | out-of-fuel | memory-cap | cap-denied | guest-trap | invalid-guest
    pub outcome: String,
    /// Denied import name, trap message, or invalid-guest reason.
    pub detail: String,
    pub fuel_used: i64,
    /// `can-parse`'s answer; false short-circuited without parse-output.
    pub claimed: bool,
    /// Present iff ok + claimed + the guest extracted structure.
    pub report: Option<RunReportOut>,
}

/// Typed runner invoke as an async task (chunk:m5-runner-adapter).
#[cfg(feature = "sandbox")]
pub struct SandboxRunnerTask {
    guest: Vec<u8>,
    command: String,
    exit_code: i32,
    stdout: String,
    stderr: String,
    capabilities: Vec<String>,
    policy: sandbox::SandboxPolicy,
}

#[cfg(feature = "sandbox")]
#[napi]
impl Task for SandboxRunnerTask {
    type Output = sandbox::RunnerOutcome;
    type JsValue = SandboxRunnerOutcome;

    fn compute(&mut self) -> Result<Self::Output> {
        sandbox::invoke_runner_parse(
            &self.guest,
            &self.command,
            self.exit_code,
            &self.stdout,
            &self.stderr,
            &self.capabilities,
            self.policy,
        )
        .map_err(sandbox_reason)
    }

    fn resolve(&mut self, _env: Env, o: Self::Output) -> Result<Self::JsValue> {
        Ok(SandboxRunnerOutcome {
            outcome: o.end.kind().to_string(),
            detail: o.end.detail(),
            fuel_used: i64::try_from(o.fuel_used).unwrap_or(i64::MAX),
            claimed: o.claimed,
            report: o.report.map(|r| RunReportOut {
                failures: r
                    .failures
                    .into_iter()
                    .map(|f| StructuredFailureOut {
                        test_name: f.test_name,
                        file: f.file,
                        assertion: f.assertion,
                        trace: f.trace,
                    })
                    .collect(),
                summary: r.summary,
            }),
        })
    }
}

#[cfg(feature = "sandbox")]
#[napi(ts_return_type = "Promise<SandboxRunnerOutcome>")]
pub fn sandbox_runner_parse(
    guest: Buffer,
    command: String,
    exit_code: i32,
    stdout: String,
    stderr: String,
    capabilities: Vec<String>,
    policy: SandboxPolicyArg,
) -> AsyncTask<SandboxRunnerTask> {
    AsyncTask::new(SandboxRunnerTask {
        guest: guest.to_vec(),
        command,
        exit_code,
        stdout,
        stderr,
        capabilities,
        policy: sandbox_policy(&policy),
    })
}

/// One hit-context crossing the FFI boundary into the ranking lane
/// (chunk:m5-ranking-shadow). Enum fields travel as WIT case names; the lane
/// refuses unknown names before the sandbox runs.
#[cfg(feature = "sandbox")]
#[napi(object)]
pub struct RankingHitArg {
    pub file: String,
    pub line: u32,
    pub column: u32,
    pub excerpt: String,
    /// declaration | call | import | comment | string | other
    pub kind_class: String,
    /// source | test | vendor
    pub file_class: String,
}

/// One lifted feature value crossing the FFI boundary. NOT yet namespaced —
/// the host prefixes `<plugin>/` before anything downstream sees the name.
#[cfg(feature = "sandbox")]
#[napi(object)]
pub struct FeatureOut {
    pub name: String,
    pub value: f64,
}

/// Per-hit feature lists for one batch member.
#[cfg(feature = "sandbox")]
#[napi(object)]
pub struct HitFeaturesOut {
    pub features: Vec<FeatureOut>,
}

/// Typed ranking-lane outcome — mirrors `sandbox::RankingOutcome`.
#[cfg(feature = "sandbox")]
#[napi(object)]
pub struct SandboxRankingOutcome {
    /// ok | out-of-fuel | memory-cap | cap-denied | guest-trap | invalid-guest
    pub outcome: String,
    /// Denied import name, trap message, or invalid-guest reason.
    pub detail: String,
    pub fuel_used: i64,
    /// Parallel to the input batch; shorter on a mid-batch failure (the host
    /// keeps what was lifted — shadow scoring is additive).
    pub contributions: Vec<HitFeaturesOut>,
}

/// Typed ranking invoke as an async task (chunk:m5-ranking-shadow).
#[cfg(feature = "sandbox")]
pub struct SandboxRankingTask {
    guest: Vec<u8>,
    hits: Vec<sandbox::RankingHit>,
    capabilities: Vec<String>,
    policy: sandbox::SandboxPolicy,
}

#[cfg(feature = "sandbox")]
#[napi]
impl Task for SandboxRankingTask {
    type Output = sandbox::RankingOutcome;
    type JsValue = SandboxRankingOutcome;

    fn compute(&mut self) -> Result<Self::Output> {
        sandbox::invoke_ranking_contribute(&self.guest, &self.hits, &self.capabilities, self.policy)
            .map_err(sandbox_reason)
    }

    fn resolve(&mut self, _env: Env, o: Self::Output) -> Result<Self::JsValue> {
        Ok(SandboxRankingOutcome {
            outcome: o.end.kind().to_string(),
            detail: o.end.detail(),
            fuel_used: i64::try_from(o.fuel_used).unwrap_or(i64::MAX),
            contributions: o
                .contributions
                .into_iter()
                .map(|features| HitFeaturesOut {
                    features: features
                        .into_iter()
                        .map(|f| FeatureOut {
                            name: f.name,
                            value: f.value,
                        })
                        .collect(),
                })
                .collect(),
        })
    }
}

#[cfg(feature = "sandbox")]
#[napi(ts_return_type = "Promise<SandboxRankingOutcome>")]
pub fn sandbox_ranking_contribute(
    guest: Buffer,
    hits: Vec<RankingHitArg>,
    capabilities: Vec<String>,
    policy: SandboxPolicyArg,
) -> AsyncTask<SandboxRankingTask> {
    AsyncTask::new(SandboxRankingTask {
        guest: guest.to_vec(),
        hits: hits
            .into_iter()
            .map(|h| sandbox::RankingHit {
                file: h.file,
                line: h.line,
                column: h.column,
                excerpt: h.excerpt,
                kind_class: h.kind_class,
                file_class: h.file_class,
            })
            .collect(),
        capabilities,
        policy: sandbox_policy(&policy),
    })
}

/// wasi-lane run as an async task (same policy, zero ambient authority).
#[cfg(feature = "sandbox")]
pub struct SandboxWasiTask {
    guest: Vec<u8>,
    argv: Vec<String>,
    policy: sandbox::SandboxPolicy,
}

#[cfg(feature = "sandbox")]
#[napi]
impl Task for SandboxWasiTask {
    type Output = sandbox::WasiOutcome;
    type JsValue = SandboxWasiOutcome;

    fn compute(&mut self) -> Result<Self::Output> {
        sandbox::run_wasi_module(&self.guest, &self.argv, self.policy).map_err(sandbox_reason)
    }

    fn resolve(&mut self, _env: Env, o: Self::Output) -> Result<Self::JsValue> {
        Ok(SandboxWasiOutcome {
            outcome: o.end.kind().to_string(),
            detail: o.end.detail(),
            fuel_used: i64::try_from(o.fuel_used).unwrap_or(i64::MAX),
            exit_code: o.exit_code,
            stdout: o.stdout,
            stderr: o.stderr,
        })
    }
}

#[cfg(feature = "sandbox")]
#[napi(ts_return_type = "Promise<SandboxWasiOutcome>")]
pub fn sandbox_run_wasi_module(
    guest: Buffer,
    argv: Vec<String>,
    policy: SandboxPolicyArg,
) -> AsyncTask<SandboxWasiTask> {
    AsyncTask::new(SandboxWasiTask {
        guest: guest.to_vec(),
        argv,
        policy: sandbox_policy(&policy),
    })
}

// ---- Dynamic language registry (SDD §8.3, feature 002, native only) -------

/// One registered pack language's provenance — lane is the promotion story
/// made observable (FR-007).
#[cfg(feature = "lang-pack")]
#[napi(object)]
pub struct LangPackRegistration {
    pub id: String,
    pub extensions: Vec<String>,
    /// "native" (blessed promotion) | "wasm" (the long-tail lane).
    pub lane: String,
    /// False when the pack's tag queries failed to compile: symbols degrade
    /// to none, parse-verify stays live.
    pub queries_ok: bool,
    /// Every shadowed claim or degradation, verbatim (FR-012).
    pub notes: Vec<String>,
}

/// Outcome of one carrier read + registration.
#[cfg(all(feature = "sandbox", feature = "lang-pack"))]
#[napi(object)]
pub struct LangPackLoadOutcome {
    /// ok | out-of-fuel | memory-cap | cap-denied | guest-trap | invalid-guest
    pub outcome: String,
    pub detail: String,
    pub fuel_used: i64,
    /// Present iff the carrier read succeeded AND registration was accepted.
    pub registered: Option<LangPackRegistration>,
}

/// Carrier read + registry registration as one async task: the grammar
/// bytes never cross the FFI boundary (chunk:m5-lang-pack).
#[cfg(all(feature = "sandbox", feature = "lang-pack"))]
pub struct LangPackLoadTask {
    guest: Vec<u8>,
    capabilities: Vec<String>,
    blessed: bool,
    policy: sandbox::SandboxPolicy,
}

#[cfg(all(feature = "sandbox", feature = "lang-pack"))]
#[napi]
impl Task for LangPackLoadTask {
    type Output = (
        sandbox::LangPackOutcome,
        Option<crate::lang_registry::RegistrationOutcome>,
    );
    type JsValue = LangPackLoadOutcome;

    fn compute(&mut self) -> Result<Self::Output> {
        let outcome = sandbox::invoke_lang_pack_load(&self.guest, &self.capabilities, self.policy)
            .map_err(sandbox_reason)?;
        let Some(bundle) = outcome.bundle.as_ref() else {
            return Ok((outcome, None));
        };
        let registration = crate::lang_registry::register(
            &bundle.name,
            &bundle.extensions,
            &bundle.grammar_wasm,
            &bundle.tag_queries,
            self.blessed,
        )
        .map_err(reason)?;
        Ok((outcome, Some(registration)))
    }

    fn resolve(&mut self, _env: Env, (o, registration): Self::Output) -> Result<Self::JsValue> {
        Ok(LangPackLoadOutcome {
            outcome: o.end.kind().to_string(),
            detail: o.end.detail(),
            fuel_used: i64::try_from(o.fuel_used).unwrap_or(i64::MAX),
            registered: registration.map(|r| LangPackRegistration {
                id: r.id,
                extensions: r.extensions,
                lane: r.lane.as_str().to_string(),
                queries_ok: r.queries_ok,
                notes: r.notes,
            }),
        })
    }
}

#[cfg(all(feature = "sandbox", feature = "lang-pack"))]
#[napi(ts_return_type = "Promise<LangPackLoadOutcome>")]
pub fn lang_pack_load(
    guest: Buffer,
    capabilities: Vec<String>,
    blessed: bool,
    policy: SandboxPolicyArg,
) -> AsyncTask<LangPackLoadTask> {
    AsyncTask::new(LangPackLoadTask {
        guest: guest.to_vec(),
        capabilities,
        blessed,
        policy: sandbox_policy(&policy),
    })
}

/// Batched classify-file outcome — mirrors `sandbox::LangPackClassifyOutcome`.
#[cfg(all(feature = "sandbox", feature = "lang-pack"))]
#[napi(object)]
pub struct LangPackClassifyOutcome {
    pub outcome: String,
    pub detail: String,
    pub fuel_used: i64,
    /// One verdict per input path: "source" | "test" | "vendor", or null
    /// where the pack deferred to the host.
    pub classes: Vec<Option<String>>,
}

#[cfg(all(feature = "sandbox", feature = "lang-pack"))]
pub struct LangPackClassifyTask {
    guest: Vec<u8>,
    paths: Vec<String>,
    capabilities: Vec<String>,
    policy: sandbox::SandboxPolicy,
}

#[cfg(all(feature = "sandbox", feature = "lang-pack"))]
#[napi]
impl Task for LangPackClassifyTask {
    type Output = sandbox::LangPackClassifyOutcome;
    type JsValue = LangPackClassifyOutcome;

    fn compute(&mut self) -> Result<Self::Output> {
        sandbox::invoke_lang_pack_classify(
            &self.guest,
            &self.paths,
            &self.capabilities,
            self.policy,
        )
        .map_err(sandbox_reason)
    }

    fn resolve(&mut self, _env: Env, o: Self::Output) -> Result<Self::JsValue> {
        Ok(LangPackClassifyOutcome {
            outcome: o.end.kind().to_string(),
            detail: o.end.detail(),
            fuel_used: i64::try_from(o.fuel_used).unwrap_or(i64::MAX),
            classes: o.classes,
        })
    }
}

#[cfg(all(feature = "sandbox", feature = "lang-pack"))]
#[napi(ts_return_type = "Promise<LangPackClassifyOutcome>")]
pub fn lang_pack_classify(
    guest: Buffer,
    paths: Vec<String>,
    capabilities: Vec<String>,
    policy: SandboxPolicyArg,
) -> AsyncTask<LangPackClassifyTask> {
    AsyncTask::new(LangPackClassifyTask {
        guest: guest.to_vec(),
        paths,
        capabilities,
        policy: sandbox_policy(&policy),
    })
}

/// Provenance snapshot of every registered pack language (map inventory).
#[cfg(feature = "lang-pack")]
#[napi]
pub fn lang_pack_snapshot() -> Vec<LangPackRegistration> {
    crate::lang_registry::snapshot()
        .into_iter()
        .map(|(id, extensions, lane, queries_ok)| LangPackRegistration {
            id,
            extensions,
            lane: lane.to_string(),
            queries_ok,
            notes: Vec::new(),
        })
        .collect()
}

/// The registered pack language claiming `ext` (no dot), if any. Callers
/// resolve bundled extensions first; registration already refuses shadowing.
#[cfg(feature = "lang-pack")]
#[napi]
pub fn lang_pack_language_for_extension(ext: String) -> Option<String> {
    crate::lang_registry::language_for_extension(&ext)
}

/// Drop every registered pack language (host-driven re-registration, tests).
#[cfg(feature = "lang-pack")]
#[napi]
pub fn lang_pack_clear() {
    crate::lang_registry::clear();
}

#[napi]
pub fn self_test() -> SelfTestReport {
    let r = engine::self_test();
    SelfTestReport {
        parse_ok: r.parse_ok,
        search_ok: r.search_ok,
        rewrite_ok: r.rewrite_ok,
        all_ok: r.all_ok(),
        details: r.details,
    }
}
