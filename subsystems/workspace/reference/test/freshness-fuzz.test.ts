/**
 * The freshness fuzzer (SDD §7.3.3, chunk m1-watcher-freshness acceptance):
 * a concurrent mutator races a query loop against the real engine child
 * process, and any stale line number fails the run. The oracle is built into
 * the content — version v of file i declares exactly one symbol,
 * `sym_<i>_v<v>`, always at line 1 + (v % 9), with version-varying size —
 * so every returned row is checkable from the row alone:
 *
 *  - line oracle: a row whose start_line ≠ 1 + (v % 9) is a stale or
 *    fabricated line number, full stop;
 *  - staleness oracle: a `warm` answer returning version v older than what
 *    was durably on disk before the query was issued proves the mtime+size
 *    verification missed a change.
 *
 * The same fuzz body runs once per watcher adapter — native fs.watch and
 * the polling fallback — which is acceptance criterion three. The mutator
 * lives in this (server-side) process; the index and its verification live
 * in the spawned engine child, so the race crosses a real process boundary.
 */

import { afterEach, describe, expect, test } from "bun:test";
import { mkdirSync, mkdtempSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import path from "node:path";
import { EngineSupervisor, IpcIndexAdapter } from "../src/adapters/engine-ipc";
import { bindWatcherToIndex, FsWatchAdapter, PollingWatcherAdapter } from "../src/adapters/fswatch";
import type { WatcherPort } from "../src/ports";

const FILE_COUNT = 5;
const FUZZ_MS = 1_200;
const MUTATE_EVERY_MS = 12;
const SYMBOL = /^sym_(\d+)_v(\d+)$/;

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

function relPath(i: number): string {
  return `src/f${i}.ts`;
}

/** Version v of file i: one symbol at a line and size that both encode v. */
function contentFor(i: number, v: number): string {
  const line = 1 + (v % 9);
  const lines: string[] = [];
  for (let n = 1; n < line; n += 1) lines.push("// pad");
  lines.push(`function sym_${i}_v${v}() {}`);
  lines.push(`// tail ${"x".repeat(v % 23)}`);
  return `${lines.join("\n")}\n`;
}

function expectedLine(v: number): number {
  return 1 + (v % 9);
}

function makeWorkspace(): string {
  const root = mkdtempSync(path.join(tmpdir(), "wb-fuzz-"));
  tempDirs.push(root);
  mkdirSync(path.join(root, "src"));
  for (let i = 0; i < FILE_COUNT; i += 1) {
    writeFileSync(path.join(root, relPath(i)), contentFor(i, 0));
  }
  return root;
}

interface FuzzShape {
  /** Where the mutator runs: this process, or a spawned one (specs/011 R7). */
  readonly mutator: "in-process" | "spawned";
  /** After the fuzz loop, rewrite every file at once (a checkout-style burst). */
  readonly burst: boolean;
}

/**
 * The spawned mutator: a child bun process that rewrites file `version %
 * FILE_COUNT` every MUTATE_EVERY_MS for `durationMs`, printing `i v` after
 * each write RETURNS — so a line read by the parent is a true lower bound
 * on what is durably on disk, the same contract the in-process mutator's
 * `durable[]` keeps. It stops by itself; the parent never kills it, so the
 * final on-disk versions are exactly the reported ones.
 */
function spawnMutator(root: string, durationMs: number): Bun.Subprocess<"ignore", "pipe", "pipe"> {
  const script = `
    const { writeFileSync } = require("node:fs");
    const path = require("node:path");
    const { FUZZ_ROOT: root, FUZZ_DURATION: durationMs, FUZZ_FILES: fileCount, FUZZ_EVERY: everyMs } = process.env;
    const contentFor = (i, v) => {
      const line = 1 + (v % 9);
      const lines = [];
      for (let n = 1; n < line; n += 1) lines.push("// pad");
      lines.push("function sym_" + i + "_v" + v + "() {}");
      lines.push("// tail " + "x".repeat(v % 23));
      return lines.join("\\n") + "\\n";
    };
    const deadline = Date.now() + Number(durationMs);
    let version = 0;
    const tick = () => {
      if (Date.now() >= deadline) { process.exit(0); return; }
      version += 1;
      const i = version % Number(fileCount);
      writeFileSync(path.join(root, "src", "f" + i + ".ts"), contentFor(i, version));
      process.stdout.write(i + " " + version + "\\n");
      setTimeout(tick, Number(everyMs));
    };
    tick();
  `;
  // Arguments ride the environment: `bun -e` does not place them in argv portably.
  return Bun.spawn([process.execPath, "-e", script], {
    stdin: "ignore",
    stdout: "pipe",
    stderr: "pipe",
    env: {
      ...process.env,
      FUZZ_ROOT: root,
      FUZZ_DURATION: String(durationMs),
      FUZZ_FILES: String(FILE_COUNT),
      FUZZ_EVERY: String(MUTATE_EVERY_MS),
    },
  });
}

async function runFuzz(
  watcher: WatcherPort,
  shape: FuzzShape = { mutator: "in-process", burst: false },
): Promise<void> {
  const root = makeWorkspace();
  const supervisor = new EngineSupervisor({
    env: { WORKBENCH_ENGINE_STUB_CORE: "1" },
    backoffMs: [0, 10, 20],
    readyTimeoutMs: 10_000,
  });
  closers.push(() => supervisor.stop());
  const index = new IpcIndexAdapter(supervisor, root);

  await index.build(root);
  const deadline = Date.now() + 10_000;
  while (Date.now() < deadline) {
    const status = await index.status();
    if (status.tier === 1 && status.building === false) break;
    await Bun.sleep(25);
  }

  const sub = await bindWatcherToIndex(watcher, index, root);
  closers.push(() => sub.close());

  // durable[i] = the newest version whose write has returned. writeFileSync
  // completes atomically w.r.t. this event loop, so a snapshot taken just
  // before a query is a true lower bound on what the child can observe.
  const durable: number[] = Array.from({ length: FILE_COUNT }, () => 0);
  let mutating = true;
  let version = 0;
  let mutator: Promise<void>;
  if (shape.mutator === "in-process") {
    mutator = (async () => {
      while (mutating) {
        version += 1;
        const i = version % FILE_COUNT;
        writeFileSync(path.join(root, relPath(i)), contentFor(i, version));
        durable[i] = version;
        await Bun.sleep(MUTATE_EVERY_MS);
      }
    })();
  } else {
    // Another process writes; this one learns each durable version from its
    // stdout line, keeping the same lower-bound contract.
    const child = spawnMutator(root, FUZZ_MS);
    mutator = (async () => {
      const reader = child.stdout.getReader();
      const decoder = new TextDecoder();
      let buffered = "";
      for (;;) {
        const { value, done } = await reader.read();
        if (done) break;
        buffered += decoder.decode(value, { stream: true });
        let nl = buffered.indexOf("\n");
        while (nl >= 0) {
          const [i, v] = buffered.slice(0, nl).split(" ").map(Number);
          buffered = buffered.slice(nl + 1);
          if (i !== undefined && v !== undefined && !Number.isNaN(i) && !Number.isNaN(v)) {
            durable[i] = v;
            version = v;
          }
          nl = buffered.indexOf("\n");
        }
      }
      const code = await child.exited;
      if (code !== 0)
        throw new Error(
          `spawned mutator exited ${code}: ${await new Response(child.stderr).text()}`,
        );
    })();
  }

  let queries = 0;
  const seen = new Set<string>();
  const until = Date.now() + FUZZ_MS;
  while (Date.now() < until) {
    const floor = [...durable];
    const result = await index.querySymbols({ name: "sym_" });
    queries += 1;
    expect(["cold", "warming", "warm"]).toContain(result.freshness);
    seen.add(result.freshness);
    for (const symbol of result.symbols) {
      const match = SYMBOL.exec(symbol.name);
      expect(match).not.toBeNull();
      if (match === null) continue;
      const i = Number(match[1]);
      const v = Number(match[2]);
      // Line oracle: any honestly parsed version carries this line.
      expect(symbol.location.line).toBe(expectedLine(v));
      // Staleness oracle: warm answers may not predate the pre-query disk.
      if (result.freshness === "warm") {
        expect(v).toBeGreaterThanOrEqual(floor[i] ?? 0);
      }
    }
  }
  mutating = false;
  await mutator;

  if (shape.burst) {
    // A checkout: every file rewritten in one synchronous sweep. On Windows
    // this is exactly the shape that overflows fs.watch's buffer (null
    // filename), which the native adapter now answers with a snapshot diff.
    for (let i = 0; i < FILE_COUNT; i += 1) {
      version += 1;
      writeFileSync(path.join(root, relPath(i)), contentFor(i, version));
      durable[i] = version;
    }
  }

  // Settle (watcher flush + any in-flight refresh), then the closing exam:
  // a warm answer whose every row is the final on-disk version.
  await Bun.sleep(400);
  const finalFloor = [...durable];
  const final = await index.querySymbols({ name: "sym_" });
  expect(final.freshness).toBe("warm");
  expect(final.symbols).toHaveLength(FILE_COUNT);
  for (const symbol of final.symbols) {
    const match = SYMBOL.exec(symbol.name);
    expect(match).not.toBeNull();
    if (match === null) continue;
    expect(Number(match[2])).toBe(finalFloor[Number(match[1])] ?? 0);
    expect(symbol.location.line).toBe(expectedLine(Number(match[2])));
  }
  expect(queries).toBeGreaterThan(5);
}

describe("freshness fuzzer (concurrent mutator vs engine child)", () => {
  test("native fs.watch adapter: no stale line number ever leaves the server", async () => {
    await runFuzz(new FsWatchAdapter());
  }, 30_000);

  test("polling fallback adapter passes the same fuzzer", async () => {
    await runFuzz(new PollingWatcherAdapter(100));
  }, 30_000);

  // specs/011 (chunk:rust-watcher-wiring): the mutator in ANOTHER PROCESS —
  // the shape of cargo fmt, git and IDE saves — plus a checkout-style burst.
  test("native fs.watch adapter: spawned-process mutator + checkout burst", async () => {
    await runFuzz(new FsWatchAdapter(), { mutator: "spawned", burst: true });
  }, 40_000);

  test("polling fallback adapter: spawned-process mutator + checkout burst", async () => {
    await runFuzz(new PollingWatcherAdapter(100), { mutator: "spawned", burst: true });
  }, 40_000);
});
