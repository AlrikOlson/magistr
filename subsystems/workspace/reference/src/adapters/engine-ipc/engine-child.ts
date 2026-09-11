/**
 * The engine child — the `--engine` role of the one workbench-mcp binary
 * (SDD §6.1). Owns every code path that feeds untrusted bytes to a native
 * parser: it loads workbench-core (native → wasm fail-open), refuses to
 * report ready unless the startup self-test passed, then serves framed
 * requests over stdin/stdout. Request-scoped and shared-nothing: every
 * request carries all of its input on the frame.
 *
 * Test hooks, both env-gated and inert in production:
 *  - WORKBENCH_ENGINE_STUB_CORE=1 swaps the core binding for a trivial JS
 *    stub so IPC/supervision suites run without built artifacts — the
 *    process, pipes and kills stay real, only the parser is stubbed;
 *  - WORKBENCH_ENGINE_KILL_ON=<method|selftest> (optionally with
 *    WORKBENCH_ENGINE_KILL_MATCH=<substring>) makes the child die abruptly
 *    mid-operation, which is how the chaos suite plants kill points.
 */

import { readFile, stat } from "node:fs/promises";
import type {
  CoreBinding,
  CoreExtraction,
  CoreLangPackClassifyOutcome,
  CoreLangPackLoadOutcome,
  CoreParseSummary,
  CoreRankingHit,
  CoreRewriteOutcome,
  CoreSandboxInvokeOutcome,
  CoreSandboxRankingOutcome,
  CoreSandboxResolverOutcome,
  CoreSandboxRunnerOutcome,
  CoreSandboxWasiOutcome,
  CoreSearchHit,
  CoreSelfTestReport,
  CoreStructuralHit,
  CoreSubprocessSpec,
  CoreSymbol,
  LoadedCore,
} from "../core";
import { loadVerifiedCore } from "../core";
import {
  encodeFrame,
  type Frame,
  FrameDecoder,
  type FrameError,
  IPC_VERSION,
  type RequestFrame,
} from "./frames";
import {
  clearPackLanguages,
  fallbackScanWorkspace,
  IndexerHost,
  isIndexMethod,
  languageForPath,
  registerPackLanguage,
} from "./indexer";

export { clearPackLanguages, languageForPath, registerPackLanguage };

/**
 * The stateless core methods the child serves. The chaos suite sweeps this
 * table; the stateful `index/*` methods live in `./indexer` (INDEX_METHODS).
 */
export const ENGINE_METHODS = ["ping", "parse", "search", "structural", "rewrite"] as const;
export type EngineMethod = (typeof ENGINE_METHODS)[number];

/** Exit code for a planted kill — distinguishable from real crashes in logs. */
export const CHAOS_EXIT_CODE = 86;

/**
 * The `script` guest lane (SDD §9.7, chunk:m4-script-sandbox): one async
 * method executing a wasi-p1 module under the §8.1 sandbox policy. Kept out
 * of ENGINE_METHODS deliberately — that table is the chaos suite's sweep of
 * the stateless sync parse surface; this method is async, artifact-backed,
 * and only exists on the native binding.
 */
export const SANDBOX_WASI_METHOD = "sandbox/wasi";

/**
 * The plugin-tool guest lane (SDD §8.3, chunk:m4-capability-manifests): one
 * async method invoking a `(string) -> string` export of a component-model
 * guest under the §8.1 sandbox policy, with the granted import names as the
 * only door (default deny, decided before instantiation). Kept out of
 * ENGINE_METHODS for the same reason as the wasi lane.
 */
export const SANDBOX_COMPONENT_METHOD = "sandbox/component";

export interface ChildSandboxComponentParams {
  /** Absolute path of the plugin's component wasm (jail-checked upstream). */
  readonly wasmPath: string;
  /** Export address: `"invoke"` or `"pkg:iface/name@ver#invoke"`. */
  readonly exportName: string;
  /** The JSON payload string handed to the export. */
  readonly payload: string;
  /** Granted import names — the install-time grant record, already
   * intersected with the manifest's declared capabilities. */
  readonly capabilities: readonly string[];
  readonly fuel: number;
  readonly memoryCapBytes: number;
  /** Host-curated language-server specs backing a granted subprocess
   * capability (chunk:m5-lsp-resolver-plugin). Absent = empty table. */
  readonly subprocessSpecs?: readonly CoreSubprocessSpec[];
  /** Workspace root reported to the guest via `workspace-root` (informational). */
  readonly subprocessRoot?: string;
}

/**
 * The typed resolver lane (chunk:m5-lsp-resolver-plugin): one async method
 * invoking a resolver-provider component's `resolve-references` export under
 * the same §8.1 policy and grant model as the plugin-tool lane.
 */
export const SANDBOX_RESOLVER_METHOD = "sandbox/resolver";

export interface ChildSandboxResolverParams {
  /** Absolute path of the plugin's component wasm (jail-checked upstream). */
  readonly wasmPath: string;
  /** The symbol whose references the guest resolves. */
  readonly symbol: string;
  /** Granted import names (manifest ∩ grants, blessed-tier enforced upstream). */
  readonly capabilities: readonly string[];
  readonly fuel: number;
  readonly memoryCapBytes: number;
  /** Host-curated language-server specs (the detection table). */
  readonly subprocessSpecs?: readonly CoreSubprocessSpec[];
  /** Workspace root reported to the guest via `workspace-root` (informational). */
  readonly subprocessRoot?: string;
}

/**
 * The lang-pack carrier lane (chunk:m5-lang-pack): read a pack's exports and
 * register its language into the child's dynamic registry in one call —
 * grammar bytes stay inside the child. `blessed` selects the execution lane
 * (specs/002-wasm-lang-pack/research.md R5). On success the child also
 * records the pack's extension claims so `languageForPath` — and with it
 * edit parse-verify and Tier-1 indexing — resolves the pack language.
 */
export const SANDBOX_LANG_PACK_METHOD = "sandbox/langPack";

export interface ChildSandboxLangPackParams {
  /** Absolute path of the pack's component wasm (jail-checked upstream). */
  readonly wasmPath: string;
  /** Trust-tier verdict (tier === "blessed"), decided upstream in plugins.ts. */
  readonly blessed: boolean;
  /** Granted import names — normally empty: a lang pack declares nothing. */
  readonly capabilities: readonly string[];
  readonly fuel: number;
  readonly memoryCapBytes: number;
}

/** Batched classify-file over one carrier instantiation (ranking stage 2). */
export const SANDBOX_LANG_PACK_CLASSIFY_METHOD = "sandbox/langPackClassify";

/**
 * The typed runner lane (chunk:m5-runner-adapter): one async method invoking
 * a runner-provider component's can-parse + parse-output under the same
 * §8.1 policy and grant model as the other component lanes.
 */
export const SANDBOX_RUNNER_METHOD = "sandbox/runner";

export interface ChildSandboxRunnerParams {
  /** Absolute path of the plugin's component wasm (jail-checked upstream). */
  readonly wasmPath: string;
  /** The resolved command line (can-parse's input). */
  readonly command: string;
  readonly exitCode: number;
  readonly stdout: string;
  readonly stderr: string;
  /** Granted import names (manifest ∩ grants; empty for pure adapters). */
  readonly capabilities: readonly string[];
  readonly fuel: number;
  readonly memoryCapBytes: number;
}

/**
 * The typed ranking lane (chunk:m5-ranking-shadow): one async method invoking
 * a ranking-provider component's contribute over a batch of hit-contexts —
 * one instantiation per batch, same §8.1 policy and grant model as the other
 * component lanes. Callers keep it OFF the search response path (SC-005).
 */
export const SANDBOX_RANKING_METHOD = "sandbox/ranking";

export interface ChildSandboxRankingParams {
  /** Absolute path of the plugin's component wasm (jail-checked upstream). */
  readonly wasmPath: string;
  readonly hits: readonly CoreRankingHit[];
  /** Granted import names (manifest ∩ grants; empty for pure signals). */
  readonly capabilities: readonly string[];
  readonly fuel: number;
  readonly memoryCapBytes: number;
}

export interface ChildSandboxWasiParams {
  /** Absolute path of the guest wasm artifact (host-owned, not workspace). */
  readonly wasmPath: string;
  readonly argv: readonly string[];
  readonly fuel: number;
  readonly memoryCapBytes: number;
}

export interface ChildParseParams {
  readonly path: string;
  readonly source: string;
}

export interface ChildSearchParams {
  readonly pattern: string;
  readonly haystack: string;
  readonly path: string;
}

export interface ChildStructuralParams {
  readonly pattern: string;
  readonly source: string;
  readonly path: string;
}

export interface ChildRewriteParams {
  readonly path: string;
  readonly source: string;
  readonly edits: readonly { readonly find: string; readonly replace: string }[];
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}

class MethodError extends Error {
  constructor(
    readonly code: string,
    message: string,
  ) {
    super(message);
    this.name = "MethodError";
  }
}

function requireString(params: Record<string, unknown>, field: string): string {
  const value = params[field];
  if (typeof value !== "string") {
    throw new MethodError("MALFORMED_FRAME", `params.${field} must be a string`);
  }
  return value;
}

function handleParse(binding: CoreBinding, params: ChildParseParams): unknown {
  const language = languageForPath(params.path);
  if (language === undefined || !binding.languages().includes(language)) {
    // Not a known source language (bundled or registered pack): nothing to
    // parse-verify.
    return { ok: true };
  }
  const summary: CoreParseSummary = binding.parse(language, params.source);
  return summary.hasError
    ? {
        ok: false,
        parserMessage: `${language} parse found ${summary.errorCount} syntax error node(s)`,
      }
    : { ok: true };
}

function handleSearch(binding: CoreBinding, params: ChildSearchParams): unknown {
  const hits: CoreSearchHit[] = binding.search(params.pattern, params.haystack);
  return { hits };
}

function handleStructural(binding: CoreBinding, params: ChildStructuralParams): unknown {
  const language = languageForPath(params.path);
  if (language === undefined || !binding.languages().includes(language)) {
    // Not a bundled source language: structurally nothing can match.
    return { hits: [] };
  }
  return { hits: binding.structuralSearch(language, params.source, params.pattern) };
}

function handleRewrite(binding: CoreBinding, params: ChildRewriteParams): unknown {
  const language = languageForPath(params.path);
  const structural = language !== undefined && binding.languages().includes(language);
  let content = params.source;
  for (const edit of params.edits) {
    if (structural) {
      const outcome: CoreRewriteOutcome = binding.rewrite(
        language,
        content,
        edit.find,
        edit.replace,
      );
      content = outcome.output;
    } else {
      content = content.replaceAll(edit.find, edit.replace);
    }
  }
  return { content, changed: content !== params.source };
}

/** Execute one request against the loaded binding. Pure of process wiring. */
export function handleRequest(binding: CoreBinding, method: string, params: unknown): unknown {
  const record = isRecord(params) ? params : {};
  switch (method) {
    case "ping":
      return { pong: true };
    case "parse":
      return handleParse(binding, {
        path: requireString(record, "path"),
        source: requireString(record, "source"),
      });
    case "search":
      return handleSearch(binding, {
        pattern: requireString(record, "pattern"),
        haystack: requireString(record, "haystack"),
        path: requireString(record, "path"),
      });
    case "structural":
      return handleStructural(binding, {
        pattern: requireString(record, "pattern"),
        source: requireString(record, "source"),
        path: requireString(record, "path"),
      });
    case "rewrite": {
      const edits = record["edits"];
      if (!Array.isArray(edits)) {
        throw new MethodError("MALFORMED_FRAME", "params.edits must be an array");
      }
      return handleRewrite(binding, {
        path: requireString(record, "path"),
        source: requireString(record, "source"),
        edits: edits.map((edit: unknown) => {
          const e = isRecord(edit) ? edit : {};
          return { find: requireString(e, "find"), replace: requireString(e, "replace") };
        }),
      });
    }
    default:
      throw new MethodError("MALFORMED_FRAME", `unknown method ${JSON.stringify(method)}`);
  }
}

/**
 * Guest-artifact cache: the qjs interpreter is ~1.5MB and identical across
 * rounds of one script's re-entry loop, so bytes are cached per path and
 * invalidated by mtime+size — never by content trust (the artifact is
 * host-owned, not workspace content).
 */
const wasmCache = new Map<string, { mtimeMs: number; size: number; bytes: Uint8Array }>();

async function loadGuestWasm(wasmPath: string): Promise<Uint8Array> {
  const s = await stat(wasmPath);
  const cached = wasmCache.get(wasmPath);
  if (cached !== undefined && cached.mtimeMs === s.mtimeMs && cached.size === s.size) {
    return cached.bytes;
  }
  const bytes = new Uint8Array(await readFile(wasmPath));
  wasmCache.set(wasmPath, { mtimeMs: s.mtimeMs, size: s.size, bytes });
  return bytes;
}

/**
 * Execute one sandboxed wasi guest round (SDD §9.7). A binding without the
 * sandbox surface (wasm-fallback or stub lane, §6.2) answers a typed
 * SANDBOX_UNAVAILABLE frame error — degraded lanes honestly lack `script`.
 */
export async function handleSandboxWasi(
  binding: CoreBinding,
  params: unknown,
): Promise<CoreSandboxWasiOutcome> {
  const record = isRecord(params) ? params : {};
  const wasmPath = requireString(record, "wasmPath");
  const argvRaw = record["argv"];
  if (!Array.isArray(argvRaw) || argvRaw.some((a) => typeof a !== "string")) {
    throw new MethodError("MALFORMED_FRAME", "params.argv must be an array of strings");
  }
  const fuel = record["fuel"];
  const memoryCapBytes = record["memoryCapBytes"];
  if (typeof fuel !== "number" || !Number.isInteger(fuel) || fuel < 1) {
    throw new MethodError("MALFORMED_FRAME", "params.fuel must be a positive integer");
  }
  if (
    typeof memoryCapBytes !== "number" ||
    !Number.isInteger(memoryCapBytes) ||
    memoryCapBytes < 1
  ) {
    throw new MethodError("MALFORMED_FRAME", "params.memoryCapBytes must be a positive integer");
  }
  if (binding.sandboxRunWasiModule === undefined) {
    throw new MethodError(
      "SANDBOX_UNAVAILABLE",
      "this core binding lacks the Wasmtime sandbox (wasm-fallback or stub lane, SDD §6.2)",
    );
  }
  let bytes: Uint8Array;
  try {
    bytes = await loadGuestWasm(wasmPath);
  } catch (error) {
    throw new MethodError(
      "SANDBOX_UNAVAILABLE",
      `guest wasm artifact unreadable: ${error instanceof Error ? error.message : String(error)}`,
    );
  }
  return binding.sandboxRunWasiModule(bytes, argvRaw as string[], { fuel, memoryCapBytes });
}

/**
 * Invoke one plugin-tool export (SDD §8.3). Same degradation contract as the
 * wasi lane: a binding without the sandbox surface answers a typed
 * SANDBOX_UNAVAILABLE frame error rather than emulating.
 */
export async function handleSandboxComponent(
  binding: CoreBinding,
  params: unknown,
): Promise<CoreSandboxInvokeOutcome> {
  const record = isRecord(params) ? params : {};
  const wasmPath = requireString(record, "wasmPath");
  const exportName = requireString(record, "exportName");
  const payload = requireString(record, "payload");
  const capabilitiesRaw = record["capabilities"];
  if (!Array.isArray(capabilitiesRaw) || capabilitiesRaw.some((c) => typeof c !== "string")) {
    throw new MethodError("MALFORMED_FRAME", "params.capabilities must be an array of strings");
  }
  for (const key of ["fuel", "memoryCapBytes"] as const) {
    const value = record[key];
    if (typeof value !== "number" || !Number.isInteger(value) || value < 1) {
      throw new MethodError("MALFORMED_FRAME", `params.${key} must be a positive integer`);
    }
  }
  if (binding.sandboxInvokeComponent === undefined) {
    throw new MethodError(
      "SANDBOX_UNAVAILABLE",
      "this core binding lacks the Wasmtime sandbox (wasm-fallback or stub lane, SDD §6.2)",
    );
  }
  let bytes: Uint8Array;
  try {
    bytes = await loadGuestWasm(wasmPath);
  } catch (error) {
    throw new MethodError(
      "SANDBOX_UNAVAILABLE",
      `plugin component unreadable: ${error instanceof Error ? error.message : String(error)}`,
    );
  }
  return binding.sandboxInvokeComponent(
    bytes,
    exportName,
    payload,
    capabilitiesRaw as string[],
    {
      fuel: record["fuel"] as number,
      memoryCapBytes: record["memoryCapBytes"] as number,
    },
    subprocessSpecsOf(record),
    subprocessRootOf(record),
  );
}

/** Validate the optional workspace root (absent = empty string). */
function subprocessRootOf(record: Record<string, unknown>): string {
  const raw = record["subprocessRoot"];
  if (raw === undefined) return "";
  if (typeof raw !== "string") {
    throw new MethodError("MALFORMED_FRAME", "params.subprocessRoot must be a string");
  }
  return raw;
}

/** Validate the optional host-curated spec table (absent = empty). */
function subprocessSpecsOf(record: Record<string, unknown>): CoreSubprocessSpec[] {
  const raw = record["subprocessSpecs"];
  if (raw === undefined) return [];
  if (!Array.isArray(raw)) {
    throw new MethodError("MALFORMED_FRAME", "params.subprocessSpecs must be an array");
  }
  return raw.map((entry, i) => {
    const spec = isRecord(entry) ? entry : {};
    const languageId = spec["languageId"];
    const bin = spec["bin"];
    const args = spec["args"];
    if (typeof languageId !== "string" || typeof bin !== "string") {
      throw new MethodError(
        "MALFORMED_FRAME",
        `params.subprocessSpecs[${i}] must carry string languageId and bin`,
      );
    }
    if (!Array.isArray(args) || args.some((a) => typeof a !== "string")) {
      throw new MethodError(
        "MALFORMED_FRAME",
        `params.subprocessSpecs[${i}].args must be an array of strings`,
      );
    }
    return { languageId, bin, args: args as string[] };
  });
}

/**
 * Invoke one typed runner parse (chunk:m5-runner-adapter). Same degradation
 * contract as the other sandbox lanes.
 */
export async function handleSandboxRunner(
  binding: CoreBinding,
  params: unknown,
): Promise<CoreSandboxRunnerOutcome> {
  const record = isRecord(params) ? params : {};
  const wasmPath = requireString(record, "wasmPath");
  const command = requireString(record, "command");
  const stdout = requireString(record, "stdout");
  const stderr = requireString(record, "stderr");
  const exitCode = record["exitCode"];
  if (typeof exitCode !== "number" || !Number.isInteger(exitCode)) {
    throw new MethodError("MALFORMED_FRAME", "params.exitCode must be an integer");
  }
  const capabilitiesRaw = record["capabilities"];
  if (!Array.isArray(capabilitiesRaw) || capabilitiesRaw.some((c) => typeof c !== "string")) {
    throw new MethodError("MALFORMED_FRAME", "params.capabilities must be an array of strings");
  }
  for (const key of ["fuel", "memoryCapBytes"] as const) {
    const value = record[key];
    if (typeof value !== "number" || !Number.isInteger(value) || value < 1) {
      throw new MethodError("MALFORMED_FRAME", `params.${key} must be a positive integer`);
    }
  }
  if (binding.sandboxRunnerParse === undefined) {
    throw new MethodError(
      "SANDBOX_UNAVAILABLE",
      "this core binding lacks the Wasmtime sandbox (wasm-fallback or stub lane, SDD §6.2)",
    );
  }
  let bytes: Uint8Array;
  try {
    bytes = await loadGuestWasm(wasmPath);
  } catch (error) {
    throw new MethodError(
      "SANDBOX_UNAVAILABLE",
      `plugin component unreadable: ${error instanceof Error ? error.message : String(error)}`,
    );
  }
  return binding.sandboxRunnerParse(
    bytes,
    command,
    exitCode,
    stdout,
    stderr,
    capabilitiesRaw as string[],
    {
      fuel: record["fuel"] as number,
      memoryCapBytes: record["memoryCapBytes"] as number,
    },
  );
}

/**
 * Invoke one typed batched ranking contribute (chunk:m5-ranking-shadow).
 * Same degradation contract as the other sandbox lanes.
 */
export async function handleSandboxRanking(
  binding: CoreBinding,
  params: unknown,
): Promise<CoreSandboxRankingOutcome> {
  const record = isRecord(params) ? params : {};
  const wasmPath = requireString(record, "wasmPath");
  const hitsRaw = record["hits"];
  if (!Array.isArray(hitsRaw)) {
    throw new MethodError("MALFORMED_FRAME", "params.hits must be an array");
  }
  const hits: CoreRankingHit[] = hitsRaw.map((entry, i) => {
    const hit = isRecord(entry) ? entry : {};
    const file = hit["file"];
    const line = hit["line"];
    const column = hit["column"];
    const excerpt = hit["excerpt"];
    const kindClass = hit["kindClass"];
    const fileClass = hit["fileClass"];
    if (
      typeof file !== "string" ||
      typeof excerpt !== "string" ||
      typeof kindClass !== "string" ||
      typeof fileClass !== "string" ||
      typeof line !== "number" ||
      !Number.isInteger(line) ||
      typeof column !== "number" ||
      !Number.isInteger(column)
    ) {
      throw new MethodError(
        "MALFORMED_FRAME",
        `params.hits[${i}] must carry file/excerpt/kindClass/fileClass strings and integer line/column`,
      );
    }
    return { file, line, column, excerpt, kindClass, fileClass };
  });
  const capabilitiesRaw = record["capabilities"];
  if (!Array.isArray(capabilitiesRaw) || capabilitiesRaw.some((c) => typeof c !== "string")) {
    throw new MethodError("MALFORMED_FRAME", "params.capabilities must be an array of strings");
  }
  for (const key of ["fuel", "memoryCapBytes"] as const) {
    const value = record[key];
    if (typeof value !== "number" || !Number.isInteger(value) || value < 1) {
      throw new MethodError("MALFORMED_FRAME", `params.${key} must be a positive integer`);
    }
  }
  if (binding.sandboxRankingContribute === undefined) {
    throw new MethodError(
      "SANDBOX_UNAVAILABLE",
      "this core binding lacks the Wasmtime sandbox (wasm-fallback or stub lane, SDD §6.2)",
    );
  }
  let bytes: Uint8Array;
  try {
    bytes = await loadGuestWasm(wasmPath);
  } catch (error) {
    throw new MethodError(
      "SANDBOX_UNAVAILABLE",
      `plugin component unreadable: ${error instanceof Error ? error.message : String(error)}`,
    );
  }
  return binding.sandboxRankingContribute(bytes, hits, capabilitiesRaw as string[], {
    fuel: record["fuel"] as number,
    memoryCapBytes: record["memoryCapBytes"] as number,
  });
}

/**
 * Invoke one typed resolver export (chunk:m5-lsp-resolver-plugin). Same
 * degradation contract as the other sandbox lanes.
 */
export async function handleSandboxResolver(
  binding: CoreBinding,
  params: unknown,
): Promise<CoreSandboxResolverOutcome> {
  const record = isRecord(params) ? params : {};
  const wasmPath = requireString(record, "wasmPath");
  const symbol = requireString(record, "symbol");
  const capabilitiesRaw = record["capabilities"];
  if (!Array.isArray(capabilitiesRaw) || capabilitiesRaw.some((c) => typeof c !== "string")) {
    throw new MethodError("MALFORMED_FRAME", "params.capabilities must be an array of strings");
  }
  for (const key of ["fuel", "memoryCapBytes"] as const) {
    const value = record[key];
    if (typeof value !== "number" || !Number.isInteger(value) || value < 1) {
      throw new MethodError("MALFORMED_FRAME", `params.${key} must be a positive integer`);
    }
  }
  if (binding.sandboxResolveReferences === undefined) {
    throw new MethodError(
      "SANDBOX_UNAVAILABLE",
      "this core binding lacks the Wasmtime sandbox (wasm-fallback or stub lane, SDD §6.2)",
    );
  }
  let bytes: Uint8Array;
  try {
    bytes = await loadGuestWasm(wasmPath);
  } catch (error) {
    throw new MethodError(
      "SANDBOX_UNAVAILABLE",
      `plugin component unreadable: ${error instanceof Error ? error.message : String(error)}`,
    );
  }
  return binding.sandboxResolveReferences(
    bytes,
    symbol,
    capabilitiesRaw as string[],
    {
      fuel: record["fuel"] as number,
      memoryCapBytes: record["memoryCapBytes"] as number,
    },
    subprocessSpecsOf(record),
    subprocessRootOf(record),
  );
}

/**
 * Load one lang pack's carrier exports and register its language
 * (chunk:m5-lang-pack). Same degradation contract as the other sandbox
 * lanes; on successful registration the child's extension map is updated so
 * languageForPath resolves the pack language from this frame on.
 */
export async function handleSandboxLangPack(
  binding: CoreBinding,
  params: unknown,
): Promise<CoreLangPackLoadOutcome> {
  const record = isRecord(params) ? params : {};
  const wasmPath = requireString(record, "wasmPath");
  const blessed = record["blessed"];
  if (typeof blessed !== "boolean") {
    throw new MethodError("MALFORMED_FRAME", "params.blessed must be a boolean");
  }
  const capabilitiesRaw = record["capabilities"];
  if (!Array.isArray(capabilitiesRaw) || capabilitiesRaw.some((c) => typeof c !== "string")) {
    throw new MethodError("MALFORMED_FRAME", "params.capabilities must be an array of strings");
  }
  for (const key of ["fuel", "memoryCapBytes"] as const) {
    const value = record[key];
    if (typeof value !== "number" || !Number.isInteger(value) || value < 1) {
      throw new MethodError("MALFORMED_FRAME", `params.${key} must be a positive integer`);
    }
  }
  if (binding.langPackLoad === undefined) {
    throw new MethodError(
      "SANDBOX_UNAVAILABLE",
      "this core binding lacks the lang-pack registry (wasm-fallback or stub lane, SDD §6.2)",
    );
  }
  let bytes: Uint8Array;
  try {
    bytes = await loadGuestWasm(wasmPath);
  } catch (error) {
    throw new MethodError(
      "SANDBOX_UNAVAILABLE",
      `plugin component unreadable: ${error instanceof Error ? error.message : String(error)}`,
    );
  }
  const outcome = await binding.langPackLoad(bytes, capabilitiesRaw as string[], blessed, {
    fuel: record["fuel"] as number,
    memoryCapBytes: record["memoryCapBytes"] as number,
  });
  const registered = outcome.registered;
  if (registered !== undefined && registered !== null) {
    registerPackLanguage(registered.id, registered.extensions);
  }
  return outcome;
}

/**
 * Batched classify-file over one carrier instantiation (chunk:m5-lang-pack).
 * Same degradation contract as the other sandbox lanes.
 */
export async function handleSandboxLangPackClassify(
  binding: CoreBinding,
  params: unknown,
): Promise<CoreLangPackClassifyOutcome> {
  const record = isRecord(params) ? params : {};
  const wasmPath = requireString(record, "wasmPath");
  const pathsRaw = record["paths"];
  if (!Array.isArray(pathsRaw) || pathsRaw.some((p) => typeof p !== "string")) {
    throw new MethodError("MALFORMED_FRAME", "params.paths must be an array of strings");
  }
  const capabilitiesRaw = record["capabilities"];
  if (!Array.isArray(capabilitiesRaw) || capabilitiesRaw.some((c) => typeof c !== "string")) {
    throw new MethodError("MALFORMED_FRAME", "params.capabilities must be an array of strings");
  }
  for (const key of ["fuel", "memoryCapBytes"] as const) {
    const value = record[key];
    if (typeof value !== "number" || !Number.isInteger(value) || value < 1) {
      throw new MethodError("MALFORMED_FRAME", `params.${key} must be a positive integer`);
    }
  }
  if (binding.langPackClassify === undefined) {
    throw new MethodError(
      "SANDBOX_UNAVAILABLE",
      "this core binding lacks the lang-pack registry (wasm-fallback or stub lane, SDD §6.2)",
    );
  }
  let bytes: Uint8Array;
  try {
    bytes = await loadGuestWasm(wasmPath);
  } catch (error) {
    throw new MethodError(
      "SANDBOX_UNAVAILABLE",
      `plugin component unreadable: ${error instanceof Error ? error.message : String(error)}`,
    );
  }
  return binding.langPackClassify(bytes, pathsRaw as string[], capabilitiesRaw as string[], {
    fuel: record["fuel"] as number,
    memoryCapBytes: record["memoryCapBytes"] as number,
  });
}

/** The trivial JS core used by IPC suites on machines without artifacts. */
export function stubCoreBinding(): CoreBinding {
  const openers = new Map([
    ["(", ")"],
    ["[", "]"],
    ["{", "}"],
  ]);
  const closers = new Set(openers.values());
  const parse = (language: string, source: string): CoreParseSummary => {
    const stack: string[] = [];
    let errorCount = 0;
    for (const ch of source) {
      const closer = openers.get(ch);
      if (closer !== undefined) stack.push(closer);
      else if (closers.has(ch) && stack.pop() !== ch) errorCount += 1;
    }
    errorCount += stack.length;
    return {
      language,
      rootKind: "stub",
      nodeCount: source.length,
      errorCount,
      hasError: errorCount > 0,
    };
  };
  const search = (pattern: string, haystack: string): CoreSearchHit[] => {
    const hits: CoreSearchHit[] = [];
    haystack.split("\n").forEach((lineText, i) => {
      const start = lineText.indexOf(pattern);
      if (start >= 0) {
        hits.push({ line: i + 1, start, end: start + pattern.length, lineText });
      }
    });
    return hits;
  };
  // Metavariable-aware emulation: `$A` matches one identifier, `$$$X` any
  // span. Enough for wire tests; fidelity lives in the Rust core.
  const structuralSearch = (
    language: string,
    source: string,
    pattern: string,
  ): CoreStructuralHit[] => {
    void language;
    const escaped = pattern.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
    const regex = new RegExp(
      escaped.replace(/\\\$\\\$\\\$[A-Z_]*/g, ".*?").replace(/\\\$[A-Z_]+/g, "[\\w$]+"),
    );
    const hits: CoreStructuralHit[] = [];
    source.split("\n").forEach((lineText, i) => {
      const match = regex.exec(lineText);
      if (match !== null) {
        hits.push({
          line: i + 1,
          column: match.index + 1,
          endLine: i + 1,
          kind: "stub_match",
          lineText,
        });
      }
    });
    return hits;
  };
  const rewrite = (language: string, source: string, pattern: string, replacement: string) => {
    void language;
    const output = source.replaceAll(pattern, replacement);
    return { output, matchCount: output === source ? 0 : 1 };
  };
  const STUB_KINDS: Readonly<Record<string, string>> = {
    function: "function",
    fn: "function",
    def: "function",
    class: "class",
    struct: "struct",
    interface: "interface",
    trait: "trait",
    enum: "enum",
  };
  const extract = (language: string, source: string): Promise<CoreExtraction> => {
    void language;
    const symbols: CoreSymbol[] = [];
    const imports: string[] = [];
    source.split("\n").forEach((lineText, i) => {
      const sym =
        /^\s*(?:export\s+|pub\s+|public\s+|async\s+)*(function|fn|def|class|struct|interface|trait|enum)\s+([A-Za-z_$][\w$]*)/.exec(
          lineText,
        );
      if (sym?.[1] !== undefined && sym[2] !== undefined) {
        symbols.push({
          name: sym[2],
          kind: STUB_KINDS[sym[1]] ?? sym[1],
          startLine: i + 1,
          startCol: 1,
          endLine: i + 1,
        });
      }
      const imp =
        /^\s*(?:import\b[^"'<]*|#include\s*|require\s*\(?\s*)["'<]([^"'>]+)["'>]/.exec(lineText) ??
        /^\s*use\s+([\w:]+)/.exec(lineText);
      if (imp?.[1] !== undefined) imports.push(imp[1]);
    });
    return Promise.resolve({ symbols, imports });
  };
  const selfTest = (): CoreSelfTestReport => {
    const parseOk = !parse("typescript", "const a = 1").hasError;
    const searchOk = search("a", "const a = 1").length === 1;
    const rewriteOk = rewrite("typescript", "a", "a", "b").output === "b";
    return {
      parseOk,
      searchOk,
      rewriteOk,
      allOk: parseOk && searchOk && rewriteOk,
      details: [],
    };
  };
  return {
    version: () => "stub",
    languages: () => ["typescript"],
    parse,
    search,
    structuralSearch,
    rewrite,
    extract,
    scanWorkspace: (root: string) => Promise.resolve(fallbackScanWorkspace(root)),
    selfTest,
  };
}

interface KillSwitch {
  readonly on?: string;
  readonly match?: string;
}

function killSwitchFromEnv(env: Readonly<Record<string, string | undefined>>): KillSwitch {
  const on = env["WORKBENCH_ENGINE_KILL_ON"];
  const match = env["WORKBENCH_ENGINE_KILL_MATCH"];
  return {
    ...(on === undefined ? {} : { on }),
    ...(match === undefined ? {} : { match }),
  };
}

function shouldKill(kill: KillSwitch, point: string, payloadText: string): boolean {
  return kill.on === point && (kill.match === undefined || payloadText.includes(kill.match));
}

function writeFrame(frame: Frame): void {
  process.stdout.write(encodeFrame(frame));
}

/**
 * The child's main loop: load and self-test the core, announce ready, then
 * serve frames from stdin until EOF. Never returns except on stream end.
 */
export async function runEngineChild(): Promise<void> {
  const kill = killSwitchFromEnv(process.env);
  if (shouldKill(kill, "selftest", "")) {
    // Planted mid-startup kill: die during the load/verify phase.
    process.exit(CHAOS_EXIT_CODE);
  }

  let binding: CoreBinding;
  let flavor: string;
  if (process.env["WORKBENCH_ENGINE_STUB_CORE"] === "1") {
    binding = stubCoreBinding();
    flavor = "stub";
  } else {
    const loaded: LoadedCore = loadVerifiedCore();
    binding = loaded.binding;
    flavor = loaded.flavor;
  }

  writeFrame({
    v: IPC_VERSION,
    kind: "ready",
    core: { flavor, version: binding.version() },
  });

  const indexers = new IndexerHost(binding);
  const cancelled = new Set<number>();
  const decoder = new FrameDecoder();
  // §7.7: request handling must not block the frame loop — a cancel frame
  // has to be decodable WHILE its request runs, or cancellation could never
  // stop in-flight work. Handlers start synchronously (the chaos kill switch
  // still fires at frame receipt) and park here until they settle.
  const inFlight = new Set<Promise<void>>();
  for await (const chunk of process.stdin) {
    const bytes: Uint8Array = typeof chunk === "string" ? new TextEncoder().encode(chunk) : chunk;
    for (const decoded of decoder.push(bytes)) {
      if (!decoded.ok) {
        if (decoded.id !== null) {
          writeFrame({ v: IPC_VERSION, kind: "err", id: decoded.id, error: decoded.error });
        }
        continue;
      }
      const frame = decoded.frame;
      if (frame.kind === "cancel") {
        cancelled.add(frame.id);
        continue;
      }
      if (frame.kind !== "req") continue;
      const task: Promise<void> = handleRequestFrame(binding, indexers, kill, cancelled, frame);
      inFlight.add(task);
      void task.finally(() => inFlight.delete(task));
    }
  }
  await Promise.all([...inFlight]);
  // specs/007-resolver-server-pool: every in-flight invoke has settled, so
  // kill the warm language-server pool explicitly — Rust statics never drop
  // at process exit, and the supervisor's own stop is a hard kill (the OS
  // kill group covers that path on Windows and Linux).
  binding.sandboxServerPoolShutdown?.();
}

/** Errors thrown by method handlers carry an answerable frame-error code. */
function frameErrorOf(error: unknown): FrameError {
  if (error instanceof Error && "code" in error && typeof error.code === "string") {
    return { code: error.code, message: error.message };
  }
  return { code: "ENGINE_UNAVAILABLE", message: String(error) };
}

async function handleRequestFrame(
  binding: CoreBinding,
  indexers: IndexerHost,
  kill: KillSwitch,
  cancelled: Set<number>,
  frame: RequestFrame,
): Promise<void> {
  if (cancelled.has(frame.id)) {
    cancelled.delete(frame.id);
    return;
  }
  if (shouldKill(kill, frame.method, JSON.stringify(frame))) {
    // Planted mid-operation kill: die with the request in flight, response unsent.
    process.exit(CHAOS_EXIT_CODE);
  }
  let outcome: { ok: true; result: unknown } | { ok: false; error: FrameError };
  try {
    // §7.7: the token lets long index queries stop between parse units.
    const isCancelled = (): boolean => cancelled.has(frame.id);
    outcome = {
      ok: true,
      result: isIndexMethod(frame.method)
        ? await indexers.handle(frame.method, frame.params, isCancelled)
        : frame.method === SANDBOX_WASI_METHOD
          ? await handleSandboxWasi(binding, frame.params)
          : frame.method === SANDBOX_COMPONENT_METHOD
            ? await handleSandboxComponent(binding, frame.params)
            : frame.method === SANDBOX_RESOLVER_METHOD
              ? await handleSandboxResolver(binding, frame.params)
              : frame.method === SANDBOX_RUNNER_METHOD
                ? await handleSandboxRunner(binding, frame.params)
                : frame.method === SANDBOX_RANKING_METHOD
                  ? await handleSandboxRanking(binding, frame.params)
                  : frame.method === SANDBOX_LANG_PACK_METHOD
                    ? await handleSandboxLangPack(binding, frame.params)
                    : frame.method === SANDBOX_LANG_PACK_CLASSIFY_METHOD
                      ? await handleSandboxLangPackClassify(binding, frame.params)
                      : handleRequest(binding, frame.method, frame.params),
    };
  } catch (error) {
    outcome = { ok: false, error: frameErrorOf(error) };
  }
  if (cancelled.delete(frame.id)) return;
  writeFrame(
    outcome.ok
      ? { v: IPC_VERSION, kind: "res", id: frame.id, result: outcome.result }
      : { v: IPC_VERSION, kind: "err", id: frame.id, error: outcome.error },
  );
}
