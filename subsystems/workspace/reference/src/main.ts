#!/usr/bin/env bun
/**
 * The one workbench-mcp binary, two roles (SDD §6.1): the default role is
 * the MCP server; `--engine` runs the engine child that owns every native
 * parse. One binary means version skew between the two processes is
 * impossible and the frame protocol stays brutally simple.
 *
 * Server usage: `workbench-mcp [workspace] [--http [port]]`
 *   stdio (default)   MCP over stdio; stdout carries only the transport.
 *   --http [port]     single-replica stateless Streamable HTTP on /mcp
 *                     (port 0 = OS-assigned; the chosen port prints to stderr).
 *
 * Key management: `workbench-mcp key rotate [workspace]` replaces the
 * per-workspace MRTR confirm-token key (SDD §7.1), invalidating every
 * outstanding confirm token.
 *
 * Init: `workbench-mcp init [workspace]` seeds run's `[commands]` allowlist
 * into workbench.toml from lockfile/manifest detection (SDD §9.6); an
 * existing `[commands]` table is never touched — it is amendable only by a
 * human from then on.
 *
 * Journal: `workbench-mcp journal [workspace] [--limit N] [--since SEQ]
 * [--before SEQ] [--session ID] [--no-diff] [--json]` prints the write
 * journal — the rows `undo action:"list"` returns — read-only and WITHOUT
 * the workspace lock, so it answers beside a live server the way `doctor`
 * does.
 *
 * Doctor: `workbench-mcp doctor config [workspace] [--config key=value ...]`
 * prints the five-layer resolved configuration with per-key provenance
 * (SDD §7.2) — no lock taken, so it runs beside a live server.
 *
 * Full doctor (SDD §11.2): `workbench-mcp doctor [workspace]` prints the
 * six-section health report (environment, index, config provenance,
 * plugins, quarantine, crashes) read-only over the .workbench artifacts;
 * `workbench-mcp doctor bundle [workspace] [--out <path>]` packs the same
 * probes into a redacted tar.gz — structure and symptoms, never repo file
 * contents. Exit 0 = healthy, 1 = findings present.
 *
 * Per-session config flags: repeatable `--config key=value` sets the
 * highest-precedence layer (§7.2) for this server process.
 *
 * Exit codes: 64 usage · 65 durable data this binary cannot read · 66
 * workspace root missing · 75 workspace already locked by a live instance
 * (retry after it exits).
 */

import { existsSync, readFileSync, writeFileSync } from "node:fs";
import { appendFile, readFile, stat, writeFile } from "node:fs/promises";
import { join, resolve } from "node:path";
import { BiomeFormatter } from "./adapters/biome";
import {
  EngineSupervisor,
  fallbackScanWorkspace,
  IpcEngineAdapter,
  IpcIndexAdapter,
  runEngineChild,
  SANDBOX_COMPONENT_METHOD,
  SANDBOX_LANG_PACK_METHOD,
  SANDBOX_RANKING_METHOD,
  SANDBOX_RESOLVER_METHOD,
  SANDBOX_RUNNER_METHOD,
  SANDBOX_WASI_METHOD,
} from "./adapters/engine-ipc";
import { startWatcher } from "./adapters/fswatch";
import {
  JournalDb,
  type JournalReader,
  JournalUnreadableError,
  openJournalReader,
} from "./adapters/journal-db";
import { passthroughFormatter } from "./adapters/passthrough-formatter";
import { RustfmtFormatter, routingFormatter } from "./adapters/rustfmt";
import { WorkspaceWriteLock } from "./adapters/workspace-lock";
import {
  detectCommands,
  parseSessionFlags,
  renderCommandsTable,
  renderDoctorConfig,
  resolveConfig,
} from "./config";
import { UnsupportedVersionError } from "./data/versioned-db";
import { buildBundle } from "./doctor/bundle";
import { runDoctor } from "./doctor/probes";
import { renderDoctorReport } from "./doctor/report";
import { ENVELOPE_VERSION_LATEST, okEnvelope, type ResponseEnvelope } from "./envelope";
import { rotateWorkspaceKey } from "./jail";
import { noopTracer, type Tracer } from "./ports";
import { bootLangPacks, type LangPackLoadInvoke } from "./protocol/lang-pack-plugin";
import { type RankingContributeInvoke, RankingSignals } from "./protocol/ranking-plugin";
import {
  detectLanguageServers,
  PluginResolver,
  type ResolverPluginDeps,
} from "./protocol/resolver-plugin";
import { componentRunnerAdapter, type RunnerLaneInvoke } from "./protocol/runner-plugin";
import { serveHttp, serveStdio } from "./protocol/serve";
import type { PluginsRuntime, RunRuntime, ScriptRuntime } from "./protocol/server";
import { pluginHostDeps, SERVER_VERSION } from "./protocol/server";
import { JsonlTracer } from "./protocol/tracing";
import { openWorkspace, WorkspaceLockedError, type WorkspaceRuntime } from "./protocol/workspace";
import { createWriteAccess } from "./protocol/write-access";
import type { PluginInvokeOutcome, ScriptGuestOutcome, SearchDeps, WriteEngineDeps } from "./tools";
import {
  BUNDLED_RUNNER_ADAPTERS,
  JOURNAL_DIFF_CHARS_MAX,
  type JournalRow,
  journalRows,
  UNDO_LIST_LIMIT_DEFAULT,
  UNDO_LIST_LIMIT_MAX,
} from "./tools";

/**
 * Compose the search tool's ports over the engine child. The supervisor is
 * lazy — the child is not spawned until the first engine call — so
 * status-only servers never fork a process. The first workspace walk kicks a
 * non-blocking Tier-1 build (SDD §7.3.2): Tier-0 answers stay available
 * while the index warms.
 */
function composeSearch(
  workspace: WorkspaceRuntime,
  tracer: Tracer,
): {
  readonly search: SearchDeps;
  stop(): Promise<void>;
  readonly engine: SearchDeps["engine"];
  readonly script: ScriptRuntime;
  readonly plugins: PluginsRuntime;
  readonly resolveGuest: ResolverPluginDeps["invokeResolver"];
  readonly runnerParse: RunnerLaneInvoke;
  readonly langPackLoad: LangPackLoadInvoke;
  readonly rankingContribute: RankingContributeInvoke;
} {
  // The quarantine record's durable home (SDD §6.2, v1.3): appended under the
  // jail-protected state dir so `doctor` can read it without repo access.
  const supervisor = new EngineSupervisor({
    quarantineLog: join(workspace.stateDir, "quarantine.jsonl"),
    tracer,
  });
  const index = new IpcIndexAdapter(supervisor, workspace.root, {
    journalMode: workspace.journalMode,
  });
  // The file watcher (specs/011-rust-watcher-wiring): edits made outside the
  // tools — cargo fmt, git checkout, IDE saves — reach the Tier-1 index
  // through the bounded bridge. `watcher.mode` (auto | native | poll | off)
  // is resolved against the same network-fs detection that picks the journal
  // mode; off wires nothing. Started in the background (nothing waits on it),
  // publishes .workbench/watcher.json for doctor, and is closed FIRST on stop
  // so no refresh is issued against a child that is going away.
  const watcherStarted = startWatcher({
    root: workspace.root,
    stateDir: workspace.stateDir,
    index,
    mode: workspace.config.effective["watcher.mode"],
    networkFs: workspace.detection.networkFs,
  }).catch(() => null);
  let buildKicked = false;
  const list = (): Promise<readonly { path: string; junk: boolean }[]> => {
    if (!buildKicked) {
      buildKicked = true;
      index.build(workspace.root).catch(() => {
        // The kick is best-effort; lexical answers are Tier-0 capable.
      });
    }
    return Promise.resolve(
      fallbackScanWorkspace(workspace.root).map((e) => ({ path: e.path, junk: e.junk })),
    );
  };
  const read = (path: string): Promise<string> =>
    Promise.resolve(readFileSync(join(workspace.root, path), "utf8"));
  const engine = new IpcEngineAdapter(supervisor, {
    read,
    list: async () => (await list()).map((f) => f.path),
  });
  // The `script` guest lane (SDD §9.7): the vendored QuickJS-ng interpreter
  // executed in the engine child under the §8.1 sandbox policy. The wasm
  // path rides every request; the child caches the bytes by mtime+size.
  const qjsWasmPath = resolve(import.meta.dirname, "../vendor/qjs/qjs-wasi.wasm");
  const script: ScriptRuntime = {
    runGuest: (argv, policy) =>
      supervisor.call(SANDBOX_WASI_METHOD, {
        wasmPath: qjsWasmPath,
        argv: [...argv],
        fuel: policy.fuel,
        memoryCapBytes: policy.memoryCapBytes,
      }) as Promise<ScriptGuestOutcome>,
  };
  // Plugin tools (SDD §8.3, chunk:m4-capability-manifests): the engine
  // child's component lane, under the same §8.1 sandbox policy machinery.
  const plugins: PluginsRuntime = {
    invokeGuest: (absPath, exportName, payload, capabilities, policy) =>
      supervisor.call(SANDBOX_COMPONENT_METHOD, {
        wasmPath: absPath,
        exportName,
        payload,
        capabilities: [...capabilities],
        fuel: policy.fuel,
        memoryCapBytes: policy.memoryCapBytes,
      }) as Promise<PluginInvokeOutcome>,
  };
  // The typed resolver lane (chunk:m5-lsp-resolver-plugin): same child, same
  // policy machinery, carrying the host-curated detection table per invoke.
  const resolveGuest: ResolverPluginDeps["invokeResolver"] = (
    absPath,
    symbol,
    capabilities,
    policy,
    specs,
    root,
  ) =>
    supervisor.call(SANDBOX_RESOLVER_METHOD, {
      wasmPath: absPath,
      symbol,
      capabilities: [...capabilities],
      fuel: policy.fuel,
      memoryCapBytes: policy.memoryCapBytes,
      subprocessSpecs: specs.map((s) => ({ ...s, args: [...s.args] })),
      subprocessRoot: root,
    }) as Promise<import("./protocol/resolver-plugin").ResolverGuestOutcome>;
  // The typed runner lane (chunk:m5-runner-adapter): same child, same policy.
  const runnerParse: RunnerLaneInvoke = (
    absPath,
    command,
    exitCode,
    stdout,
    stderr,
    capabilities,
    policy,
  ) =>
    supervisor.call(SANDBOX_RUNNER_METHOD, {
      wasmPath: absPath,
      command,
      exitCode,
      stdout,
      stderr,
      capabilities: [...capabilities],
      fuel: policy.fuel,
      memoryCapBytes: policy.memoryCapBytes,
    }) as Promise<import("./protocol/runner-plugin").RunnerGuestOutcome>;
  // The lang-pack carrier lane (chunk:m5-lang-pack): the child reads the
  // pack's exports and registers the language; `blessed` selects the lane.
  const langPackLoad: LangPackLoadInvoke = (absPath, blessed, capabilities, policy) =>
    supervisor.call(SANDBOX_LANG_PACK_METHOD, {
      wasmPath: absPath,
      blessed,
      capabilities: [...capabilities],
      fuel: policy.fuel,
      memoryCapBytes: policy.memoryCapBytes,
    }) as Promise<import("./protocol/lang-pack-plugin").LangPackGuestOutcome>;
  // The batched ranking lane (chunk:m5-ranking-shadow): same child, same
  // policy; callers keep it OFF the search response path (SC-005).
  const rankingContribute: RankingContributeInvoke = (absPath, hits, capabilities, policy) =>
    supervisor.call(SANDBOX_RANKING_METHOD, {
      wasmPath: absPath,
      hits: hits.map((h) => ({ ...h })),
      capabilities: [...capabilities],
      fuel: policy.fuel,
      memoryCapBytes: policy.memoryCapBytes,
    }) as Promise<import("./protocol/ranking-plugin").RankingGuestOutcome>;
  return {
    search: { engine, index, workspace: { list, read } },
    stop: async () => {
      const watcher = await watcherStarted;
      await watcher?.close();
      await supervisor.stop();
    },
    engine,
    script,
    plugins,
    resolveGuest,
    runnerParse,
    langPackLoad,
    rankingContribute,
  };
}

/**
 * Compose the §7.5 write path: the durable journal (§7.6), the per-workspace
 * advisory write lock (§7.7), and jail-checked file access (§7.1 — the jail
 * decides before any filesystem access exists). The formatter is the Biome
 * adapter (chunk:backlog-biome-formatter): the §7.5 pipeline runs format +
 * re-verify around it and falls back to verified unformatted content when it
 * throws; undo never formats — exact-bytes restoration is its contract.
 */
function composeWrite(
  workspace: WorkspaceRuntime,
  engine: SearchDeps["engine"],
): { readonly write: WriteEngineDeps; readonly journal: JournalDb } {
  const journal = new JournalDb({
    path: join(workspace.stateDir, "journal.db"),
    journalMode: workspace.journalMode,
  });
  // Jail-checked access with the §7.6 trash-backed remove — nothing the
  // write engine touches is ever unlinked destructively.
  const access = createWriteAccess({ root: workspace.root, stateDir: workspace.stateDir });
  // `format.enabled = false` (§7.2) turns the format step into a no-op: the
  // write engine still parse-verifies, journals and swaps atomically — only
  // the restyling is skipped. Load-bearing on a workspace that does not keep
  // its tree formatter-clean, where formatting a whole file on every write
  // buries the intended change in unrelated restyling.
  const formatEnabled = workspace.config.effective["format.enabled"] !== false;
  const write: WriteEngineDeps = {
    engine,
    // specs/012: `.rs` → the workspace's rustfmt (rust-toolchain.toml honored,
    // edition passed, rustfmt.toml discovered from the file's directory);
    // everything else → Biome exactly as before. A missing or failing rustfmt
    // degrades to the verified unformatted content with a note (§7.5).
    formatter: formatEnabled
      ? routingFormatter({
          rules: [
            { extensions: ["rs"], formatter: new RustfmtFormatter({ root: workspace.root }) },
          ],
          fallback: new BiomeFormatter({ root: workspace.root }),
        })
      : passthroughFormatter(),
    journal,
    workspace: access,
    lock: new WorkspaceWriteLock(workspace.root),
  };
  return { write, journal };
}

/**
 * Compose the `run` executor (SDD §9.6): one argv, no shell, hard wall-clock
 * kill. The allowlist never travels through this seam — buildServer reads it
 * from the merged config, so this is pure mechanics. A spawn failure (the
 * declared binary is missing) is data, not a crash: the tool's fallback row
 * carries the message.
 */
function composeRun(workspace: WorkspaceRuntime, runnerParse: RunnerLaneInvoke): RunRuntime {
  // The bundled go adapter rides the public workbench:runner@0.1.0 component
  // (chunk:m5-runner-adapter); its TS twin stays as the degradation floor.
  // Trust order and every other bundled adapter are unchanged.
  const goTwin = BUNDLED_RUNNER_ADAPTERS.find((a) => a.name === "go");
  const adapters =
    goTwin === undefined
      ? undefined
      : BUNDLED_RUNNER_ADAPTERS.map((a) =>
          a === goTwin
            ? componentRunnerAdapter({
                name: goTwin.name,
                guestPath: resolve(import.meta.dirname, "../plugins/runner-go/guest.wasm"),
                invoke: runnerParse,
                fallback: goTwin,
              })
            : a,
        );
  return {
    ...(adapters === undefined ? {} : { adapters }),
    execute: async (argv, opts) => {
      const started = Date.now();
      try {
        const proc = Bun.spawn([...argv], {
          cwd: workspace.root,
          stdin: "ignore",
          stdout: "pipe",
          stderr: "pipe",
        });
        let timedOut = false;
        const timer = setTimeout(() => {
          timedOut = true;
          proc.kill();
        }, opts.timeoutMs);
        const [stdout, stderr, exitCode] = await Promise.all([
          new Response(proc.stdout).text(),
          new Response(proc.stderr).text(),
          proc.exited,
        ]);
        clearTimeout(timer);
        return {
          exitCode: timedOut ? null : exitCode,
          stdout,
          stderr,
          timedOut,
          durationMs: Date.now() - started,
        };
      } catch (error) {
        return {
          exitCode: null,
          stdout: "",
          stderr: `spawn failed: ${error instanceof Error ? error.message : String(error)}`,
          timedOut: false,
          durationMs: Date.now() - started,
        };
      }
    },
  };
}

/** The `journal` CLI's envelope — the shape `undo action:"list"` returns. */
function journalEnvelope(
  rows: readonly JournalRow[],
  notes: readonly string[],
): ResponseEnvelope<JournalRow> {
  return okEnvelope({
    v: ENVELOPE_VERSION_LATEST,
    freshness: "warm",
    budget: { requested: 0, used: Math.ceil(JSON.stringify(rows).length / 4) },
    results: rows,
    ...(notes.length === 0 ? {} : { notes }),
  });
}

/** The human rendering: one header line per op, one line per file, diffs last. */
function renderJournal(rows: readonly JournalRow[]): string {
  const out: string[] = [];
  let currentSeq: number | undefined;
  for (const row of rows) {
    if (row.seq !== currentSeq) {
      currentSeq = row.seq;
      out.push(
        `#${row.seq}  ${new Date(row.at).toISOString()}  ${row.tool}` +
          `${row.sessionId === "" ? "" : `  session=${row.sessionId}`}` +
          `  ${row.undoable ? "undoable" : `undone${row.undoneBy === undefined ? "" : ` by #${row.undoneBy}`}`}` +
          `${row.undoOf === undefined ? "" : `  (undo of ${row.undoOf.join(", ")})`}`,
      );
    }
    if (row.path === undefined) {
      out.push("    (no file changed)");
      continue;
    }
    const mark = row.change === "create" ? "A" : row.change === "delete" ? "D" : "M";
    out.push(
      `    ${mark} ${row.path}  +${row.added} -${row.removed}` +
        `${row.evicted === true ? "  [blob evicted — history only]" : ""}`,
    );
    if (row.diff !== undefined && row.diff.length > 0) {
      for (const line of row.diff.split("\n")) out.push(`      ${line}`);
      if (row.diffTruncated === true) out.push("      … diff cut at the per-row cap");
    }
  }
  return out.length === 0 ? "(the journal is empty)" : out.join("\n");
}

function fail(code: number, message: string): never {
  console.error(`workbench-mcp: ${message}`);
  process.exit(code);
}

if (process.argv.includes("--engine")) {
  await runEngineChild();
} else if (process.argv[2] === "key") {
  if (process.argv[3] !== "rotate") {
    fail(64, "usage: workbench-mcp key rotate [workspace]");
  }
  const root = process.argv[4] ?? ".";
  const stateDir = join(root, ".workbench");
  if (!existsSync(stateDir)) {
    fail(66, `not a workbench workspace (no ${stateDir}) — start the server once first`);
  }
  rotateWorkspaceKey(stateDir);
  console.error("workbench-mcp: workspace key rotated; outstanding confirm tokens are void");
} else if (process.argv[2] === "init") {
  // Seed run's [commands] allowlist from lockfile/manifest detection
  // (SDD §9.6). A human runs this; the served surface can never write
  // workbench.toml (PROTECTED_FILE), so seeding is the one sanctioned
  // machine-assisted amendment — and even it refuses to touch an existing
  // table.
  const root = process.argv[3] ?? ".";
  const absRoot = resolve(process.cwd(), root);
  if (!existsSync(absRoot)) fail(66, `workspace root does not exist: ${absRoot}`);
  const tomlPath = join(absRoot, "workbench.toml");
  const detected = detectCommands(absRoot);
  if (detected.length === 0) {
    console.error("workbench-mcp: no project commands detected — nothing seeded");
  } else {
    const existing = existsSync(tomlPath) ? readFileSync(tomlPath, "utf8") : "";
    if (/^\s*\[commands\]/m.test(existing)) {
      console.error(
        "workbench-mcp: workbench.toml already declares [commands] — left untouched (amend by hand)",
      );
    } else {
      const block = renderCommandsTable(detected);
      writeFileSync(
        tomlPath,
        existing.length === 0 ? block : `${existing.replace(/\n*$/, "\n\n")}${block}`,
      );
      console.error(`workbench-mcp: seeded ${detected.length} command(s) into ${tomlPath}`);
      for (const d of detected) {
        console.error(`  ${d.name} = ${JSON.stringify(d.line)}  (${d.provenance})`);
      }
    }
  }
} else if (process.argv[2] === "journal") {
  // The journal, read beside a live server (SDD §9.8, §11.2's precedent):
  // read-only, NO lock, no PRAGMA, no migration — so a cockpit can list the
  // write journal of a workspace an agent's own server currently owns. The
  // rows are the ones `undo action:"list"` returns, rendered by the same
  // pure function, and --json wraps them in the same envelope so a consumer
  // has one shape to parse either way.
  const usage =
    "usage: workbench-mcp journal [workspace] [--limit N] [--since SEQ] [--before SEQ] " +
    "[--session ID] [--no-diff] [--json]";
  let root = ".";
  let limit = UNDO_LIST_LIMIT_DEFAULT;
  let since: number | undefined;
  let before: number | undefined;
  let session: string | undefined;
  let diff = true;
  let asJson = false;
  const rest = process.argv.slice(3);
  const numeric = (value: string | undefined, flag: string): number => {
    if (value === undefined) fail(64, usage);
    const parsed = Number(value);
    if (!Number.isInteger(parsed) || parsed < 0) fail(64, `${flag} expects a journal seq`);
    return parsed;
  };
  for (let i = 0; i < rest.length; i++) {
    const arg = rest[i];
    if (arg === "--limit") {
      limit = numeric(rest[i + 1], "--limit");
      if (limit < 1 || limit > UNDO_LIST_LIMIT_MAX) {
        fail(64, `--limit expects an integer in [1, ${UNDO_LIST_LIMIT_MAX}]`);
      }
      i++;
    } else if (arg === "--since") {
      since = numeric(rest[i + 1], "--since");
      i++;
    } else if (arg === "--before") {
      before = numeric(rest[i + 1], "--before");
      i++;
    } else if (arg === "--session") {
      const next = rest[i + 1];
      if (next === undefined) fail(64, usage);
      session = next;
      i++;
    } else if (arg === "--no-diff") {
      diff = false;
    } else if (arg === "--json") {
      asJson = true;
    } else if (arg?.startsWith("--")) {
      fail(64, usage);
    } else if (arg !== undefined) {
      root = arg;
    }
  }
  const absRoot = resolve(process.cwd(), root);
  if (!existsSync(absRoot)) fail(66, `workspace root does not exist: ${absRoot}`);
  const journalPath = join(absRoot, ".workbench", "journal.db");
  const rows: readonly JournalRow[] = [];
  if (!existsSync(journalPath)) {
    const note = `no journal yet at ${journalPath} — nothing has been written through this workspace`;
    if (asJson) console.log(JSON.stringify(journalEnvelope(rows, [note])));
    else console.error(`workbench-mcp: ${note}`);
  } else {
    let reader: JournalReader | undefined;
    try {
      reader = openJournalReader(journalPath);
      const entries = reader.recent({
        limit,
        ...(session === undefined ? {} : { sessionId: session }),
        ...(since === undefined ? {} : { sinceSeq: since }),
        ...(before === undefined ? {} : { beforeSeq: before }),
      });
      const listed = journalRows(entries, { diff, diffCharCap: JOURNAL_DIFF_CHARS_MAX });
      if (asJson) console.log(JSON.stringify(journalEnvelope(listed, [])));
      else console.log(renderJournal(listed));
    } catch (error) {
      // A journal this binary cannot read is a data problem, not an
      // environment one — same exit as the server's fail-closed path (§10.2).
      if (error instanceof JournalUnreadableError) fail(65, error.message);
      throw error;
    } finally {
      reader?.close();
    }
  }
} else if (process.argv[2] === "doctor" && process.argv[3] === "config") {
  const usage = "usage: workbench-mcp doctor config [workspace] [--config key=value ...]";
  let root = ".";
  const pairs: string[] = [];
  const rest = process.argv.slice(4);
  for (let i = 0; i < rest.length; i++) {
    const arg = rest[i];
    if (arg === "--config") {
      const next = rest[i + 1];
      if (next === undefined) fail(64, usage);
      pairs.push(next);
      i++;
    } else if (arg?.startsWith("--")) {
      fail(64, usage);
    } else if (arg !== undefined) {
      root = arg;
    }
  }
  const absRoot = resolve(process.cwd(), root);
  if (!existsSync(absRoot)) fail(66, `workspace root does not exist: ${absRoot}`);
  const { flags, errors } = parseSessionFlags(pairs);
  if (errors.length > 0) fail(64, errors.join("; "));
  try {
    const config = resolveConfig({ root: absRoot, sessionFlags: flags });
    console.log(renderDoctorConfig(config, absRoot));
  } catch (error) {
    // A config layer written by a newer binary (§10.2): the data is the
    // problem, not the environment — same exit as the server path.
    if (error instanceof UnsupportedVersionError) fail(65, error.message);
    throw error;
  }
} else if (process.argv[2] === "doctor") {
  // The full doctor plane (SDD §11.2, feature 004-doctor-bundles): read-only
  // probes over the shipped .workbench artifacts — NO lock, no writes, safe
  // beside a live server. `doctor` prints the report; `doctor bundle` packs
  // the same probes into a redacted tar.gz (structure and symptoms, never
  // repo file contents). Exit 0 healthy, 1 findings (contract cli-doctor.md).
  const usage =
    "usage: workbench-mcp doctor [bundle] [workspace] [--out <path>] [--config key=value ...]";
  const isBundle = process.argv[3] === "bundle";
  let root = ".";
  let out: string | undefined;
  const pairs: string[] = [];
  const rest = process.argv.slice(isBundle ? 4 : 3);
  for (let i = 0; i < rest.length; i++) {
    const arg = rest[i];
    if (arg === "--config") {
      const next = rest[i + 1];
      if (next === undefined) fail(64, usage);
      pairs.push(next);
      i++;
    } else if (arg === "--out") {
      const next = rest[i + 1];
      if (next === undefined || !isBundle) fail(64, usage);
      out = next;
      i++;
    } else if (arg?.startsWith("--")) {
      fail(64, usage);
    } else if (arg !== undefined) {
      root = arg;
    }
  }
  const absRoot = resolve(process.cwd(), root);
  if (!existsSync(absRoot)) fail(66, `workspace root does not exist: ${absRoot}`);
  const { flags, errors } = parseSessionFlags(pairs);
  if (errors.length > 0) fail(64, errors.join("; "));
  const run = await runDoctor({
    root: absRoot,
    env: process.env,
    sessionFlags: flags,
    toolVersion: SERVER_VERSION,
    now: () => new Date(),
  });
  if (isBundle) {
    const target = resolve(
      process.cwd(),
      out ?? `workbench-doctor-bundle-${run.report.generatedAt.replaceAll(":", "-")}.tar.gz`,
    );
    // A diagnostic must not destroy a prior diagnostic (contract).
    if (existsSync(target)) fail(64, `bundle target already exists: ${target}`);
    const bundle = buildBundle(run);
    writeFileSync(target, bundle.bytes);
    console.error(`workbench-mcp: bundle written to ${target}`);
    for (const member of bundle.members) console.error(`  ${member}`);
  } else {
    console.log(renderDoctorReport(run));
  }
  process.exit(run.report.findingsPresent ? 1 : 0);
} else {
  const args = process.argv.slice(2);
  let root = ".";
  let http = false;
  let port = 0;
  const configPairs: string[] = [];
  for (let i = 0; i < args.length; i++) {
    const arg = args[i];
    if (arg === "--http") {
      http = true;
      const next = args[i + 1];
      if (next !== undefined && /^\d+$/.test(next)) {
        port = Number(next);
        i++;
      }
    } else if (arg === "--config") {
      const next = args[i + 1];
      if (next === undefined) fail(64, "--config expects key=value");
      configPairs.push(next);
      i++;
    } else if (arg?.startsWith("--")) {
      fail(64, `unknown option ${arg}`);
    } else if (arg !== undefined) {
      root = arg;
    }
  }

  const { flags: sessionFlags, errors: flagErrors } = parseSessionFlags(configPairs);
  if (flagErrors.length > 0) fail(64, flagErrors.join("; "));

  try {
    const workspace = openWorkspace(root, process.env, sessionFlags);
    // The flight recorder (SDD §11.2, feature 005-tracing-ring-buffer): on by
    // default, one JSONL span per served request + engine call, written
    // fire-and-forget under the state dir. `tracing.enabled=false` at any
    // layer wires the no-op tracer instead — no file, no other change.
    const tracer: Tracer =
      workspace.config.effective["tracing.enabled"] === true
        ? new JsonlTracer(join(workspace.stateDir, "trace.jsonl"))
        : noopTracer;
    const composed = composeSearch(workspace, tracer);
    const composedWrite = composeWrite(workspace, composed.engine);
    // §10.2: journal open-time degradations (quarantine, migration) ride
    // every envelope's notes, like the workspace's own.
    const runtime: WorkspaceRuntime = {
      ...workspace,
      notes: [...workspace.notes, ...composedWrite.journal.lifecycleNotes],
    };
    // Tier-2 references (SDD §8.3, chunk:m5-lsp-resolver-plugin): when the
    // detection table has at least one server, the search deps gain the
    // resolver-plugin upgrade lane. No detection ⇒ the resolver is inert and
    // references stay at tree-sitter fidelity — never a requirement (§1.1).
    const resolverSpecs = detectLanguageServers(workspace.root);
    const hostDeps = pluginHostDeps(runtime, composed.plugins);
    // Lang packs (SDD §8.3, chunk:m5-lang-pack): boot every installed pack —
    // carrier read in the child's sandbox lane, registration into the
    // dynamic language registry, tier-selected execution lane. Fire and
    // forget: a workspace with no packs pays nothing, a failing pack is
    // breakered, and until boot settles the pack language is simply lexical.
    void bootLangPacks({
      inventory: hostDeps.inventory,
      readManifest: hostDeps.readManifest,
      componentInfo: hostDeps.componentInfo,
      loadPack: composed.langPackLoad,
      health: hostDeps.health,
      now: hostDeps.now,
    }).catch(() => {});
    // Ranking signals (SDD §8.3, chunk:m5-ranking-shadow): constructed
    // synchronously so the search path holds the seam from the start; boot
    // fills it in the background. Until it settles — and whenever a signal
    // is cold, shadow, or unhealthy — search runs the first-party baseline.
    const promotionsPath = join(workspace.stateDir, "ranking-promotions.json");
    const shadowLogPath = join(workspace.stateDir, "ranking-shadow.jsonl");
    const rankingSignals = new RankingSignals({
      inventory: hostDeps.inventory,
      readManifest: hostDeps.readManifest,
      componentInfo: hostDeps.componentInfo,
      contribute: composed.rankingContribute,
      health: hostDeps.health,
      promotions: {
        read: () => readFile(promotionsPath, "utf8").catch(() => null),
        write: (text) => writeFile(promotionsPath, text, "utf8"),
      },
      shadowLog: {
        size: () =>
          stat(shadowLogPath).then(
            (s) => s.size,
            () => 0,
          ),
        read: () => readFile(shadowLogPath, "utf8").catch(() => ""),
        write: (text) => writeFile(shadowLogPath, text, "utf8"),
        append: (line) => appendFile(shadowLogPath, line, "utf8"),
      },
      now: hostDeps.now,
    });
    void rankingSignals.boot().catch(() => {});
    const search: SearchDeps =
      resolverSpecs.length === 0
        ? { ...composed.search, rankingSignals }
        : {
            ...composed.search,
            rankingSignals,
            resolver: new PluginResolver({
              root: workspace.root,
              specs: resolverSpecs,
              indexTier: async () => (await composed.search.index.status()).tier,
              inventory: hostDeps.inventory,
              readManifest: hostDeps.readManifest,
              componentInfo: hostDeps.componentInfo,
              invokeResolver: composed.resolveGuest,
              health: hostDeps.health,
              now: hostDeps.now,
            }),
          };
    const release = () => {
      composed
        .stop()
        .catch(() => {})
        .finally(() => {
          composedWrite.journal.close();
          workspace.close();
          process.exit(0);
        });
    };
    process.on("SIGINT", release);
    process.on("SIGTERM", release);
    process.on("exit", () => workspace.close());

    if (http) {
      const handle = serveHttp(
        runtime,
        search,
        port,
        composedWrite.write,
        composed.script,
        composed.plugins,
        composeRun(runtime, composed.runnerParse),
        tracer,
      );
      console.error(`workbench-mcp: http listening on port ${handle.port}`);
    } else {
      await serveStdio(
        runtime,
        search,
        composedWrite.write,
        composed.script,
        composed.plugins,
        composeRun(runtime, composed.runnerParse),
        tracer,
      );
      // stdin reached EOF: the client is gone. Release the workspace lock the
      // same way a signal would, so the next session is not refused with 75.
      release();
    }
  } catch (error) {
    if (error instanceof WorkspaceLockedError) fail(75, error.message);
    // Durable data written by a newer binary (SDD §10.2): fail closed rather
    // than guess. EX_DATAERR — the data is the problem, not the environment.
    if (error instanceof UnsupportedVersionError) fail(65, error.message);
    if (error instanceof Error && error.message.startsWith("workspace root does not exist")) {
      fail(66, error.message);
    }
    throw error;
  }
}
