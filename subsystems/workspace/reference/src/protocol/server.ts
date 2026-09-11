/**
 * The MCP server role (SDD §5.1–5.2), on the official SDK.
 *
 * Statelessness is structural: a server instance is a pure function of the
 * workspace runtime plus the injected port composition, every request's
 * envelope version is negotiated from that request's own `_meta`, and nothing
 * here holds per-session state. The Streamable HTTP path constructs a fresh
 * server + transport pair per request, so there is no session to leak even by
 * accident.
 *
 * Tools are served through the SDK's low-level Server so each `inputSchema`
 * ships **verbatim** from `src/generated/schemas.ts` — generated from
 * `schemas/tools/` (Constitution V: nothing hand-writes a tool schema in
 * TypeScript, and the high-level McpServer API only accepts zod shapes).
 *
 * Surface: `workbench_status` (M0), `search` in lexical, structural,
 * references and files modes (chunks m1-search-lexical and
 * m1-search-structural), `map` (chunk m1-map), `read` (chunk m1-read),
 * `edit` (chunk m2-edit-verbs), `files` (chunk m3-files-tool) and `undo`
 * (chunk m2-journal-undo) when the write path is composed, and the Tasks
 * extension — `tasks/get` + `tasks/list` over the derived `index/build` task
 * (chunk m1-tasks-integration). The remaining tools land with their own chunks.
 */

import { createHash } from "node:crypto";
import { readFile, stat, writeFile } from "node:fs/promises";
import { join } from "node:path";
import { Server } from "@modelcontextprotocol/sdk/server/index.js";
import {
  CallToolRequestSchema,
  type ElicitResult,
  ErrorCode,
  GetTaskRequestSchema,
  ListTasksRequestSchema,
  ListToolsRequestSchema,
  McpError,
  type Tool,
} from "@modelcontextprotocol/sdk/types.js";
import {
  ENVELOPE_VERSION_LATEST,
  ENVELOPE_VERSION_MIN,
  isErrorCode,
  negotiateEnvelopeVersion,
  okEnvelope,
  type ResponseEnvelope,
  sealEnvelope,
  taxonomyError,
} from "../envelope";
import { envelopeSchema, toolSchemas } from "../generated/schemas";
import {
  canonicalizeWithin,
  checkConfirmGrant,
  ensureWorkspaceKey,
  mintConfirmGrant,
} from "../jail";
import { noopTracer, type RootEndAttributes, type SpanStatus, type Tracer } from "../ports/tracing";
import {
  type ConfirmPort,
  EDIT_OPS,
  type EditDeps,
  type EditInput,
  FILES_OPS,
  type FilesDeps,
  type FilesInput,
  type InstalledPluginRecord,
  isPluginToolName,
  listPluginTools,
  type MapDeps,
  type MapInput,
  type PluginGuestPolicy,
  type PluginHealthRecord,
  type PluginHostDeps,
  type PluginInvokeOutcome,
  type ReadDeps,
  type ReadInput,
  type RunExecutor,
  type RunInput,
  type RunnerAdapter,
  runEdit,
  runFiles,
  runMap,
  runPluginTool,
  runRead,
  runRun,
  runScript,
  runSearch,
  runUndo,
  type ScriptDeps,
  type ScriptGuestOutcome,
  type ScriptGuestPolicy,
  type ScriptInput,
  type SearchDeps,
  type SearchInput,
  UNDO_LIST_LIMIT_MAX,
  UNDO_STEPS_MAX,
  type UndoInput,
  type WriteEngineDeps,
} from "../tools";
import { indexBuildTask } from "./tasks";
import type { WorkspaceRuntime } from "./workspace";

/** One binary, one version: keep in lockstep with package.json. */
export const SERVER_NAME = "workbench-mcp";
export const SERVER_VERSION = "0.0.0";

export interface StatusResult {
  readonly root: string;
  readonly journalMode: string;
  readonly appliedJournalMode: string;
  readonly networkFs: boolean;
  readonly detectionMethod: string;
  readonly envelope: { readonly min: number; readonly latest: number };
}

/** The status envelope for one request — deterministic for a fixed workspace. */
export function statusEnvelope(
  workspace: WorkspaceRuntime,
  meta?: Readonly<Record<string, unknown>>,
  extraNotes: readonly string[] = [],
): ResponseEnvelope<StatusResult> {
  const negotiation = negotiateEnvelopeVersion(meta);
  if (!negotiation.ok) {
    return withWorkspaceNotes(workspace, negotiation.envelope);
  }
  const notes = [...workspace.notes, ...extraNotes];
  return okEnvelope<StatusResult>({
    v: negotiation.v,
    freshness: "warm",
    budget: { requested: 1, used: 1 },
    results: [
      {
        root: workspace.root,
        journalMode: workspace.journalMode,
        appliedJournalMode: workspace.appliedJournalMode,
        networkFs: workspace.detection.networkFs,
        detectionMethod: workspace.detection.method,
        envelope: { min: ENVELOPE_VERSION_MIN, latest: ENVELOPE_VERSION_LATEST },
      },
    ],
    ...(notes.length === 0 ? {} : { notes }),
  });
}

/** Degradations ride every envelope's notes (SDD §5.2), errors included. */
function withWorkspaceNotes<R>(
  workspace: WorkspaceRuntime,
  envelope: ResponseEnvelope<R>,
): ResponseEnvelope<R> {
  if (workspace.notes.length === 0) return envelope;
  return { ...envelope, notes: [...(envelope.notes ?? []), ...workspace.notes] };
}

/**
 * The caller's session identity from `_meta` (SDD §7.6, §5.1): identity
 * travels on the request; "" is the anonymous identity. Bounded so a hostile
 * client cannot bloat the journal's identity column.
 */
export function sessionIdFrom(meta?: Readonly<Record<string, unknown>>): string {
  const raw = meta?.["workbench/sessionId"];
  return typeof raw === "string" && raw.length > 0 && raw.length <= 256 ? raw : "";
}

/**
 * The composed guest-execution seam for `script` (SDD §9.7): the engine
 * child's sandbox lane plus the vendored interpreter path, provided by the
 * composing layer. Absent (tests, degraded compositions) means the script
 * tool is not offered at all — honest absence over emulation (§6.2).
 */
export interface ScriptRuntime {
  runGuest(argv: readonly string[], policy: ScriptGuestPolicy): Promise<ScriptGuestOutcome>;
}

/**
 * The composed execution seam for `run` (SDD §9.6): one argv, no shell, hard
 * wall-clock cap — the real spawn lives in the composing layer (main.ts).
 * Absent (tests, degraded compositions) means the run tool is not offered at
 * all — honest absence over emulation (§6.2). The allowlist itself is NOT
 * part of this seam: it always comes from the merged config on the workspace
 * runtime, so no composition can hand `run` a wider table than the human
 * declared.
 */
export interface RunRuntime {
  readonly execute: RunExecutor;
  /**
   * Optional adapter trust order override (chunk:m5-runner-adapter): the
   * composition injects component-lane adapters here; absent, runRun uses
   * the bundled six unchanged.
   */
  readonly adapters?: readonly RunnerAdapter[];
}

/**
 * The composed guest-execution seam for plugin tools (SDD §8.1/§8.3,
 * chunk:m4-capability-manifests): the engine child's component lane. Absent
 * (tests, degraded compositions) means plugin tools are not offered at all —
 * honest absence over emulation (§6.2). Core tools never touch this seam,
 * which is what "core tools continue at full fidelity" is made of.
 */
export interface PluginsRuntime {
  invokeGuest(
    absPath: string,
    exportName: string,
    payload: string,
    capabilities: readonly string[],
    policy: PluginGuestPolicy,
  ): Promise<PluginInvokeOutcome>;
}

/** The declared MCP tool surface: schemas verbatim from the generated module. */
function toolList(write?: WriteEngineDeps, script?: ScriptRuntime, run?: RunRuntime): Tool[] {
  const runTools: Tool[] =
    run === undefined
      ? []
      : [
          {
            name: "run",
            title: "Run a declared project command",
            description:
              "The declared-command escape hatch (SDD §9.6). Executes one command from the " +
              "merged config's [commands] table as a single argv — no shell, ever. The " +
              "allowlist is seeded at init from lockfile/manifest detection, amendable only " +
              "by a human (workbench.toml is a protected path), and governable by org policy " +
              "(§7.2). Known runners (bun/jest/vitest/pytest/cargo test/cargo nextest/go) " +
              "answer structured failures — test name, file, assertion, trimmed trace — via " +
              "bundled runner adapters; cargo build/check/clippy (and a test run that failed " +
              "to compile) answer a diagnostics row — level, code, message, file:line:column, " +
              "help — errors first, trimmed to the budget with a manifest; unknown commands " +
              "answer head+tail with a truncation manifest. An " +
              "undeclared command answers COMMAND_NOT_DECLARED (declare it in your config " +
              "layer); text/file munging or shell attempts answer RUN_REDIRECTED — use " +
              "search/read to inspect and edit to modify.",
            inputSchema: toolSchemas.run.inputSchema as unknown as Tool["inputSchema"],
            outputSchema: envelopeSchema as unknown as Tool["outputSchema"],
          },
        ];
  const scriptTools: Tool[] =
    write === undefined || script === undefined
      ? []
      : [
          {
            name: "script",
            title: "Run a sandboxed script",
            description:
              "Typed Code Mode for the long tail — replaces ad-hoc bash pipelines (SDD §9.7). " +
              "The source runs as a JavaScript function body in a QuickJS-ng-on-WASM guest " +
              "under the Wasmtime host: fuel-metered, memory-capped, no ambient filesystem, " +
              "network, or shell. The one global is `workspace`: search(args) and read(args) " +
              "answer the same envelopes as the tools, glob(pattern) lists matching workspace " +
              "paths — all live and jail-checked — while workspace.edit.queue({file, old, " +
              "new, occurrence?}) and workspace.files.queue({op: create|mkdir|delete, ...}) " +
              "only QUEUE writes. Use `return` for the JSON-serializable result. Queued " +
              "writes resolve through the §7.5 write engine and are always confirm-gated: " +
              "dryRun:true (default) returns the value plus every dry-run diff and never " +
              "applies; dryRun:false answers CONFIRM_REQUIRED with a confirmToken and " +
              "re-invoking the identical call with confirm applies. Abuse dies typed: " +
              "SANDBOX_FUEL_EXCEEDED, SANDBOX_MEMORY_EXCEEDED, SANDBOX_CAP_DENIED.",
            inputSchema: toolSchemas.script.inputSchema as unknown as Tool["inputSchema"],
            outputSchema: envelopeSchema as unknown as Tool["outputSchema"],
          },
        ];
  const writeTools: Tool[] =
    write === undefined
      ? []
      : [
          {
            name: "edit",
            title: "Edit the workspace",
            description:
              "Verified structural editing — replaces sed, awk and patch (SDD §9.4). Symbol " +
              "verbs (replace_body | insert_after | insert_before | rename) address code via " +
              "the Tier-1 index; rewrite applies an ast-grep pattern across a glob scope; " +
              "replace_text is the anchored literal fallback. Every verb runs the §7.5 write " +
              "engine: a new syntax error rejects the whole call (PARSE_REJECTED with the " +
              "parser message and rejected diff), applied diffs are the receipt, and read's " +
              "contentHash passed as expectedHash turns a changed file into STALE_ANCHOR with " +
              "the fresh hash. Single-file symbol/text edits auto-apply; rewrite, rename and " +
              "anything touching >1 file or >200 changed lines is confirm-gated: MRTR clients " +
              "approve the aggregated diff in-call via elicitation, everyone else gets " +
              "CONFIRM_REQUIRED carrying a confirmToken and re-invokes the identical call " +
              "with confirm to apply. A stale confirm answers STALE_ANCHOR, a forged or " +
              "mismatched one CONFIRM_MISMATCH.",
            inputSchema: toolSchemas.edit.inputSchema as unknown as Tool["inputSchema"],
            outputSchema: envelopeSchema as unknown as Tool["outputSchema"],
          },
          {
            name: "files",
            title: "File operations",
            description:
              "Typed file operations — replaces mv, cp, mkdir, touch and rm (SDD §9.5). move " +
              "is import-aware: import specifiers that resolve to the moved file are " +
              "rewritten across the workspace (typescript, javascript, python, java, c, c++ " +
              "— specifier-based, name-resolution-free, the notes say exactly what was " +
              "rewritten) and the moved file's own relative imports are recomputed; " +
              "inherently multi-file, so always confirm-gated (MRTR elicitation in-call, or " +
              "CONFIRM_REQUIRED + confirmToken as two calls). delete is a journal-backed " +
              "move to .workbench/trash/ — recoverable with undo, nothing is unlinked " +
              "destructively. create is touch (never overwrites); mkdir materializes the " +
              "directory as a journaled .gitkeep. Every verb runs the §7.5 write engine: " +
              "dry-run diff, parse-verify, format, journal, atomic swap; read's contentHash " +
              "passed as expectedHash turns a changed source into STALE_ANCHOR.",
            inputSchema: toolSchemas.files.inputSchema as unknown as Tool["inputSchema"],
            outputSchema: envelopeSchema as unknown as Tool["outputSchema"],
          },
          {
            name: "undo",
            title: "Read and rewind journaled ops",
            description:
              'The journal, read and rewound (SDD §9.8, §7.6). action "list" reads the ' +
              "durable per-workspace op log — newest op first, one row per file an op " +
              "touched, each carrying the op seq, its added/removed counts and its unified " +
              'diff — and writes nothing. action "rewind" (the default) rewinds whole ops ' +
              "(write batches): the newest `steps`, or exactly the ops named by `ops` (the " +
              "seqs list reports). Rewind is session-scoped by default — only the calling " +
              "session's ops (identity via _meta workbench/sessionId) unless scope is " +
              '"workspace". Conflicts are detected by content hash and surfaced in the ' +
              "envelope as a three-way choice (keepCurrent | force | merge by hand), never " +
              "silently clobbered. Restorations run through the verified write pipeline " +
              "with staleness anchors.",
            inputSchema: toolSchemas.undo.inputSchema as unknown as Tool["inputSchema"],
            outputSchema: envelopeSchema as unknown as Tool["outputSchema"],
          },
        ];
  return [
    {
      name: "workbench_status",
      title: "Workbench status",
      description:
        "Workspace health: root, SQLite journal mode (WAL, or TRUNCATE when the workspace " +
        "sits on a network filesystem), and the supported envelope versions. Every result " +
        "is a versioned ResponseEnvelope; degradations ride its `notes`.",
      inputSchema: { type: "object", properties: {} },
      outputSchema: envelopeSchema as unknown as Tool["outputSchema"],
    },
    {
      name: "map",
      title: "Map the workspace",
      description:
        "Workspace orientation — replaces ls -R and orientation greps (SDD §9.2). A pruned " +
        "directory tree (Tier-0 capable, no index required), per-file symbol outlines when " +
        "the Tier-1 index is up (detail: skeleton | outline | full), index health, a " +
        "language census, and the installed-plugin inventory with trust labels (sideloads " +
        "are always untrusted). The overview row carries a short ttlMs keyed to the index " +
        "generation for client caching.",
      inputSchema: toolSchemas.map.inputSchema as unknown as Tool["inputSchema"],
      outputSchema: envelopeSchema as unknown as Tool["outputSchema"],
    },
    {
      name: "read",
      title: "Read a file",
      description:
        "Budgeted, enveloped file reading — replaces cat, head, tail and sed -n (SDD §9.3). " +
        "Reads the whole file, an explicit lines: {start, end} range, or a symbol resolved " +
        "via the Tier-1 index (surround: none | enclosing | imports). The unit header " +
        "carries precise line anchors and a sha256 content hash for edit's staleness " +
        "anchor; content is mtime+size-verified before return and truncation always " +
        "carries a manifest with a line cursor.",
      inputSchema: toolSchemas.read.inputSchema as unknown as Tool["inputSchema"],
      outputSchema: envelopeSchema as unknown as Tool["outputSchema"],
    },
    {
      name: "search",
      title: "Search the workspace",
      description:
        "Ranked, grouped search — replaces grep, glob and find (SDD §9.1). Modes: auto " +
        "(runs lexical), lexical (regex over live file content), structural (ast-grep " +
        "pattern with $VAR metavariables, hits carry node kinds), references (symbol " +
        "edges from the warm index, each with confidence provenance), files " +
        "(glob/filename). semantic is reserved and rejected with a typed error. Results " +
        "are grouped by file, definitions first, junk and tests demoted; truncation " +
        "always carries a manifest.",
      inputSchema: toolSchemas.search.inputSchema as unknown as Tool["inputSchema"],
      outputSchema: envelopeSchema as unknown as Tool["outputSchema"],
    },
    ...writeTools,
    ...scriptTools,
    ...runTools,
  ];
}

/** The confirmToken of a CONFIRM_REQUIRED failure envelope, else null. */
function pendingConfirmToken(envelope: ResponseEnvelope<unknown>): string | null {
  if (envelope.ok) return null;
  const error = envelope.error as { code?: unknown; confirmToken?: unknown };
  return error.code === "CONFIRM_REQUIRED" && typeof error.confirmToken === "string"
    ? error.confirmToken
    : null;
}

/** Ceiling on the diff text an MRTR elicitation message carries. */
export const ELICIT_DIFF_CHARS = 4000;

/**
 * MRTR (SDD §5.1, §9.4): present the aggregated dry-run diff to the client
 * mid-call and ask for the confirmation. True only on an explicit accept.
 * The message is built from the SEALED envelope — elicitation is a response
 * surface, and redaction has exactly one choke point (§7.1).
 */
async function elicitConfirm(
  server: Server,
  envelope: ResponseEnvelope<unknown>,
  signal?: AbortSignal,
): Promise<boolean> {
  const sealed = sealEnvelope(envelope).envelope;
  const message = sealed.ok
    ? ""
    : `${sealed.error.message}\n\n${(sealed.notes ?? []).join("\n").slice(0, ELICIT_DIFF_CHARS)}`;
  let result: ElicitResult;
  try {
    result = await server.elicitInput(
      {
        mode: "form",
        message: `${message}\n\nApply these changes?`,
        requestedSchema: {
          type: "object",
          properties: {
            apply: {
              type: "boolean",
              title: "Apply",
              description: "Apply the diff above to the workspace",
            },
          },
          required: ["apply"],
        },
      },
      signal === undefined ? undefined : { signal },
    );
  } catch {
    // An MRTR-capable client that still fails the elicitation gets the token
    // fallback: the CONFIRM_REQUIRED envelope it receives carries the token.
    return false;
  }
  return result.action === "accept" && result.content?.["apply"] === true;
}

/** Seal (redact) and wrap an envelope as an MCP tool result. */
function toolResult(envelope: ResponseEnvelope<unknown>): {
  content: Array<{ type: "text"; text: string }>;
  structuredContent: Record<string, unknown>;
} {
  // The one door to the wire (SDD §7.1): every envelope is sealed —
  // redacted at the envelope layer — before either representation ships.
  const sealed = sealEnvelope(envelope);
  return {
    content: [{ type: "text" as const, text: sealed.json }],
    structuredContent: sealed.envelope as unknown as Record<string, unknown>,
  };
}

/**
 * The installed-plugin inventory (SDD §8.4), read from
 * `.workbench/plugins.json` on every call — stateless by construction, and
 * fail-closed: an entry whose source is not exactly "registry" is a sideload,
 * and a trust tier is honored only on registry entries, so a sideloaded
 * manifest cannot label itself trusted.
 */
export async function loadPluginInventory(root: string): Promise<readonly InstalledPluginRecord[]> {
  let raw: string;
  try {
    raw = await readFile(join(root, ".workbench", "plugins.json"), "utf8");
  } catch {
    // No inventory file: nothing installed.
    return [];
  }
  const parsed: unknown = JSON.parse(raw);
  const list = (parsed as { plugins?: unknown }).plugins;
  if (!Array.isArray(list)) {
    throw new Error(".workbench/plugins.json: `plugins` must be an array");
  }
  return list.map((entry): InstalledPluginRecord => {
    const record = entry as Record<string, unknown>;
    if (typeof record["name"] !== "string" || record["name"].length === 0) {
      throw new Error(".workbench/plugins.json: every plugin needs a non-empty name");
    }
    const source = record["source"] === "registry" ? "registry" : "sideload";
    const tier =
      source === "registry" && (record["tier"] === "blessed" || record["tier"] === "verified")
        ? record["tier"]
        : undefined;
    // The invocable extension (chunk:m4-capability-manifests), additive to
    // the map inventory shape: `dir` points at the plugin's workspace-relative
    // directory, `grants` is the install-time grant record. Both fail-closed:
    // a malformed value means "inventory-only" / "nothing granted".
    const dir =
      typeof record["dir"] === "string" && record["dir"].length > 0 && !record["dir"].includes("..")
        ? record["dir"]
        : undefined;
    const grantsRaw = record["grants"];
    const grants =
      Array.isArray(grantsRaw) && grantsRaw.every((g) => typeof g === "string")
        ? (grantsRaw as string[])
        : undefined;
    return {
      name: record["name"],
      ...(typeof record["version"] === "string" ? { version: record["version"] } : {}),
      source,
      ...(tier === undefined ? {} : { tier }),
      ...(dir === undefined ? {} : { dir }),
      ...(grants === undefined ? {} : { grants }),
    };
  });
}

/**
 * Compose the plugin host's dependencies over the workspace (SDD §8.1/§8.3).
 * Every path is jail-checked before any filesystem access exists (§7.1), the
 * inventory and manifests are read per call (stateless), and the circuit
 * breaker persists under `.workbench/` beside the quarantine record.
 * Exported for the composition root, which reuses the same inventory,
 * manifest, component and health seams for the resolver-plugin adapter
 * (chunk:m5-lsp-resolver-plugin) — one reading of the grant records, not two.
 */
export function pluginHostDeps(
  workspace: WorkspaceRuntime,
  plugins: PluginsRuntime,
): PluginHostDeps {
  const healthPath = join(workspace.stateDir, "plugin-health.json");
  const readHealth = async (): Promise<readonly PluginHealthRecord[]> => {
    try {
      const parsed: unknown = JSON.parse(await readFile(healthPath, "utf8"));
      const list = (parsed as { breaches?: unknown }).breaches;
      if (!Array.isArray(list)) return [];
      return list.filter(
        (b): b is PluginHealthRecord =>
          typeof b === "object" &&
          b !== null &&
          typeof (b as PluginHealthRecord).plugin === "string" &&
          typeof (b as PluginHealthRecord).componentHash === "string",
      );
    } catch {
      return [];
    }
  };
  return {
    inventory: () => loadPluginInventory(workspace.root),
    readManifest: (dir) => {
      const decision = canonicalizeWithin(workspace.root, join(dir, "plugin.toml"));
      if (!decision.ok) return Promise.reject(new Error(decision.reason));
      return readFile(decision.path, "utf8");
    },
    componentInfo: async (dir, componentPath) => {
      const decision = canonicalizeWithin(workspace.root, join(dir, componentPath));
      if (!decision.ok) return null;
      try {
        const bytes = await readFile(decision.path);
        return {
          absPath: decision.path,
          sha256: createHash("sha256").update(bytes).digest("hex"),
        };
      } catch {
        return null;
      }
    },
    invokeGuest: (absPath, exportName, payload, capabilities, policy) =>
      plugins.invokeGuest(absPath, exportName, payload, capabilities, policy),
    health: {
      read: readHealth,
      append: async (record) => {
        const breaches = [...(await readHealth()), record];
        await writeFile(healthPath, `${JSON.stringify({ breaches }, null, 2)}\n`, "utf8");
      },
    },
    now: () => new Date().toISOString(),
  };
}

async function mapEnvelope(
  workspace: WorkspaceRuntime,
  search: SearchDeps,
  args: Record<string, unknown>,
  meta?: Readonly<Record<string, unknown>>,
): Promise<ResponseEnvelope<unknown>> {
  const negotiation = negotiateEnvelopeVersion(meta);
  if (!negotiation.ok) {
    return withWorkspaceNotes(workspace, negotiation.envelope);
  }
  const deps: MapDeps = {
    index: search.index,
    workspace: search.workspace,
    plugins: () => loadPluginInventory(workspace.root),
  };
  const envelope = await runMap(deps, args as unknown as MapInput, negotiation.v);
  return withWorkspaceNotes(workspace, envelope);
}

async function readEnvelope(
  workspace: WorkspaceRuntime,
  search: SearchDeps,
  args: Record<string, unknown>,
  meta?: Readonly<Record<string, unknown>>,
): Promise<ResponseEnvelope<unknown>> {
  const negotiation = negotiateEnvelopeVersion(meta);
  if (!negotiation.ok) {
    return withWorkspaceNotes(workspace, negotiation.envelope);
  }
  if (typeof args["file"] !== "string" || args["file"].length === 0) {
    throw new McpError(ErrorCode.InvalidParams, "read: file must be a non-empty string");
  }
  if (args["symbol"] !== undefined && args["lines"] !== undefined) {
    throw new McpError(
      ErrorCode.InvalidParams,
      "read: symbol and lines are mutually exclusive (SDD §9.3)",
    );
  }
  // The jail decides before any filesystem access exists (SDD §7.1); the
  // injected deps operate only on the admitted canonical path.
  const decision = canonicalizeWithin(workspace.root, args["file"]);
  if (!decision.ok) {
    return withWorkspaceNotes(
      workspace,
      taxonomyError({ v: negotiation.v, code: decision.code, message: decision.reason }),
    );
  }
  const deps: ReadDeps = {
    index: search.index,
    workspace: {
      read: () => readFile(decision.path, "utf8"),
      stat: async () => {
        const s = await stat(decision.path);
        return { mtimeMs: s.mtimeMs, size: s.size };
      },
    },
  };
  const envelope = await runRead(deps, args as unknown as ReadInput, negotiation.v);
  return withWorkspaceNotes(workspace, envelope);
}

async function searchEnvelope(
  workspace: WorkspaceRuntime,
  search: SearchDeps,
  args: Record<string, unknown>,
  meta?: Readonly<Record<string, unknown>>,
  signal?: AbortSignal,
): Promise<ResponseEnvelope<unknown>> {
  const negotiation = negotiateEnvelopeVersion(meta);
  if (!negotiation.ok) {
    return withWorkspaceNotes(workspace, negotiation.envelope);
  }
  if (typeof args["query"] !== "string" || args["query"].length === 0) {
    throw new McpError(ErrorCode.InvalidParams, "search: query must be a non-empty string");
  }
  const envelope = await runSearch(
    search,
    args as unknown as SearchInput,
    negotiation.v,
    signal === undefined ? {} : { signal },
  );
  return withWorkspaceNotes(workspace, envelope);
}

async function editEnvelope(
  workspace: WorkspaceRuntime,
  search: SearchDeps,
  write: WriteEngineDeps,
  args: Record<string, unknown>,
  meta?: Readonly<Record<string, unknown>>,
): Promise<ResponseEnvelope<unknown>> {
  const negotiation = negotiateEnvelopeVersion(meta);
  if (!negotiation.ok) {
    return withWorkspaceNotes(workspace, negotiation.envelope);
  }
  const op = args["op"];
  if (typeof op !== "string" || !(EDIT_OPS as readonly string[]).includes(op)) {
    throw new McpError(ErrorCode.InvalidParams, `edit: op must be one of ${EDIT_OPS.join(", ")}`);
  }
  const requireString = (key: string): void => {
    if (typeof args[key] !== "string" || args[key] === "") {
      throw new McpError(ErrorCode.InvalidParams, `edit: ${op} requires a non-empty ${key}`);
    }
  };
  const symbolVerb =
    op === "replace_body" || op === "insert_after" || op === "insert_before" || op === "rename";
  if (symbolVerb || op === "replace_text") requireString("file");
  if (symbolVerb) requireString("symbol");
  if (op === "rename") requireString("newName");
  if (op === "replace_body" || op === "insert_after" || op === "insert_before") {
    if (typeof args["content"] !== "string") {
      throw new McpError(ErrorCode.InvalidParams, `edit: ${op} requires content`);
    }
  }
  if (op === "rewrite") {
    requireString("pattern");
    if (typeof args["rewritePattern"] !== "string") {
      throw new McpError(ErrorCode.InvalidParams, "edit: rewrite requires rewritePattern");
    }
  }
  if (op === "replace_text") {
    requireString("old");
    if (typeof args["new"] !== "string") {
      throw new McpError(ErrorCode.InvalidParams, "edit: replace_text requires new");
    }
  }
  if (args["confirm"] !== undefined && typeof args["confirm"] !== "string") {
    throw new McpError(ErrorCode.InvalidParams, "edit: confirm must be a string token");
  }
  // The jail decides on the named file before the tool runs (SDD §7.1); the
  // write path's composed access re-checks every path it actually touches.
  if (typeof args["file"] === "string") {
    const decision = canonicalizeWithin(workspace.root, args["file"]);
    if (!decision.ok) {
      return withWorkspaceNotes(
        workspace,
        taxonomyError({ v: negotiation.v, code: decision.code, message: decision.reason }),
      );
    }
  }
  // The key is (re)read from disk per call (§7.1): verification is stateless
  // by construction, tokens survive a server restart, and a rotation voids
  // every outstanding token immediately.
  const confirm: ConfirmPort = {
    mint: (diffHash, contentHash) =>
      mintConfirmGrant(ensureWorkspaceKey(workspace.stateDir), diffHash, contentHash),
    check: (token) => checkConfirmGrant(ensureWorkspaceKey(workspace.stateDir), token),
  };
  const deps: EditDeps = {
    write,
    index: search.index,
    confirm,
    list: () => search.workspace.list(),
  };
  const envelope = await runEdit(
    deps,
    args as unknown as EditInput,
    negotiation.v,
    sessionIdFrom(meta),
  );
  return withWorkspaceNotes(workspace, envelope);
}

async function filesEnvelope(
  workspace: WorkspaceRuntime,
  search: SearchDeps,
  write: WriteEngineDeps,
  args: Record<string, unknown>,
  meta?: Readonly<Record<string, unknown>>,
): Promise<ResponseEnvelope<unknown>> {
  const negotiation = negotiateEnvelopeVersion(meta);
  if (!negotiation.ok) {
    return withWorkspaceNotes(workspace, negotiation.envelope);
  }
  const op = args["op"];
  if (typeof op !== "string" || !(FILES_OPS as readonly string[]).includes(op)) {
    throw new McpError(ErrorCode.InvalidParams, `files: op must be one of ${FILES_OPS.join(", ")}`);
  }
  const requireString = (key: string): void => {
    if (typeof args[key] !== "string" || args[key] === "") {
      throw new McpError(ErrorCode.InvalidParams, `files: ${op} requires a non-empty ${key}`);
    }
  };
  if (op === "move" || op === "copy" || op === "delete") requireString("from");
  if (op !== "delete") requireString("to");
  if (args["confirm"] !== undefined && typeof args["confirm"] !== "string") {
    throw new McpError(ErrorCode.InvalidParams, "files: confirm must be a string token");
  }
  // The jail decides on both named paths before the tool runs (SDD §7.1); the
  // write path's composed access re-checks every path it actually touches.
  for (const key of ["from", "to"] as const) {
    if (typeof args[key] !== "string") continue;
    const decision = canonicalizeWithin(workspace.root, args[key]);
    if (!decision.ok) {
      return withWorkspaceNotes(
        workspace,
        taxonomyError({ v: negotiation.v, code: decision.code, message: decision.reason }),
      );
    }
  }
  const confirm: ConfirmPort = {
    mint: (diffHash, contentHash) =>
      mintConfirmGrant(ensureWorkspaceKey(workspace.stateDir), diffHash, contentHash),
    check: (token) => checkConfirmGrant(ensureWorkspaceKey(workspace.stateDir), token),
  };
  const deps: FilesDeps = {
    write,
    confirm,
    list: () => search.workspace.list(),
  };
  const envelope = await runFiles(
    deps,
    args as unknown as FilesInput,
    negotiation.v,
    sessionIdFrom(meta),
  );
  return withWorkspaceNotes(workspace, envelope);
}

async function scriptEnvelope(
  workspace: WorkspaceRuntime,
  search: SearchDeps,
  write: WriteEngineDeps,
  script: ScriptRuntime,
  args: Record<string, unknown>,
  meta?: Readonly<Record<string, unknown>>,
): Promise<ResponseEnvelope<unknown>> {
  const negotiation = negotiateEnvelopeVersion(meta);
  if (!negotiation.ok) {
    return withWorkspaceNotes(workspace, negotiation.envelope);
  }
  if (typeof args["source"] !== "string" || args["source"].length === 0) {
    throw new McpError(ErrorCode.InvalidParams, "script: source must be a non-empty string");
  }
  if (args["confirm"] !== undefined && typeof args["confirm"] !== "string") {
    throw new McpError(ErrorCode.InvalidParams, "script: confirm must be a string token");
  }
  for (const key of ["fuel", "memoryCapBytes"] as const) {
    const value = args[key];
    if (
      value !== undefined &&
      (typeof value !== "number" || !Number.isInteger(value) || value < 1)
    ) {
      throw new McpError(ErrorCode.InvalidParams, `script: ${key} must be a positive integer`);
    }
  }
  const confirm: ConfirmPort = {
    mint: (diffHash, contentHash) =>
      mintConfirmGrant(ensureWorkspaceKey(workspace.stateDir), diffHash, contentHash),
    check: (token) => checkConfirmGrant(ensureWorkspaceKey(workspace.stateDir), token),
  };
  const deps: ScriptDeps = {
    search,
    write,
    confirm,
    index: search.index,
    // The jail decides every guest read before any filesystem access exists
    // (SDD §7.1); a refusal is a capability violation at the script layer.
    openRead: (file) => {
      const decision = canonicalizeWithin(workspace.root, file);
      if (!decision.ok) {
        return Promise.resolve({ ok: false as const, reason: decision.reason });
      }
      return Promise.resolve({
        ok: true as const,
        access: {
          read: () => readFile(decision.path, "utf8"),
          stat: async () => {
            const s = await stat(decision.path);
            return { mtimeMs: s.mtimeMs, size: s.size };
          },
        },
      });
    },
    runGuest: (argv, policy) => script.runGuest(argv, policy),
    list: () => search.workspace.list(),
  };
  const envelope = await runScript(
    deps,
    args as unknown as ScriptInput,
    negotiation.v,
    sessionIdFrom(meta),
  );
  return withWorkspaceNotes(workspace, envelope);
}

async function runToolEnvelope(
  workspace: WorkspaceRuntime,
  run: RunRuntime,
  args: Record<string, unknown>,
  meta?: Readonly<Record<string, unknown>>,
): Promise<ResponseEnvelope<unknown>> {
  const negotiation = negotiateEnvelopeVersion(meta);
  if (!negotiation.ok) {
    return withWorkspaceNotes(workspace, negotiation.envelope);
  }
  if (typeof args["command"] !== "string" || args["command"].length === 0) {
    throw new McpError(ErrorCode.InvalidParams, "run: command must be a non-empty string");
  }
  try {
    const envelope =
      run.adapters === undefined
        ? await runRun(
            // The allowlist comes from the merged config, never the
            // composition (§9.6): org policy pinning and human-only
            // amendment already applied.
            { commands: workspace.config.commands, execute: run.execute },
            args as unknown as RunInput,
            negotiation.v,
          )
        : await runRun(
            { commands: workspace.config.commands, execute: run.execute },
            args as unknown as RunInput,
            negotiation.v,
            run.adapters,
          );
    return withWorkspaceNotes(workspace, envelope);
  } catch (error) {
    if (error instanceof RangeError) {
      throw new McpError(ErrorCode.InvalidParams, error.message);
    }
    throw error;
  }
}

async function undoEnvelope(
  workspace: WorkspaceRuntime,
  write: WriteEngineDeps,
  args: Record<string, unknown>,
  meta?: Readonly<Record<string, unknown>>,
): Promise<ResponseEnvelope<unknown>> {
  const negotiation = negotiateEnvelopeVersion(meta);
  if (!negotiation.ok) {
    return withWorkspaceNotes(workspace, negotiation.envelope);
  }
  const action = args["action"];
  if (action !== undefined && action !== "rewind" && action !== "list") {
    throw new McpError(ErrorCode.InvalidParams, 'undo: action must be "rewind" or "list"');
  }
  const steps = args["steps"];
  if (
    steps !== undefined &&
    (typeof steps !== "number" || !Number.isInteger(steps) || steps < 1 || steps > UNDO_STEPS_MAX)
  ) {
    throw new McpError(
      ErrorCode.InvalidParams,
      `undo: steps must be an integer in [1, ${UNDO_STEPS_MAX}]`,
    );
  }
  const ops = args["ops"];
  if (ops !== undefined) {
    if (!Array.isArray(ops) || ops.length === 0 || ops.length > UNDO_STEPS_MAX) {
      throw new McpError(
        ErrorCode.InvalidParams,
        `undo: ops must be an array of 1 to ${UNDO_STEPS_MAX} journal seqs`,
      );
    }
    for (const seq of ops) {
      if (typeof seq !== "number" || !Number.isInteger(seq) || seq < 1) {
        throw new McpError(
          ErrorCode.InvalidParams,
          "undo: ops must be positive integer journal seqs",
        );
      }
    }
  }
  const limit = args["limit"];
  if (
    limit !== undefined &&
    (typeof limit !== "number" ||
      !Number.isInteger(limit) ||
      limit < 1 ||
      limit > UNDO_LIST_LIMIT_MAX)
  ) {
    throw new McpError(
      ErrorCode.InvalidParams,
      `undo: limit must be an integer in [1, ${UNDO_LIST_LIMIT_MAX}]`,
    );
  }
  if (args["diff"] !== undefined && typeof args["diff"] !== "boolean") {
    throw new McpError(ErrorCode.InvalidParams, "undo: diff must be a boolean");
  }
  for (const name of ["sinceSeq", "beforeSeq"] as const) {
    const value = args[name];
    if (
      value !== undefined &&
      (typeof value !== "number" || !Number.isInteger(value) || value < 0)
    ) {
      throw new McpError(
        ErrorCode.InvalidParams,
        `undo: ${name} must be a non-negative integer journal seq`,
      );
    }
  }
  if (args["scope"] !== undefined && args["scope"] !== "session" && args["scope"] !== "workspace") {
    throw new McpError(ErrorCode.InvalidParams, 'undo: scope must be "session" or "workspace"');
  }
  const policy = args["onConflict"];
  if (
    policy !== undefined &&
    policy !== "surface" &&
    policy !== "force" &&
    policy !== "keepCurrent"
  ) {
    throw new McpError(
      ErrorCode.InvalidParams,
      'undo: onConflict must be "surface", "force" or "keepCurrent"',
    );
  }
  const envelope = await runUndo(
    write,
    args as unknown as UndoInput,
    negotiation.v,
    sessionIdFrom(meta),
  );
  return withWorkspaceNotes(workspace, envelope);
}

/**
 * Build a server over the workspace runtime and the composed search
 * dependencies. Cheap by design: the stateless HTTP path builds one per
 * request; the injected deps are workspace infrastructure, shared like the
 * workspace database. `write` — the composed §7.5 write-engine deps, jail
 * already applied by the composer — is optional while the write path rolls
 * out: without it the mutation surface (undo) is not offered at all.
 */
/**
 * The root span's outcome, read from the sealed envelope at the choke point
 * (feature 005): taxonomy code + recovery when the request failed, plus the
 * allowlisted result attributes. `null` — emit nothing — when the payload is
 * not an envelope this tracer can represent (unreachable for envelopes built
 * by the sanctioned constructors).
 */
function spanOutcome(
  sealed: Record<string, unknown>,
): { readonly status: SpanStatus; readonly result: RootEndAttributes } | null {
  const result: RootEndAttributes = {
    ...(typeof sealed["v"] === "number" ? { envelopeV: sealed["v"] } : {}),
    ...(Array.isArray(sealed["notes"]) ? { notesCount: sealed["notes"].length } : {}),
  };
  if (sealed["ok"] === true) {
    return {
      status: { ok: true },
      result: { ...result, truncated: sealed["truncation"] !== undefined },
    };
  }
  const code = (sealed["error"] as { code?: unknown } | undefined)?.code;
  if (!isErrorCode(code)) return null;
  return { status: { ok: false, code }, result };
}

export function buildServer(
  workspace: WorkspaceRuntime,
  search: SearchDeps,
  write?: WriteEngineDeps,
  script?: ScriptRuntime,
  plugins?: PluginsRuntime,
  run?: RunRuntime,
  tracer: Tracer = noopTracer,
): Server {
  // `edit.confirm_mode = "token"` (§7.2) routes every confirm-gated write
  // through the two-call token flow even when the client could answer an
  // elicitation: the agent confirms, the human is not interrupted.
  const elicitAllowed = workspace.config.effective["edit.confirm_mode"] !== "token";
  const server = new Server(
    { name: SERVER_NAME, version: SERVER_VERSION },
    // Tasks (SDD §5.1): list only — no cancel (a Tier-1 build is cheaper to
    // finish than to abort) and no task-augmented requests yet (M2's rewrite
    // tasks will add `requests` when they land).
    { capabilities: { tools: {}, tasks: { list: {} } } },
  );
  // Core tools first, plugin tools appended under their derived namespaces
  // (SDD §8.3). A broken manifest drops its plugin from the list, never the
  // list itself — core tools continue at full fidelity.
  server.setRequestHandler(ListToolsRequestSchema, async () => {
    const core = toolList(write, script, run);
    if (plugins === undefined) return { tools: core };
    const listed = await listPluginTools(pluginHostDeps(workspace, plugins));
    const pluginTools: Tool[] = listed.tools.map((tool) => ({
      name: tool.name,
      title: tool.title,
      description: tool.description,
      // Plugin payloads are envelope-versioned JSON by decision (§8.2/§8.3):
      // the host passes the whole argument object through as the payload, so
      // the wire schema is deliberately open. The plugin's own contract lives
      // in its manifest; the host wraps every output in the envelope.
      inputSchema: { type: "object", additionalProperties: true },
      outputSchema: envelopeSchema as unknown as Tool["outputSchema"],
    }));
    return { tools: [...core, ...pluginTools] };
  });
  // The Tasks extension, statelessly (SDD §5.1, §7.3.2): every answer is
  // derived on the spot from the engine child's build status — no task store.
  // Asking about tasks may lazily spawn the engine child; the answer it gives
  // (no build ⇒ no tasks) is the same one a cold child implies.
  server.setRequestHandler(ListTasksRequestSchema, async () => {
    const task = indexBuildTask(await search.index.status());
    return { tasks: task === null ? [] : [task] };
  });
  server.setRequestHandler(GetTaskRequestSchema, async (request) => {
    const task = indexBuildTask(await search.index.status());
    if (task === null || task.taskId !== request.params.taskId) {
      throw new McpError(ErrorCode.InvalidParams, "Failed to retrieve task: Task not found");
    }
    return { ...task };
  });
  server.setRequestHandler(CallToolRequestSchema, async (request, extra) => {
    // The root-span choke point (feature 005, research R2): every core case
    // and the plugin default branch pass through this one dispatch, so no
    // tool can dodge the flight recorder. A throw carrying no taxonomy code
    // (protocol-level rejection, caller cancellation) ends no span — the
    // record stays contract-clean by construction.
    const span = tracer.startToolSpan({ tool: request.params.name });
    try {
      const result = await tracer.runWith(span, () => dispatchTool());
      const outcome = spanOutcome(result.structuredContent);
      if (outcome !== null) span.end(outcome.status, outcome.result);
      return result;
    } catch (error) {
      const code = (error as { code?: unknown }).code;
      if (isErrorCode(code)) span.end({ ok: false, code });
      throw error;
    }

    async function dispatchTool(): Promise<ReturnType<typeof toolResult>> {
      const meta = request.params._meta;
      const args = (request.params.arguments ?? {}) as Record<string, unknown>;
      switch (request.params.name) {
        case "workbench_status":
          // Ranking-signal state (chunk:m5-ranking-shadow) rides the notes
          // channel — schema-free, per SDD §5.2.
          return toolResult(
            statusEnvelope(workspace, meta, search.rankingSignals?.statusNotes() ?? []),
          );
        case "map":
          return toolResult(await mapEnvelope(workspace, search, args, meta));
        case "read":
          return toolResult(await readEnvelope(workspace, search, args, meta));
        case "search":
          // §7.7: the MCP request's abort signal rides into the engine so a
          // client cancellation actually stops the search burning cycles.
          return toolResult(await searchEnvelope(workspace, search, args, meta, extra.signal));
        case "edit": {
          if (write === undefined) {
            throw new McpError(
              ErrorCode.MethodNotFound,
              "edit is not offered: this server was composed without the write path",
            );
          }
          let envelope = await editEnvelope(workspace, search, write, args, meta);
          // MRTR (SDD §5.1, §9.4): a client that can answer elicitations gets
          // the dry-run→confirm contract inside this one call — an accept
          // re-runs the identical call with the minted token. Every other
          // client keeps the equal-path token fallback: CONFIRM_REQUIRED
          // carries confirmToken; a second call with confirm applies.
          const token = pendingConfirmToken(envelope);
          if (
            token !== null &&
            elicitAllowed &&
            server.getClientCapabilities()?.elicitation !== undefined
          ) {
            if (await elicitConfirm(server, envelope, extra.signal)) {
              envelope = await editEnvelope(
                workspace,
                search,
                write,
                { ...args, confirm: token },
                meta,
              );
            }
          }
          return toolResult(envelope);
        }
        case "files": {
          if (write === undefined) {
            throw new McpError(
              ErrorCode.MethodNotFound,
              "files is not offered: this server was composed without the write path",
            );
          }
          let envelope = await filesEnvelope(workspace, search, write, args, meta);
          // MRTR (SDD §5.1, §9.5): identical contract to edit — an accepted
          // elicitation re-runs the identical call with the minted token.
          const token = pendingConfirmToken(envelope);
          if (
            token !== null &&
            elicitAllowed &&
            server.getClientCapabilities()?.elicitation !== undefined
          ) {
            if (await elicitConfirm(server, envelope, extra.signal)) {
              envelope = await filesEnvelope(
                workspace,
                search,
                write,
                { ...args, confirm: token },
                meta,
              );
            }
          }
          return toolResult(envelope);
        }
        case "script": {
          if (write === undefined || script === undefined) {
            throw new McpError(
              ErrorCode.MethodNotFound,
              "script is not offered: this server was composed without the sandbox lane (§6.2)",
            );
          }
          let envelope = await scriptEnvelope(workspace, search, write, script, args, meta);
          // MRTR (SDD §5.1, §9.7): identical contract to edit — an accepted
          // elicitation re-runs the identical call with the minted token; the
          // script re-runs deterministically, reproducing the queue.
          const token = pendingConfirmToken(envelope);
          if (
            token !== null &&
            elicitAllowed &&
            server.getClientCapabilities()?.elicitation !== undefined
          ) {
            if (await elicitConfirm(server, envelope, extra.signal)) {
              envelope = await scriptEnvelope(
                workspace,
                search,
                write,
                script,
                { ...args, confirm: token },
                meta,
              );
            }
          }
          return toolResult(envelope);
        }
        case "run":
          if (run === undefined) {
            throw new McpError(
              ErrorCode.MethodNotFound,
              "run is not offered: this server was composed without an executor lane (§6.2)",
            );
          }
          return toolResult(await runToolEnvelope(workspace, run, args, meta));
        case "undo":
          if (write === undefined) {
            throw new McpError(
              ErrorCode.MethodNotFound,
              "undo is not offered: this server was composed without the write path",
            );
          }
          return toolResult(await undoEnvelope(workspace, write, args, meta));
        default: {
          // Plugin tools (SDD §8.3): dispatched only AFTER every core case, and
          // only for namespaced names — a plugin cannot shadow, wrap, intercept
          // or rename a core tool because a core name never reaches this branch
          // and a derived wire name always contains "/".
          if (plugins !== undefined && isPluginToolName(request.params.name)) {
            const negotiation = negotiateEnvelopeVersion(meta);
            if (!negotiation.ok) {
              return toolResult(withWorkspaceNotes(workspace, negotiation.envelope));
            }
            const envelope = await runPluginTool(
              pluginHostDeps(workspace, plugins),
              request.params.name,
              args,
              negotiation.v,
            );
            if (envelope !== null) {
              return toolResult(withWorkspaceNotes(workspace, envelope));
            }
          }
          throw new McpError(
            ErrorCode.MethodNotFound,
            `unknown tool ${JSON.stringify(request.params.name)}`,
          );
        }
      }
    }
  });
  return server;
}
