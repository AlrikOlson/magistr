/**
 * Filesystem watcher adapters (SDD §7.3.3) — the two production WatcherPort
 * implementations. `FsWatchAdapter` rides the platform's native facility via
 * `fs.watch(recursive)`; `PollingWatcherAdapter` is the fallback for
 * filesystems where native watching lies too much to be useful (network
 * mounts), diffing periodic snapshots instead. Both coalesce bursts behind
 * the port's documented debounce: subscribers see one event per settled
 * path, `WATCH_DEBOUNCE_MS` after the burst ends.
 *
 * Neither adapter is trusted for correctness — every query answer is
 * additionally verified against mtime+size before return (ChildIndexer).
 * A watcher only shrinks the window in which that verification has to do
 * synchronous re-parse work.
 */

import { existsSync, watch } from "node:fs";
import { relative } from "node:path";
import type {
  IndexPort,
  WatchEvent,
  WatchEventKind,
  WatcherPort,
  WatchSubscription,
} from "../../ports";
import { PORT_VERSIONS } from "../../ports";
import { fallbackScanWorkspace } from "../engine-ipc";

/** SDD §7.3.3: watcher events debounce at 75ms into incremental re-parses. */
export const WATCH_DEBOUNCE_MS = 75;

/** Default snapshot cadence of the polling fallback. */
export const POLL_INTERVAL_MS = 200;

function toPosix(path: string): string {
  return path.replaceAll("\\", "/");
}

/**
 * Per-path event coalescing: a burst of raw events becomes one WatchEvent
 * per path, emitted `debounceMs` after the path last changed. The kind is
 * re-derived from existence at flush time, so a create+delete burst settles
 * as `removed` and a delete+recreate as `modified`.
 */
class Debouncer {
  private readonly pending = new Map<
    string,
    { timer: ReturnType<typeof setTimeout>; sawCreate: boolean }
  >();
  private closed = false;

  constructor(
    private readonly debounceMs: number,
    private readonly emit: (event: WatchEvent) => void,
  ) {}

  push(path: string, exists: () => boolean, sawCreate: boolean): void {
    if (this.closed) return;
    const prior = this.pending.get(path);
    if (prior !== undefined) clearTimeout(prior.timer);
    // A create hint sticks for the burst: create+modify still settles created.
    const created = sawCreate || (prior?.sawCreate ?? false);
    this.pending.set(path, {
      sawCreate: created,
      timer: setTimeout(() => {
        this.pending.delete(path);
        if (this.closed) return;
        const kind: WatchEventKind = exists() ? (created ? "created" : "modified") : "removed";
        this.emit({ kind, path });
      }, this.debounceMs),
    });
  }

  close(): void {
    this.closed = true;
    for (const entry of this.pending.values()) clearTimeout(entry.timer);
    this.pending.clear();
  }
}

interface Snapshot {
  readonly size: number;
  readonly mtimeMs: number;
}

/**
 * Snapshot differ shared by both adapters: the same walk the degraded
 * indexer uses, diffed against the last snapshot, pushed into a debouncer
 * as created/modified/removed hints. The polling adapter runs it on a
 * cadence; the native adapter runs it when the platform says its event
 * buffer overflowed (specs/011, deviation: on Windows, libuv reports an
 * overflow as a change event with a NULL filename — "something under the
 * root changed, rescan" — and a 200-file synchronous burst is enough to
 * trigger it, so ignoring null would silently drop most of a `git checkout`).
 */
class SnapshotDiffer {
  private known = new Map<string, Snapshot>();
  private primed = false;

  constructor(
    private readonly root: string,
    private readonly debouncer: Debouncer,
  ) {}

  /** Take the first snapshot without emitting anything. */
  prime(): void {
    this.known = this.take() ?? this.known;
    this.primed = true;
  }

  /** Walk, diff against the last snapshot, emit hints, remember the new one. */
  rescan(): void {
    const next = this.take();
    if (next === null) return; // The root itself vanished mid-walk; keep the last snapshot.
    const base = toPosix(this.root);
    if (this.primed) {
      for (const [rel, snap] of next) {
        const prior = this.known.get(rel);
        if (prior !== undefined && prior.size === snap.size && prior.mtimeMs === snap.mtimeMs) {
          continue;
        }
        const full = `${base}/${rel}`;
        this.debouncer.push(full, () => existsSync(full), prior === undefined);
      }
      for (const rel of this.known.keys()) {
        if (next.has(rel)) continue;
        const full = `${base}/${rel}`;
        this.debouncer.push(full, () => existsSync(full), false);
      }
    }
    this.known = next;
    this.primed = true;
  }

  private take(): Map<string, Snapshot> | null {
    let entries: ReturnType<typeof fallbackScanWorkspace>;
    try {
      entries = fallbackScanWorkspace(this.root);
    } catch {
      return null;
    }
    const next = new Map<string, Snapshot>();
    for (const entry of entries) next.set(entry.path, { size: entry.size, mtimeMs: entry.mtimeMs });
    return next;
  }
}

/**
 * Native watcher: one recursive `fs.watch` per subscribed root, plus a
 * snapshot taken at subscribe so an overflow signal (null filename) can be
 * answered with a diff instead of silence.
 */
export class FsWatchAdapter implements WatcherPort {
  readonly version = PORT_VERSIONS.watcher;
  /** Overflow signals seen (tests and the status record). */
  rescans = 0;

  constructor(private readonly debounceMs: number = WATCH_DEBOUNCE_MS) {}

  subscribe(
    paths: readonly string[],
    onEvent: (event: WatchEvent) => void,
  ): Promise<WatchSubscription> {
    const debouncer = new Debouncer(this.debounceMs, onEvent);
    const watchers = paths.map((root) => {
      const differ = new SnapshotDiffer(root, debouncer);
      differ.prime();
      return watch(root, { recursive: true }, (eventType, filename) => {
        if (filename === null) {
          // The platform's event buffer overflowed: every path under the
          // root may have changed. Diff the tree instead of guessing.
          this.rescans += 1;
          differ.rescan();
          return;
        }
        const rel = toPosix(filename);
        const full = `${toPosix(root)}/${rel}`;
        // fs.watch cannot distinguish create from modify portably; existence
        // at flush time decides, with `rename` as the create hint.
        debouncer.push(full, () => existsSync(full), eventType === "rename" && existsSync(full));
      });
    });
    return Promise.resolve({
      close: () => {
        for (const watcher of watchers) watcher.close();
        debouncer.close();
        return Promise.resolve();
      },
    });
  }
}

/**
 * Polling fallback: diffs periodic workspace snapshots (the same walk the
 * degraded indexer uses) and synthesizes created/modified/removed events.
 */
export class PollingWatcherAdapter implements WatcherPort {
  readonly version = PORT_VERSIONS.watcher;

  constructor(
    private readonly intervalMs: number = POLL_INTERVAL_MS,
    private readonly debounceMs: number = WATCH_DEBOUNCE_MS,
  ) {}

  subscribe(
    paths: readonly string[],
    onEvent: (event: WatchEvent) => void,
  ): Promise<WatchSubscription> {
    const debouncer = new Debouncer(this.debounceMs, onEvent);
    const timers = paths.map((root) => {
      const differ = new SnapshotDiffer(root, debouncer);
      differ.prime();
      return setInterval(() => differ.rescan(), this.intervalMs);
    });
    return Promise.resolve({
      close: () => {
        for (const timer of timers) clearInterval(timer);
        debouncer.close();
        return Promise.resolve();
      },
    });
  }
}

/** Default ceiling on concurrent index calls the bridge keeps in flight (specs/011 R4). */
export const WATCH_MAX_IN_FLIGHT = 8;

/** The write engine's atomic-swap temp suffix (§7.5): `<path>.workbench-tmp-<pid>`. */
const WRITE_ENGINE_TMP = /\.workbench-tmp-\d+$/;

export type BridgeOutcome = "refreshed" | "removed" | "failed";

export interface BridgeOptions {
  /** At most this many `refreshFile`/`removeFile` calls in flight (default 8). */
  readonly maxInFlight?: number;
  /** Extra per-path filter over the workspace-relative path; true = ignore. */
  readonly ignore?: (rel: string) => boolean;
  /** A settled watcher event that passed the filters and was queued. */
  readonly onEvent?: (kind: WatchEventKind, rel: string) => void;
  /** One index call finished: what happened, and the error for `failed`. */
  readonly onSettled?: (rel: string, outcome: BridgeOutcome, error?: unknown) => void;
}

export interface BridgeSubscription extends WatchSubscription {
  /** Resolves once every queued event has been drained (tests and shutdown). */
  idle(): Promise<void>;
  /** Queue depth + in-flight count right now. */
  backlog(): number;
}

/**
 * The watcher → indexer bridge: subscribes to `root` and forwards settled
 * events as incremental re-indexes — created/modified files re-read from
 * disk via `refreshFile`, removed files dropped via `removeFile`. A refresh
 * that finds the file already gone degrades to removal; any other failure
 * is swallowed, because the query-time verification (never the watcher) is
 * what freshness correctness rests on.
 *
 * Bounded (specs/011 R4): events enter a per-path, arrival-ordered queue — a
 * newer event for a queued path replaces it and a removal supersedes a
 * queued refresh — and a drain loop keeps at most `maxInFlight` index calls
 * in flight. The engine child refuses requests past its pending ceiling
 * (`ENGINE_BUSY`), so an unbounded `git checkout` burst would otherwise
 * drop most of its refreshes; the bound keeps the watcher from competing
 * with calls that have a caller waiting. `.git/`, `.workbench/` and the write
 * engine's own temp files never reach the index.
 */
export async function bindWatcherToIndex(
  watcher: WatcherPort,
  index: IndexPort,
  root: string,
  options: BridgeOptions = {},
): Promise<BridgeSubscription> {
  const base = toPosix(root);
  const maxInFlight = Math.max(1, options.maxInFlight ?? WATCH_MAX_IN_FLIGHT);
  const queue = new Map<string, WatchEventKind>();
  let inFlight = 0;
  let closed = false;
  const idleWaiters: (() => void)[] = [];

  const settleIdle = (): void => {
    if (queue.size === 0 && inFlight === 0) {
      for (const resolve of idleWaiters.splice(0)) resolve();
    }
  };

  const run = async (rel: string, kind: WatchEventKind): Promise<void> => {
    try {
      if (kind === "removed") {
        await index.removeFile(rel);
        options.onSettled?.(rel, "removed");
      } else {
        await index.refreshFile(rel);
        options.onSettled?.(rel, "refreshed");
      }
    } catch (error) {
      try {
        await index.removeFile(rel);
        options.onSettled?.(rel, "failed", error);
      } catch (removeError) {
        // Unreachable index: verification will answer cold until it heals.
        options.onSettled?.(rel, "failed", removeError);
      }
    }
  };

  const drain = (): void => {
    while (!closed && inFlight < maxInFlight && queue.size > 0) {
      const next = queue.entries().next();
      if (next.done) break;
      const [rel, kind] = next.value;
      queue.delete(rel);
      inFlight += 1;
      void run(rel, kind).finally(() => {
        inFlight -= 1;
        drain();
        settleIdle();
      });
    }
    settleIdle();
  };

  const subscription = await watcher.subscribe([root], (event) => {
    if (closed) return;
    const rel = toPosix(relative(base, event.path));
    if (
      rel === "" ||
      rel.startsWith("..") ||
      rel.startsWith(".git/") ||
      rel.startsWith(".workbench/") ||
      WRITE_ENGINE_TMP.test(rel) ||
      options.ignore?.(rel) === true
    ) {
      return;
    }
    options.onEvent?.(event.kind, rel);
    // Coalesce: the newest kind wins, and a removal supersedes a queued
    // refresh; re-insert so arrival order is the order of the LAST event.
    queue.delete(rel);
    queue.set(rel, event.kind);
    drain();
  });

  return {
    close: async () => {
      closed = true;
      queue.clear();
      await subscription.close();
      settleIdle();
    },
    idle: () =>
      new Promise<void>((resolve) => {
        if (queue.size === 0 && inFlight === 0) resolve();
        else idleWaiters.push(resolve);
      }),
    backlog: () => queue.size + inFlight,
  };
}
