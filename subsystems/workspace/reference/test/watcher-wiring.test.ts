/**
 * Watcher wiring (specs/011-rust-watcher-wiring) — chunk:rust-watcher-wiring:
 *
 *   SC-006 mode selection: auto+local ⇒ fs.watch, auto+network ⇒ polling,
 *          forced modes honored, off ⇒ nothing;
 *   SC-004 the status record: written at start, stopped at close, absent when
 *          off; doctor's view: live / last-session / not-active / unreadable;
 *   SC-001 lag: a NEW file written by ANOTHER PROCESS becomes addressable by
 *          `read symbol:` and tagged in search within a recorded lag, for both
 *          adapters; deletion removes it; the off control never sees it;
 *   SC-002 burst: 2000 external files index with zero ENGINE_BUSY, settle
 *          time recorded;
 *   SC-003 idle CPU recorded for none / fs.watch / polling over the same tree.
 *
 * The real path is the fuzzer's rig: a spawned engine child (stub core) +
 * IpcIndexAdapter + the real read/search tools. Numbers print as
 * `[011 SC-00x] …` lines and are recorded in the ship trace.
 */

import { afterEach, describe, expect, test } from "bun:test";
import {
  existsSync,
  mkdirSync,
  mkdtempSync,
  readFileSync,
  rmSync,
  statSync,
  unlinkSync,
  writeFileSync,
} from "node:fs";
import { readFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import path from "node:path";
import {
  EngineSupervisor,
  fallbackScanWorkspace,
  IpcEngineAdapter,
  IpcIndexAdapter,
} from "../src/adapters/engine-ipc";
import {
  FsWatchAdapter,
  PollingWatcherAdapter,
  type RunningWatcher,
  readWatcherStatus,
  selectWatcherMode,
  startWatcher,
  WATCHER_STATUS_FILE,
} from "../src/adapters/fswatch";
import type {
  ImportGraphResult,
  IndexPort,
  IndexStatus,
  OutlineQueryResult,
  ReferenceQueryResult,
  SymbolQueryResult,
} from "../src/ports";
import { PORT_VERSIONS } from "../src/ports";
import { type ReadDeps, runRead } from "../src/tools/read";
import { runSearch, type SearchDeps } from "../src/tools/search";

const closers: (() => unknown)[] = [];
const tempDirs: string[] = [];

afterEach(async () => {
  for (const close of closers.splice(0).reverse()) {
    await close();
  }
  for (const dir of tempDirs.splice(0)) {
    rmSync(dir, { recursive: true, force: true, maxRetries: 10, retryDelay: 50 });
  }
});

function makeRoot(prefix = "wb-wiring-"): string {
  const root = mkdtempSync(path.join(tmpdir(), prefix));
  tempDirs.push(root);
  mkdirSync(path.join(root, "src"));
  return root;
}

async function until(
  predicate: () => boolean | Promise<boolean>,
  timeoutMs = 10_000,
): Promise<boolean> {
  const deadline = Date.now() + timeoutMs;
  while (Date.now() < deadline) {
    if (await predicate()) return true;
    await Bun.sleep(15);
  }
  return await predicate();
}

/** Write a file from ANOTHER process — the shape of cargo fmt / git / an IDE. */
function externalWrite(absPath: string, content: string): void {
  const script = `require("node:fs").writeFileSync(${JSON.stringify(absPath)}, ${JSON.stringify(content)});`;
  const proc = Bun.spawnSync([process.execPath, "-e", script], { stdout: "pipe", stderr: "pipe" });
  if (proc.exitCode !== 0) throw new Error(`external write failed: ${proc.stderr.toString()}`);
}

// ---------------------------------------------------------------------------
// Mode selection (SC-006)

describe("selectWatcherMode (R2)", () => {
  test("auto follows the network-fs detection; forced modes are honored; off is nothing", () => {
    expect(selectWatcherMode("auto", false)).toEqual({ mode: "auto", adapter: "fs.watch" });
    expect(selectWatcherMode("auto", true)).toEqual({ mode: "auto", adapter: "polling" });
    expect(selectWatcherMode("native", true)).toEqual({ mode: "native", adapter: "fs.watch" });
    expect(selectWatcherMode("poll", false)).toEqual({ mode: "poll", adapter: "polling" });
    expect(selectWatcherMode("off", false)).toEqual({ mode: "off", adapter: null });
    // Unknown / absent settings resolve like the default.
    expect(selectWatcherMode(undefined, false)).toEqual({ mode: "auto", adapter: "fs.watch" });
    expect(selectWatcherMode("sideways", true)).toEqual({ mode: "auto", adapter: "polling" });
  });
});

// ---------------------------------------------------------------------------
// Status record + doctor's view (SC-004)

class NullIndex implements IndexPort {
  readonly version = PORT_VERSIONS.index;
  readonly refreshed: string[] = [];
  readonly removed: string[] = [];
  status(): Promise<IndexStatus> {
    return Promise.resolve({ tier: 1, generation: 0 });
  }
  build(): Promise<IndexStatus> {
    return this.status();
  }
  querySymbols(): Promise<SymbolQueryResult> {
    return Promise.resolve({ symbols: [], freshness: "warm" });
  }
  queryReferences(): Promise<ReferenceQueryResult> {
    return Promise.resolve({ edges: [], freshness: "warm" });
  }
  queryOutline(): Promise<OutlineQueryResult> {
    return Promise.resolve({ files: [], freshness: "warm" });
  }
  queryImportGraph(): Promise<ImportGraphResult> {
    return Promise.resolve({ fanIn: [], fanOut: [], freshness: "warm" });
  }
  upsertFile(): Promise<void> {
    return Promise.resolve();
  }
  refreshFile(p: string): Promise<void> {
    this.refreshed.push(p);
    return Promise.resolve();
  }
  removeFile(p: string): Promise<void> {
    this.removed.push(p);
    return Promise.resolve();
  }
}

describe("startWatcher + the status record (R3)", () => {
  test("off wires nothing and writes no record", async () => {
    const root = makeRoot();
    const stateDir = path.join(root, ".workbench");
    const running = await startWatcher({
      root,
      stateDir,
      index: new NullIndex(),
      mode: "off",
      networkFs: false,
    });
    expect(running).toBeNull();
    expect(existsSync(path.join(stateDir, WATCHER_STATUS_FILE))).toBe(false);
    expect(readWatcherStatus(stateDir)).toMatchObject({ state: "not-active" });
  });

  test("native: record at start (live), counters after events, stopped at close (last session)", async () => {
    const root = makeRoot();
    const stateDir = path.join(root, ".workbench");
    const index = new NullIndex();
    let t = 1_000_000;
    const running = await startWatcher({
      root,
      stateDir,
      index,
      mode: "auto",
      networkFs: false,
      now: () => t,
      pid: 4242,
    });
    if (running === null) throw new Error("expected a running watcher");
    closers.push(() => running.close());
    expect(running.adapter).toBe("fs.watch");
    const file = path.join(stateDir, WATCHER_STATUS_FILE);
    const first = JSON.parse(readFileSync(file, "utf8"));
    expect(first).toMatchObject({
      v: 1,
      pid: 4242,
      mode: "auto",
      adapter: "fs.watch",
      root,
      events: 0,
      startedAtMs: 1_000_000,
    });
    expect(readWatcherStatus(stateDir, () => true)).toMatchObject({ state: "live" });
    expect(readWatcherStatus(stateDir, () => false)).toMatchObject({ state: "last-session" });

    // An external write flows through the bridge and into the counters.
    t += 2_000;
    externalWrite(path.join(root, "src/new.ts"), "export const n = 1;\n");
    expect(await until(() => index.refreshed.includes("src/new.ts"))).toBe(true);
    await running.idle();
    const snap = running.snapshot();
    expect(snap.events).toBeGreaterThanOrEqual(1);
    expect(snap.refreshes).toBeGreaterThanOrEqual(1);
    expect(snap.lastEventAtMs).toBe(1_002_000);
    // The throttled write lands (≤ 1 s after the event).
    expect(await until(() => JSON.parse(readFileSync(file, "utf8")).events >= 1, 3_000)).toBe(true);

    t += 5_000;
    await running.close();
    const last = JSON.parse(readFileSync(file, "utf8"));
    expect(last.stoppedAtMs).toBe(1_007_000);
    expect(readWatcherStatus(stateDir, () => true)).toMatchObject({ state: "last-session" });
  });

  test("doctor's view: unreadable is named, not thrown; a dead pid is last-session", () => {
    const root = makeRoot();
    const stateDir = path.join(root, ".workbench");
    mkdirSync(stateDir, { recursive: true });
    writeFileSync(path.join(stateDir, WATCHER_STATUS_FILE), "{not json");
    expect(readWatcherStatus(stateDir)).toMatchObject({
      state: "unreadable",
      file: ".workbench/watcher.json",
    });
    writeFileSync(
      path.join(stateDir, WATCHER_STATUS_FILE),
      JSON.stringify({ v: 1, pid: 7, mode: "weird", adapter: "fs.watch" }),
    );
    expect(readWatcherStatus(stateDir)).toMatchObject({ state: "unreadable" });
    writeFileSync(
      path.join(stateDir, WATCHER_STATUS_FILE),
      JSON.stringify({
        v: 1,
        pid: 999_999_9,
        mode: "auto",
        adapter: "polling",
        root,
        startedAtMs: 1,
        updatedAtMs: 2,
        events: 3,
        refreshes: 2,
        removals: 0,
        failures: 1,
      }),
    );
    expect(readWatcherStatus(stateDir)).toMatchObject({
      state: "last-session",
      record: { adapter: "polling", events: 3 },
    });
  });
});

// ---------------------------------------------------------------------------
// The real path: engine child + IpcIndexAdapter + read/search tools

interface Rig {
  readonly root: string;
  readonly stateDir: string;
  readonly index: IpcIndexAdapter;
  readonly search: SearchDeps;
  readonly read: ReadDeps;
  readonly busy: { count: number };
}

async function makeRig(files: Record<string, string>): Promise<Rig> {
  const root = makeRoot("wb-wiring-rig-");
  for (const [rel, content] of Object.entries(files)) {
    mkdirSync(path.dirname(path.join(root, rel)), { recursive: true });
    writeFileSync(path.join(root, rel), content);
  }
  const supervisor = new EngineSupervisor({
    env: { WORKBENCH_ENGINE_STUB_CORE: "1" },
    backoffMs: [0, 10, 20],
    readyTimeoutMs: 10_000,
  });
  closers.push(() => supervisor.stop());
  const index = new IpcIndexAdapter(supervisor, root);
  await index.build(root);
  expect(
    await until(async () => {
      const s = await index.status();
      return s.tier === 1 && s.building === false;
    }),
  ).toBe(true);
  const list = () => fallbackScanWorkspace(root).map((e) => ({ path: e.path, junk: e.junk }));
  const read = (p: string) => readFile(path.join(root, p), "utf8");
  const engine = new IpcEngineAdapter(supervisor, {
    read,
    list: () => Promise.resolve(list().map((f) => f.path)),
  });
  return {
    root,
    stateDir: path.join(root, ".workbench"),
    index,
    search: { engine, index, workspace: { list: () => Promise.resolve(list()), read } },
    read: {
      index,
      workspace: {
        read,
        stat: (p) => {
          const s = statSync(path.join(root, p));
          return Promise.resolve({ mtimeMs: s.mtimeMs, size: s.size });
        },
      },
    },
    busy: { count: 0 },
  };
}

async function start(rig: Rig, mode: "native" | "poll" | "off"): Promise<RunningWatcher | null> {
  const running = await startWatcher({
    root: rig.root,
    stateDir: rig.stateDir,
    index: rig.index,
    mode,
    networkFs: false,
    onSettled: (_rel, outcome, error) => {
      if (outcome === "failed" && (error as { code?: unknown })?.code === "ENGINE_BUSY") {
        rig.busy.count += 1;
      }
    },
  });
  if (running !== null) closers.push(() => running.close());
  return running;
}

const BASE = {
  "src/a.ts": "export function alpha() {\n  return 1;\n}\n",
  "src/b.ts": "export function beta() {\n  return 2;\n}\n",
};

/** The watcher's observable: the index KNOWS the file (querySymbols by name). */
async function indexKnows(rig: Rig, file: string, symbol: string): Promise<boolean> {
  const r = await rig.index.querySymbols({ name: symbol });
  return r.symbols.some((s) => s.name === symbol && s.location.path === file);
}

/** `read symbol:` over the same file — must work once the index knows it. */
async function readResolves(rig: Rig, file: string, symbol: string): Promise<boolean> {
  const envelope = await runRead(rig.read, { file, symbol }, 1);
  return envelope.ok;
}

async function searchTagged(rig: Rig, file: string, symbol: string): Promise<boolean> {
  const envelope = await runSearch(rig.search, { query: symbol, mode: "lexical", context: 0 }, 1);
  if (!envelope.ok) return false;
  const group = envelope.results.find((g) => g.file === file);
  return group?.hits.some((h) => h.kind !== undefined) === true;
}

describe("external edits reach the index (SC-001)", () => {
  for (const mode of ["native", "poll"] as const) {
    test(`${mode}: a NEW file from another process is addressable by read symbol: and tagged in search; deletion removes it`, async () => {
      const rig = await makeRig(BASE);
      const running = await start(rig, mode);
      if (running === null) throw new Error("expected a running watcher");
      // Let the subscription settle before the external write.
      await Bun.sleep(mode === "poll" ? 300 : 100);
      const rel = `src/fresh_${mode}.ts`;
      const symbol = `fresh_${mode}_fn`;
      const t0 = performance.now();
      externalWrite(path.join(rig.root, rel), `export function ${symbol}() {\n  return 3;\n}\n`);
      expect(await until(() => indexKnows(rig, rel, symbol), 15_000)).toBe(true);
      const lagMs = Math.round(performance.now() - t0);
      console.log(
        `[011 SC-001] ${running.adapter}: external new file → symbol in the index in ${lagMs}ms`,
      );
      // Both tool paths answer from it: read's symbol address and search's kind tag.
      expect(await readResolves(rig, rel, symbol)).toBe(true);
      expect(await searchTagged(rig, rel, symbol)).toBe(true);
      // Removal flows too.
      unlinkSync(path.join(rig.root, rel));
      expect(
        await until(async () => {
          const r = await rig.index.querySymbols({ name: symbol });
          return r.symbols.length === 0;
        }, 15_000),
      ).toBe(true);
      expect(rig.busy.count).toBe(0);
    }, 40_000);
  }

  test("off control: without the watcher a new file stays unknown to the index (and untagged in search) until a rebuild", async () => {
    const rig = await makeRig(BASE);
    expect(await start(rig, "off")).toBeNull();
    externalWrite(
      path.join(rig.root, "src/ghost.ts"),
      "export function ghost_fn() {\n  return 4;\n}\n",
    );
    await Bun.sleep(1_200);
    expect(await indexKnows(rig, "src/ghost.ts", "ghost_fn")).toBe(false);
    expect(await searchTagged(rig, "src/ghost.ts", "ghost_fn")).toBe(false);
    // Honest record: `read symbol:` self-heals a new file by parsing it on
    // demand, so it is NOT the watcher's observable — the index, search tags,
    // outlines and references are.
    expect(await readResolves(rig, "src/ghost.ts", "ghost_fn")).toBe(true);
    await rig.index.build(rig.root);
    expect(await until(() => indexKnows(rig, "src/ghost.ts", "ghost_fn"), 15_000)).toBe(true);
  }, 30_000);
});

// ---------------------------------------------------------------------------
// Bursts and idle (SC-002, SC-003)

const BURST = 2000;

function burstTree(): Record<string, string> {
  const files: Record<string, string> = {};
  for (let i = 0; i < BURST; i += 1) {
    files[`src/part${i % 20}/burst_${i}.ts`] =
      `export function burst_${i}_v0() {\n  return ${i};\n}\n`;
  }
  return files;
}

describe("bursts are bounded and idle is free (SC-002, SC-003)", () => {
  test("a 2000-file external burst indexes every file with zero ENGINE_BUSY; settle time recorded", async () => {
    const rig = await makeRig(burstTree());
    const running = await start(rig, "native");
    if (running === null) throw new Error("expected a running watcher");
    await Bun.sleep(200);
    const t0 = performance.now();
    // The checkout: every file rewritten at once, version 0 → 1.
    for (let i = 0; i < BURST; i += 1) {
      writeFileSync(
        path.join(rig.root, `src/part${i % 20}/burst_${i}.ts`),
        `export function burst_${i}_v1() {\n  return ${i};\n}\n`,
      );
    }
    // Settle = every event seen and every refresh landed.
    expect(await until(() => running.snapshot().refreshes >= BURST, 60_000)).toBe(true);
    await running.idle();
    const settleMs = Math.round(performance.now() - t0);
    const snap = running.snapshot();
    console.log(
      `[011 SC-002] burst ${BURST} files: events=${snap.events} refreshes=${snap.refreshes} ` +
        `failures=${snap.failures} ENGINE_BUSY=${rig.busy.count} settle=${settleMs}ms`,
    );
    expect(rig.busy.count).toBe(0);
    expect(snap.failures).toBe(0);
    // Every file carries its v1 symbol now — sampled across the tree, then exhaustively by count.
    for (const i of [0, 1, 999, 1500, BURST - 1]) {
      expect(await indexKnows(rig, `src/part${i % 20}/burst_${i}.ts`, `burst_${i}_v1`)).toBe(true);
    }
    // Exhaustively: every tenth file by exact symbol name (query results are capped, so
    // a single wildcard count cannot prove 2000).
    for (let i = 0; i < BURST; i += 10) {
      const r = await rig.index.querySymbols({ name: `burst_${i}_v1` });
      expect(r.symbols.some((s) => s.name === `burst_${i}_v1`)).toBe(true);
    }
  }, 120_000);

  test("idle CPU over a 2000-file tree: none vs fs.watch vs polling (recorded)", async () => {
    const root = makeRoot("wb-wiring-idle-");
    for (const [rel, content] of Object.entries(burstTree())) {
      mkdirSync(path.dirname(path.join(root, rel)), { recursive: true });
      writeFileSync(path.join(root, rel), content);
    }
    const index = new NullIndex();
    const WINDOW_MS = 1_500;
    const measure = async (): Promise<number> => {
      await Bun.sleep(150); // let any subscription settle first
      const before = process.cpuUsage();
      await Bun.sleep(WINDOW_MS);
      const delta = process.cpuUsage(before);
      return Math.round((delta.user + delta.system) / 1_000); // ms of CPU
    };
    const none = await measure();
    const native = await new FsWatchAdapter().subscribe([root], () => {});
    const withNative = await measure();
    await native.close();
    const polling = await new PollingWatcherAdapter().subscribe([root], () => {});
    const withPolling = await measure();
    await polling.close();
    void index;
    console.log(
      `[011 SC-003] idle CPU over ${WINDOW_MS}ms, ${BURST} files: none=${none}ms fs.watch=${withNative}ms polling=${withPolling}ms`,
    );
    // The bar (Constitution I): native is indistinguishable from no watcher
    // within a generous tolerance; the polling number is recorded, not bounded.
    expect(withNative).toBeLessThanOrEqual(none + 25);
  }, 60_000);
});
