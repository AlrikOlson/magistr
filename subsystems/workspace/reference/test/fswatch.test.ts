/**
 * Watcher-adapter suite (SDD §7.3.3): the polling adapter's snapshot diffing
 * and debounce coalescing are asserted exactly (its cadence is ours to
 * control); the native fs.watch adapter is asserted live (events arrive,
 * settle to the right kind and path) without over-specifying the platform's
 * event chatter. The watcher → index bridge is proven against a recording
 * IndexPort stub. End-to-end freshness lives in freshness-fuzz.test.ts.
 */

import { describe, expect, test } from "bun:test";
import { mkdirSync, mkdtempSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import path from "node:path";
import { bindWatcherToIndex, FsWatchAdapter, PollingWatcherAdapter } from "../src/adapters/fswatch";
import type {
  ImportGraphResult,
  IndexPort,
  IndexStatus,
  OutlineQueryResult,
  ReferenceQueryResult,
  SymbolQueryResult,
  WatchEvent,
  WatcherPort,
  WatchSubscription,
} from "../src/ports";
import { PORT_VERSIONS } from "../src/ports";

const tempDirs: string[] = [];
const subs: WatchSubscription[] = [];

async function cleanup(): Promise<void> {
  for (const sub of subs.splice(0)) {
    await sub.close();
  }
  for (const dir of tempDirs.splice(0)) {
    rmSync(dir, { recursive: true, force: true, maxRetries: 10, retryDelay: 50 });
  }
}

function makeRoot(): string {
  const root = mkdtempSync(path.join(tmpdir(), "wb-watch-"));
  tempDirs.push(root);
  mkdirSync(path.join(root, "src"));
  return root;
}

async function until(predicate: () => boolean, timeoutMs = 5_000): Promise<boolean> {
  const deadline = Date.now() + timeoutMs;
  while (Date.now() < deadline) {
    if (predicate()) return true;
    await Bun.sleep(20);
  }
  return predicate();
}

describe("PollingWatcherAdapter", () => {
  test("detects create, modify and remove; a write burst settles to one event", async () => {
    try {
      const root = makeRoot();
      const events: WatchEvent[] = [];
      const watcher = new PollingWatcherAdapter(30, 40);
      subs.push(await watcher.subscribe([root], (event) => events.push(event)));

      const file = path.join(root, "src", "a.ts");
      writeFileSync(file, "function one() {}\n");
      expect(await until(() => events.some((e) => e.kind === "created"))).toBe(true);
      expect(events.filter((e) => e.path.endsWith("src/a.ts"))).toHaveLength(1);

      // Three writes inside one debounce window: exactly one settled event.
      const before = events.length;
      writeFileSync(file, "function two() {}\n");
      writeFileSync(file, "function three() {}\n");
      writeFileSync(file, "function four() {} // longer\n");
      expect(await until(() => events.length > before)).toBe(true);
      await Bun.sleep(120); // longer than interval+debounce: no second emission
      expect(events.length).toBe(before + 1);
      expect(events[events.length - 1]?.kind).toBe("modified");

      rmSync(file);
      expect(await until(() => events.some((e) => e.kind === "removed"))).toBe(true);
    } finally {
      await cleanup();
    }
  });
});

describe("FsWatchAdapter", () => {
  test("native events arrive and settle to the right kind and path", async () => {
    try {
      const root = makeRoot();
      const events: WatchEvent[] = [];
      const watcher = new FsWatchAdapter(40);
      subs.push(await watcher.subscribe([root], (event) => events.push(event)));
      await Bun.sleep(50); // let the recursive watch attach

      const file = path.join(root, "src", "b.ts");
      writeFileSync(file, "function born() {}\n");
      expect(
        await until(() => events.some((e) => e.path.endsWith("src/b.ts") && e.kind !== "removed")),
      ).toBe(true);

      rmSync(file);
      expect(
        await until(() => events.some((e) => e.path.endsWith("src/b.ts") && e.kind === "removed")),
      ).toBe(true);
    } finally {
      await cleanup();
    }
  });
});

/** Records the bridge's calls; refreshFile can be made to fail per path. */
class RecordingIndex implements IndexPort {
  readonly version = PORT_VERSIONS.index;
  readonly refreshed: string[] = [];
  readonly removed: string[] = [];
  readonly failRefresh = new Set<string>();

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
  refreshFile(filePath: string): Promise<void> {
    if (this.failRefresh.has(filePath)) {
      return Promise.reject(new Error(`no such file: ${filePath}`));
    }
    this.refreshed.push(filePath);
    return Promise.resolve();
  }
  removeFile(filePath: string): Promise<void> {
    this.removed.push(filePath);
    return Promise.resolve();
  }
}

/** A hand-cranked watcher so bridge routing is tested without timers. */
class ManualWatcher implements WatcherPort {
  readonly version = PORT_VERSIONS.watcher;
  private handler: ((event: WatchEvent) => void) | undefined;

  subscribe(
    _paths: readonly string[],
    onEvent: (event: WatchEvent) => void,
  ): Promise<WatchSubscription> {
    this.handler = onEvent;
    return Promise.resolve({ close: () => Promise.resolve() });
  }

  emit(event: WatchEvent): void {
    this.handler?.(event);
  }
}

describe("bindWatcherToIndex", () => {
  test("routes modified→refresh, removed→remove, degrades failed refresh, filters .git", async () => {
    const index = new RecordingIndex();
    const watcher = new ManualWatcher();
    const sub = await bindWatcherToIndex(watcher, index, "/ws/root");

    watcher.emit({ kind: "modified", path: "/ws/root/src/a.ts" });
    watcher.emit({ kind: "created", path: "/ws/root/src/new.ts" });
    watcher.emit({ kind: "removed", path: "/ws/root/src/old.ts" });
    watcher.emit({ kind: "modified", path: "/ws/root/.git/index" });
    watcher.emit({ kind: "modified", path: "/ws/root/.workbench/index.db" });
    index.failRefresh.add("src/gone.ts");
    watcher.emit({ kind: "modified", path: "/ws/root/src/gone.ts" });

    expect(await until(() => index.removed.length === 2)).toBe(true);
    expect(index.refreshed).toEqual(["src/a.ts", "src/new.ts"]);
    // The vanished-mid-refresh file degraded to removal; .git/.workbench never arrive.
    expect(index.removed).toEqual(["src/old.ts", "src/gone.ts"]);
    await sub.close();
  });
});

describe("bindWatcherToIndex is bounded (specs/011 R4)", () => {
  /** An index whose refreshes hold until released, so in-flight counts are observable. */
  class GatedIndex extends RecordingIndex {
    inFlight = 0;
    peak = 0;
    private readonly waiters: (() => void)[] = [];
    override refreshFile(filePath: string): Promise<void> {
      this.inFlight += 1;
      this.peak = Math.max(this.peak, this.inFlight);
      return new Promise<void>((resolve) => {
        this.waiters.push(() => {
          this.inFlight -= 1;
          this.refreshed.push(filePath);
          resolve();
        });
      });
    }
    releaseOne(): boolean {
      const next = this.waiters.shift();
      if (next === undefined) return false;
      next();
      return true;
    }
    releaseAll(): void {
      while (this.releaseOne()) {
        // drain
      }
    }
  }

  test("at most maxInFlight refreshes are in flight; all N complete in arrival order", async () => {
    const index = new GatedIndex();
    const watcher = new ManualWatcher();
    const sub = await bindWatcherToIndex(watcher, index, "/ws/root", { maxInFlight: 3 });
    for (let i = 0; i < 50; i += 1) {
      watcher.emit({ kind: "modified", path: `/ws/root/src/f${i}.ts` });
    }
    expect(await until(() => index.inFlight === 3)).toBe(true);
    expect(sub.backlog()).toBe(50);
    // Release in steps: the ceiling never exceeds 3.
    for (let released = 0; released < 50; released += 1) {
      expect(index.inFlight).toBeLessThanOrEqual(3);
      expect(index.releaseOne()).toBe(true);
      await Bun.sleep(0);
    }
    await sub.idle();
    expect(index.peak).toBe(3);
    expect(index.refreshed).toEqual(Array.from({ length: 50 }, (_, i) => `src/f${i}.ts`));
    expect(sub.backlog()).toBe(0);
    await sub.close();
  });

  test("queued events coalesce per path; a removal supersedes a queued refresh", async () => {
    const index = new GatedIndex();
    const watcher = new ManualWatcher();
    const sub = await bindWatcherToIndex(watcher, index, "/ws/root", { maxInFlight: 1 });
    watcher.emit({ kind: "modified", path: "/ws/root/src/busy.ts" }); // occupies the one slot
    expect(await until(() => index.inFlight === 1)).toBe(true);
    watcher.emit({ kind: "modified", path: "/ws/root/src/a.ts" });
    watcher.emit({ kind: "modified", path: "/ws/root/src/a.ts" });
    watcher.emit({ kind: "created", path: "/ws/root/src/a.ts" });
    watcher.emit({ kind: "modified", path: "/ws/root/src/gone.ts" });
    watcher.emit({ kind: "removed", path: "/ws/root/src/gone.ts" });
    expect(sub.backlog()).toBe(3); // busy (in flight) + a + gone
    while (sub.backlog() > 0) {
      index.releaseAll();
      await Bun.sleep(1);
    }
    await sub.idle();
    // a.ts refreshed once; gone.ts only removed (its queued refresh was superseded).
    expect(index.refreshed).toEqual(["src/busy.ts", "src/a.ts"]);
    expect(index.removed).toEqual(["src/gone.ts"]);
    await sub.close();
  });

  test("the write engine's temp files and an extra ignore filter never reach the index; hooks see outcomes", async () => {
    const index = new RecordingIndex();
    const watcher = new ManualWatcher();
    const events: string[] = [];
    const settled: string[] = [];
    const sub = await bindWatcherToIndex(watcher, index, "/ws/root", {
      ignore: (rel) => rel.endsWith(".log"),
      onEvent: (kind, rel) => events.push(`${kind}:${rel}`),
      onSettled: (rel, outcome) => settled.push(`${outcome}:${rel}`),
    });
    watcher.emit({ kind: "created", path: "/ws/root/src/a.ts.workbench-tmp-1234" });
    watcher.emit({ kind: "modified", path: "/ws/root/out/build.log" });
    watcher.emit({ kind: "modified", path: "/ws/root/src/a.ts" });
    index.failRefresh.add("src/bad.ts");
    watcher.emit({ kind: "modified", path: "/ws/root/src/bad.ts" });
    await sub.idle();
    expect(index.refreshed).toEqual(["src/a.ts"]);
    expect(index.removed).toEqual(["src/bad.ts"]);
    expect(events).toEqual(["modified:src/a.ts", "modified:src/bad.ts"]);
    expect(settled).toEqual(["refreshed:src/a.ts", "failed:src/bad.ts"]);
    await sub.close();
  });

  test("close drops the queue and idle() resolves", async () => {
    const index = new GatedIndex();
    const watcher = new ManualWatcher();
    const sub = await bindWatcherToIndex(watcher, index, "/ws/root", { maxInFlight: 1 });
    watcher.emit({ kind: "modified", path: "/ws/root/src/a.ts" });
    watcher.emit({ kind: "modified", path: "/ws/root/src/b.ts" });
    expect(await until(() => index.inFlight === 1)).toBe(true);
    await sub.close();
    index.releaseAll();
    await sub.idle();
    expect(index.refreshed).toEqual(["src/a.ts"]); // b.ts was dropped with the queue
    watcher.emit({ kind: "modified", path: "/ws/root/src/c.ts" });
    await Bun.sleep(10);
    expect(index.refreshed).toEqual(["src/a.ts"]);
  });
});
