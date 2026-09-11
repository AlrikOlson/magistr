/**
 * workbench-core binding loader (SDD §3).
 *
 * One crate, two artifacts: the platform-native napi addon and the
 * wasm32-wasip1 fallback. Selection is a runtime decision made here, in the
 * adapter layer — try native, fail open to wasm — never a build-time switch.
 * The engine child (chunk m0-process-split) calls `loadVerifiedCore()` and
 * must not report ready unless the startup self-test exercised parse, search
 * and rewrite through whichever binding actually loaded.
 *
 * The load attempts are injectable so selection logic is unit-testable on
 * machines (and CI lanes) that carry neither artifact.
 */

import { existsSync } from "node:fs";
import { createRequire } from "node:module";
import path from "node:path";

const require = createRequire(import.meta.url);

/** Summary of one parse — mirrors the napi `ParseSummary` object. */
export interface CoreParseSummary {
  readonly language: string;
  readonly rootKind: string;
  readonly nodeCount: number;
  readonly errorCount: number;
  readonly hasError: boolean;
}

/** One matching line — mirrors the napi `SearchHit` object. */
export interface CoreSearchHit {
  readonly line: number;
  readonly start: number;
  readonly end: number;
  readonly lineText: string;
}

/** One structural match — mirrors the napi `StructuralHit` object. 1-based positions. */
export interface CoreStructuralHit {
  readonly line: number;
  readonly column: number;
  readonly endLine: number;
  /** tree-sitter node kind of the matched node (e.g. `call_expression`). */
  readonly kind: string;
  readonly lineText: string;
}

/** Outcome of a structural rewrite — mirrors the napi `RewriteOutcome` object. */
export interface CoreRewriteOutcome {
  readonly output: string;
  readonly matchCount: number;
}

/** One extracted symbol — mirrors the napi `SymbolRec` object. 1-based positions. */
export interface CoreSymbol {
  readonly name: string;
  readonly kind: string;
  readonly startLine: number;
  readonly startCol: number;
  readonly endLine: number;
  /** Trait a Rust impl member came from (specs/009); absent otherwise. */
  readonly detail?: string | null;
  /** First line of the item's doc comment (specs/009); absent otherwise. */
  readonly doc?: string | null;
}

/** One inclusive 1-based range of test-only code — mirrors the napi
 * `TestRangeRec` (specs/009 R4). */
export interface CoreTestRange {
  readonly startLine: number;
  readonly endLine: number;
}

/** Tier-1 extraction for one file — mirrors the napi `Extraction` object. */
export interface CoreExtraction {
  readonly symbols: readonly CoreSymbol[];
  readonly imports: readonly string[];
  /** Absent on the wasm-fallback lane and on older bindings. */
  readonly testRanges?: readonly CoreTestRange[];
}

/** One scanned file — mirrors the napi `ScanEntry` object. */
export interface CoreScanEntry {
  /** Workspace-relative, `/`-separated. */
  readonly path: string;
  readonly size: number;
  readonly mtimeMs: number;
  /** Vendor/generated: demoted in ranking, never excluded. */
  readonly junk: boolean;
}

/** How a sandboxed guest run ended — mirrors `GuestEnd::kind()` tags. */
export type CoreSandboxOutcomeKind =
  | "ok"
  | "out-of-fuel"
  | "memory-cap"
  | "cap-denied"
  | "guest-trap"
  | "guest-exit"
  | "invalid-guest";

/** The one guest resource policy — mirrors the napi `SandboxPolicyArg` object. */
export interface CoreSandboxPolicy {
  /** Fuel budget; exhaustion is a deterministic kill. */
  readonly fuel: number;
  /** Linear-memory cap in bytes; a grow past it traps the guest. */
  readonly memoryCapBytes: number;
}

/** Component-lane result — mirrors the napi `SandboxComponentOutcome` object. */
export interface CoreSandboxComponentOutcome {
  readonly outcome: CoreSandboxOutcomeKind;
  /** Denied import name, trap message, or invalid-guest reason. */
  readonly detail: string;
  readonly fuelUsed: number;
  /** Debug-rendered component values returned by the export. */
  readonly results: readonly string[];
  /** Values the guest passed to the granted `observe` capability. */
  readonly observations: readonly number[];
}

/** Component-lane JSON invoke result — mirrors the napi `SandboxInvokeOutcome`
 * object (SDD §8.3: plugin tool payloads are JSON strings). */
export interface CoreSandboxInvokeOutcome {
  readonly outcome: CoreSandboxOutcomeKind;
  /** Denied import name, trap message, or invalid-guest reason. */
  readonly detail: string;
  readonly fuelUsed: number;
  /** The export's string result, present iff outcome is "ok". */
  readonly result: string | null;
}

/** One host-curated language-server spec — mirrors the napi
 * `SubprocessSpecArg` object (chunk:m5-lsp-resolver-plugin). Only the host
 * constructs these; a guest addresses a `languageId`, never a binary. */
export interface CoreSubprocessSpec {
  readonly languageId: string;
  readonly bin: string;
  readonly args: readonly string[];
}

/** One lifted source-location — mirrors the napi `ResolvedLocationOut`. */
export interface CoreResolvedLocation {
  readonly path: string;
  readonly line: number;
  readonly column: number;
}

/** Typed resolver-lane result — mirrors the napi `SandboxResolverOutcome`. */
export interface CoreSandboxResolverOutcome {
  readonly outcome: CoreSandboxOutcomeKind;
  /** Denied import name, trap message, or invalid-guest reason. */
  readonly detail: string;
  readonly fuelUsed: number;
  /** Lifted locations, present (possibly empty) iff outcome is "ok". */
  readonly locations: readonly CoreResolvedLocation[];
  /** Language-server sessions this invoke spawned fresh (additive,
   * specs/007-resolver-server-pool FR-013). */
  readonly sessionsOpened: number;
  /** Warm pooled sessions this invoke reused instead of spawning. */
  readonly sessionsReused: number;
}

/** One pooled language-server session — mirrors the napi
 * `ServerPoolSessionOut` (specs/007-resolver-server-pool; tests/diagnostics). */
export interface CoreServerPoolSession {
  readonly languageId: string;
  readonly root: string;
  readonly pid: number;
  /** Held by an in-flight invoke right now. */
  readonly leased: boolean;
  /** Milliseconds since the session was last leased or returned. */
  readonly idleMs: number;
}

/** One lifted structured failure — mirrors the napi `StructuredFailureOut`. */
export interface CoreStructuredFailure {
  readonly testName: string;
  /** Absent/null both mean "no file" — napi lowers Option::None as undefined. */
  readonly file?: string | null;
  readonly assertion: string;
  readonly trace: string;
}

/** One lifted run-report — mirrors the napi `RunReportOut`. */
export interface CoreRunReport {
  readonly failures: readonly CoreStructuredFailure[];
  /** Absent/null both mean "no summary". */
  readonly summary?: string | null;
}

/** Typed runner-lane result — mirrors the napi `SandboxRunnerOutcome`. */
export interface CoreSandboxRunnerOutcome {
  readonly outcome: CoreSandboxOutcomeKind;
  /** Denied import name, trap message, or invalid-guest reason. */
  readonly detail: string;
  readonly fuelUsed: number;
  /** `can-parse`'s answer; false short-circuited without parse-output. */
  readonly claimed: boolean;
  /** Present iff ok + claimed + the guest extracted structure. */
  readonly report?: CoreRunReport | null;
}

/** One hit-context crossing into the ranking lane — mirrors the napi
 * `RankingHitArg` (chunk:m5-ranking-shadow). Enum fields travel as WIT case
 * names; the lane refuses unknown names before the sandbox runs. */
export interface CoreRankingHit {
  readonly file: string;
  /** 1-based, like SourceLocation. */
  readonly line: number;
  readonly column: number;
  readonly excerpt: string;
  /** declaration | call | import | comment | string | other */
  readonly kindClass: string;
  /** source | test | vendor */
  readonly fileClass: string;
}

/** One lifted feature — mirrors the napi `FeatureOut`. NOT yet namespaced;
 * the host prefixes `<plugin>/` before anything downstream sees the name. */
export interface CoreRankingFeature {
  readonly name: string;
  readonly value: number;
}

/** Per-hit feature list — mirrors the napi `HitFeaturesOut`. */
export interface CoreRankingHitFeatures {
  readonly features: readonly CoreRankingFeature[];
}

/** Typed ranking-lane result — mirrors the napi `SandboxRankingOutcome`. */
export interface CoreSandboxRankingOutcome {
  readonly outcome: CoreSandboxOutcomeKind;
  /** Denied import name, trap message, or invalid-guest reason. */
  readonly detail: string;
  readonly fuelUsed: number;
  /** Parallel to the input batch; shorter on a mid-batch failure (the host
   * keeps what was lifted — shadow scoring is additive). */
  readonly contributions: readonly CoreRankingHitFeatures[];
}

/** One registered pack language's provenance — mirrors the napi
 * `LangPackRegistration` object (chunk:m5-lang-pack). `lane` is the
 * promotion story made observable: "native" (blessed) | "wasm". */
export interface CoreLangPackRegistration {
  readonly id: string;
  readonly extensions: readonly string[];
  readonly lane: string;
  /** False when the pack's tag queries failed to compile: symbols degrade
   * to none, parse-verify stays live. */
  readonly queriesOk: boolean;
  /** Every shadowed claim or degradation, verbatim (FR-012). */
  readonly notes: readonly string[];
}

/** Carrier read + registration result — mirrors the napi
 * `LangPackLoadOutcome` object. */
export interface CoreLangPackLoadOutcome {
  readonly outcome: CoreSandboxOutcomeKind;
  readonly detail: string;
  readonly fuelUsed: number;
  /** Present iff the carrier read succeeded AND registration was accepted.
   * Absent/null both mean "not registered" — napi lowers Option::None as
   * undefined. */
  readonly registered?: CoreLangPackRegistration | null;
}

/** Batched classify-file result — mirrors the napi `LangPackClassifyOutcome`
 * object. One verdict per input path; null = the pack deferred. */
export interface CoreLangPackClassifyOutcome {
  readonly outcome: CoreSandboxOutcomeKind;
  readonly detail: string;
  readonly fuelUsed: number;
  readonly classes: readonly (string | null)[];
}

/** wasi-lane result — mirrors the napi `SandboxWasiOutcome` object. */
export interface CoreSandboxWasiOutcome {
  readonly outcome: CoreSandboxOutcomeKind;
  readonly detail: string;
  readonly fuelUsed: number;
  readonly exitCode: number | null;
  readonly stdout: string;
  readonly stderr: string;
}

/** Startup self-test result — mirrors the napi `SelfTestReport` object. */
export interface CoreSelfTestReport {
  readonly parseOk: boolean;
  readonly searchOk: boolean;
  readonly rewriteOk: boolean;
  readonly allOk: boolean;
  readonly details: readonly string[];
}

/** The workbench-core FFI surface, identical for native and wasm builds. */
export interface CoreBinding {
  version(): string;
  languages(): string[];
  parse(language: string, source: string): CoreParseSummary;
  search(pattern: string, haystack: string, caseInsensitive?: boolean): CoreSearchHit[];
  /** ast-grep structural match (`$VAR` metavariables) with node kinds. */
  structuralSearch(language: string, source: string, pattern: string): CoreStructuralHit[];
  rewrite(
    language: string,
    source: string,
    pattern: string,
    replacement: string,
  ): CoreRewriteOutcome;
  /** Async: runs on a worker thread, one parser arena per task (SDD §7.3.2). */
  extract(language: string, source: string): Promise<CoreExtraction>;
  /** Async: gitignore-aware workspace walk with junk flagging (SDD §7.3.4). */
  scanWorkspace(root: string): Promise<CoreScanEntry[]>;
  selfTest(): CoreSelfTestReport;
  /**
   * Wasmtime extension host (SDD §8.1) — the three members below exist on the
   * native binding only. Wasmtime cannot target wasm32, so the wasm fallback
   * lane lacks them by construction; absence means "extensions unavailable in
   * this degraded lane" (§6.2), and callers must probe before use.
   */
  sandboxCapabilities?(): string[];
  sandboxRunComponent?(
    guest: Uint8Array,
    exportName: string,
    args: number[],
    capabilities: string[],
    policy: CoreSandboxPolicy,
  ): Promise<CoreSandboxComponentOutcome>;
  sandboxInvokeComponent?(
    guest: Uint8Array,
    exportName: string,
    payload: string,
    capabilities: string[],
    policy: CoreSandboxPolicy,
    subprocessSpecs?: CoreSubprocessSpec[],
    subprocessRoot?: string,
  ): Promise<CoreSandboxInvokeOutcome>;
  /** Typed resolver lane (chunk:m5-lsp-resolver-plugin): invoke a
   * resolver-provider component's resolve-references export. */
  sandboxResolveReferences?(
    guest: Uint8Array,
    symbol: string,
    capabilities: string[],
    policy: CoreSandboxPolicy,
    subprocessSpecs?: CoreSubprocessSpec[],
    subprocessRoot?: string,
  ): Promise<CoreSandboxResolverOutcome>;
  /** Kill every warm pooled language server (specs/007-resolver-server-pool).
   * The engine child calls this at its normal exit. Returns the count. */
  sandboxServerPoolShutdown?(): number;
  /** Snapshot of the pooled language-server sessions (tests/diagnostics). */
  sandboxServerPoolStats?(): CoreServerPoolSession[];
  /** Typed runner lane (chunk:m5-runner-adapter): can-parse + parse-output
   * of a runner-provider component. */
  sandboxRunnerParse?(
    guest: Uint8Array,
    command: string,
    exitCode: number,
    stdout: string,
    stderr: string,
    capabilities: string[],
    policy: CoreSandboxPolicy,
  ): Promise<CoreSandboxRunnerOutcome>;
  /** Typed ranking lane (chunk:m5-ranking-shadow): batched contribute over
   * a ranking-provider component — one instantiation per batch. Off the
   * search response path ONLY (research R2 / SC-005). */
  sandboxRankingContribute?(
    guest: Uint8Array,
    hits: CoreRankingHit[],
    capabilities: string[],
    policy: CoreSandboxPolicy,
  ): Promise<CoreSandboxRankingOutcome>;
  sandboxRunWasiModule?(
    guest: Uint8Array,
    argv: string[],
    policy: CoreSandboxPolicy,
  ): Promise<CoreSandboxWasiOutcome>;
  /** Lang-pack carrier lane (chunk:m5-lang-pack): read a
   * lang-pack-provider's exports and register the language into the dynamic
   * registry in one call — grammar bytes never cross the FFI twice.
   * `blessed` selects the execution lane (research R5). */
  langPackLoad?(
    guest: Uint8Array,
    capabilities: string[],
    blessed: boolean,
    policy: CoreSandboxPolicy,
  ): Promise<CoreLangPackLoadOutcome>;
  /** Batched classify-file over one carrier instantiation. */
  langPackClassify?(
    guest: Uint8Array,
    paths: string[],
    capabilities: string[],
    policy: CoreSandboxPolicy,
  ): Promise<CoreLangPackClassifyOutcome>;
  /** Provenance snapshot of every registered pack language. */
  langPackSnapshot?(): CoreLangPackRegistration[];
  /** The registered pack language claiming `ext` (no dot), if any. */
  langPackLanguageForExtension?(ext: string): string | null;
  /** Drop every registered pack language (re-registration, tests). */
  langPackClear?(): void;
}

export type CoreFlavor = "native" | "wasm";

export interface LoadedCore {
  readonly binding: CoreBinding;
  readonly flavor: CoreFlavor;
  /** Present when the native attempt failed and wasm was selected. */
  readonly nativeError?: string;
}

/** Injectable load attempts; each returns a binding or throws. */
export interface CoreLoadAttempts {
  readonly tryNative: () => CoreBinding;
  readonly tryWasm: () => CoreBinding;
}

export class CoreLoadError extends Error {
  constructor(
    readonly nativeError: string,
    readonly wasmError: string,
  ) {
    super(`workbench-core failed to load. native: ${nativeError}; wasm fallback: ${wasmError}`);
    this.name = "CoreLoadError";
  }
}

export class CoreSelfTestError extends Error {
  constructor(
    readonly flavor: CoreFlavor,
    readonly report: CoreSelfTestReport,
  ) {
    super(
      `workbench-core (${flavor}) failed its startup self-test: ` +
        `parse=${report.parseOk} search=${report.searchOk} rewrite=${report.rewriteOk}` +
        (report.details.length > 0 ? ` — ${report.details.join("; ")}` : ""),
    );
    this.name = "CoreSelfTestError";
  }
}

/** napi-rs artifact key for the running platform, e.g. `win32-x64-msvc`. */
export function nativeArtifactKey(
  platform: NodeJS.Platform = process.platform,
  arch: string = process.arch,
): string {
  const base = `${platform}-${arch}`;
  if (platform === "win32") return `${base}-msvc`;
  if (platform === "linux") return `${base}-gnu`;
  return base;
}

/** Repo-relative default location of built artifacts. */
export function defaultArtifactsDir(): string {
  return path.resolve(import.meta.dirname, "../../../crates/workbench-core/artifacts");
}

function defaultTryNative(artifactsDir: string): CoreBinding {
  const file = path.join(artifactsDir, `workbench-core.${nativeArtifactKey()}.node`);
  if (!existsSync(file)) {
    throw new Error(`native addon not found: ${file}`);
  }
  return require(file) as CoreBinding;
}

function defaultTryWasm(artifactsDir: string): CoreBinding {
  const wasmFile = path.join(artifactsDir, "workbench-core.wasm32-wasi.wasm");
  if (!existsSync(wasmFile)) {
    throw new Error(`wasm fallback not found: ${wasmFile}`);
  }
  const loaderPath = path.resolve(
    import.meta.dirname,
    "../../../crates/workbench-core/loader/wasi-bun.cjs",
  );
  const { loadWorkbenchCoreWasm } = require(loaderPath) as {
    loadWorkbenchCoreWasm: (p: string) => CoreBinding;
  };
  return loadWorkbenchCoreWasm(wasmFile);
}

export interface LoadCoreOptions {
  readonly attempts?: Partial<CoreLoadAttempts>;
  /** Where the built artifacts live; defaults to the in-repo build output. */
  readonly artifactsDir?: string;
  /**
   * Force one flavor (no fallback). Mirrors the WORKBENCH_CORE_FORCE
   * environment variable; the explicit option wins over the environment.
   */
  readonly force?: CoreFlavor;
}

function forcedFlavor(options: LoadCoreOptions): CoreFlavor | undefined {
  if (options.force !== undefined) return options.force;
  const env = process.env["WORKBENCH_CORE_FORCE"];
  if (env === "native" || env === "wasm") return env;
  return undefined;
}

/**
 * Load the core binding: native first, wasm on native failure. Throws
 * {@link CoreLoadError} only when both attempts fail.
 */
export function loadCore(options: LoadCoreOptions = {}): LoadedCore {
  const artifactsDir = options.artifactsDir ?? defaultArtifactsDir();
  const tryNative = options.attempts?.tryNative ?? (() => defaultTryNative(artifactsDir));
  const tryWasm = options.attempts?.tryWasm ?? (() => defaultTryWasm(artifactsDir));

  const force = forcedFlavor(options);
  if (force === "native") {
    return { binding: tryNative(), flavor: "native" };
  }
  if (force === "wasm") {
    return { binding: tryWasm(), flavor: "wasm" };
  }

  let nativeError: string;
  try {
    return { binding: tryNative(), flavor: "native" };
  } catch (error) {
    nativeError = error instanceof Error ? error.message : String(error);
  }
  try {
    return { binding: tryWasm(), flavor: "wasm", nativeError };
  } catch (error) {
    const wasmError = error instanceof Error ? error.message : String(error);
    throw new CoreLoadError(nativeError, wasmError);
  }
}

/**
 * Run the startup self-test on a loaded binding; throws
 * {@link CoreSelfTestError} unless parse, search and rewrite all passed.
 */
export function verifyCore(loaded: LoadedCore): LoadedCore {
  const report = loaded.binding.selfTest();
  if (!report.allOk) {
    throw new CoreSelfTestError(loaded.flavor, report);
  }
  return loaded;
}

/**
 * The engine child's entrypoint: load (native → wasm fail-open) and refuse to
 * hand back a binding whose self-test did not exercise parse, search and
 * rewrite successfully.
 */
export function loadVerifiedCore(options: LoadCoreOptions = {}): LoadedCore {
  return verifyCore(loadCore(options));
}
