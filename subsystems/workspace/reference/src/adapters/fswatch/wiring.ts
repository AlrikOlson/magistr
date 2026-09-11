/**
 * Watcher wiring (specs/011-rust-watcher-wiring): how the serving path
 * turns the `watcher.mode` setting and the workspace's network-fs detection
 * into a running subscription, and how that subscription publishes a
 * cache-class status record the out-of-process `doctor` can read.
 *
 *   selectWatcherMode  — auto ⇒ polling on a detected network filesystem
 *                        (native watching lies there), native otherwise;
 *                        forced modes are honored; off ⇒ nothing.
 *   startWatcher       — builds the adapter, binds it through the bounded
 *                        bridge, and writes `.workbench/watcher.json` at
 *                        start, throttled while events flow, and at stop.
 *                        Never throws into the caller: a failed subscribe
 *                        is recorded in the file and answers null.
 *   readWatcherStatus  — doctor's read-only view: live (pid alive),
 *                        last-session (pid gone), not-active (no record),
 *                        unreadable (named file).
 *
 * The record is cache-class: never read by the server, never migrated,
 * reported-and-ignored when unreadable.
 */

import { existsSync, mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import type { IndexPort } from "../../ports";
import {
  type BridgeOutcome,
  type BridgeSubscription,
  bindWatcherToIndex,
  FsWatchAdapter,
  PollingWatcherAdapter,
} from "./watcher";

export const WATCHER_MODES = ["auto", "native", "poll", "off"] as const;
export type WatcherMode = (typeof WATCHER_MODES)[number];
export type WatcherAdapterName = "fs.watch" | "polling";

export const WATCHER_STATUS_FILE = "watcher.json";
/** Status writes while events flow are at least this far apart. */
export const WATCHER_STATUS_THROTTLE_MS = 1_000;

export interface WatcherStatusRecord {
  readonly v: 1;
  readonly pid: number;
  readonly mode: WatcherMode;
  readonly adapter: WatcherAdapterName;
  readonly root: string;
  readonly startedAtMs: number;
  readonly updatedAtMs: number;
  readonly stoppedAtMs?: number;
  readonly events: number;
  readonly refreshes: number;
  readonly removals: number;
  readonly failures: number;
  readonly lastEventAtMs?: number;
  readonly lastError?: string;
}

export function isWatcherMode(value: unknown): value is WatcherMode {
  return typeof value === "string" && (WATCHER_MODES as readonly string[]).includes(value);
}

/** The adapter a setting resolves to under the workspace's detection; null = off. */
export function selectWatcherMode(
  setting: unknown,
  networkFs: boolean,
): { readonly mode: WatcherMode; readonly adapter: WatcherAdapterName | null } {
  const mode: WatcherMode = isWatcherMode(setting) ? setting : "auto";
  switch (mode) {
    case "off":
      return { mode, adapter: null };
    case "native":
      return { mode, adapter: "fs.watch" };
    case "poll":
      return { mode, adapter: "polling" };
    case "auto":
      return { mode, adapter: networkFs ? "polling" : "fs.watch" };
  }
}

export interface StartWatcherOptions {
  readonly root: string;
  readonly stateDir: string;
  readonly index: IndexPort;
  readonly mode: unknown;
  readonly networkFs: boolean;
  readonly now?: () => number;
  readonly pid?: number;
  /** Test seam: the bridge's in-flight ceiling. */
  readonly maxInFlight?: number;
  /** Test seam: observe every settled bridge outcome. */
  readonly onSettled?: (rel: string, outcome: BridgeOutcome, error?: unknown) => void;
}

export interface RunningWatcher {
  readonly mode: WatcherMode;
  readonly adapter: WatcherAdapterName;
  /** Current counters (a copy). */
  snapshot(): WatcherStatusRecord;
  /** Drain the bridge queue (tests). */
  idle(): Promise<void>;
  close(): Promise<void>;
}

function writeRecord(stateDir: string, record: WatcherStatusRecord): void {
  try {
    mkdirSync(stateDir, { recursive: true });
    writeFileSync(join(stateDir, WATCHER_STATUS_FILE), `${JSON.stringify(record, null, 2)}\n`);
  } catch {
    // Cache-class: a record that cannot be written costs nothing but the probe.
  }
}

/**
 * Start the watcher for a served workspace. Returns null when the mode is
 * off or the subscription could not be made (recorded, never thrown).
 */
export async function startWatcher(options: StartWatcherOptions): Promise<RunningWatcher | null> {
  const { mode, adapter } = selectWatcherMode(options.mode, options.networkFs);
  if (adapter === null) return null;
  const now = options.now ?? Date.now;
  const pid = options.pid ?? process.pid;
  const startedAtMs = now();
  let record: WatcherStatusRecord = {
    v: 1,
    pid,
    mode,
    adapter,
    root: options.root,
    startedAtMs,
    updatedAtMs: startedAtMs,
    events: 0,
    refreshes: 0,
    removals: 0,
    failures: 0,
  };
  let lastWriteMs = 0;
  let flushTimer: ReturnType<typeof setTimeout> | null = null;
  const flush = (): void => {
    flushTimer = null;
    lastWriteMs = now();
    record = { ...record, updatedAtMs: lastWriteMs };
    writeRecord(options.stateDir, record);
  };
  const schedule = (): void => {
    if (flushTimer !== null) return;
    const wait = Math.max(0, WATCHER_STATUS_THROTTLE_MS - (now() - lastWriteMs));
    flushTimer = setTimeout(flush, wait);
    flushTimer.unref?.();
  };

  const watcher = adapter === "fs.watch" ? new FsWatchAdapter() : new PollingWatcherAdapter();
  let bridge: BridgeSubscription;
  try {
    bridge = await bindWatcherToIndex(watcher, options.index, options.root, {
      ...(options.maxInFlight === undefined ? {} : { maxInFlight: options.maxInFlight }),
      onEvent: () => {
        record = { ...record, events: record.events + 1, lastEventAtMs: now() };
        schedule();
      },
      onSettled: (rel, outcome, error) => {
        if (outcome === "refreshed") record = { ...record, refreshes: record.refreshes + 1 };
        else if (outcome === "removed") record = { ...record, removals: record.removals + 1 };
        else {
          record = {
            ...record,
            failures: record.failures + 1,
            lastError: error instanceof Error ? error.message : String(error),
          };
        }
        options.onSettled?.(rel, outcome, error);
        schedule();
      },
    });
  } catch (error) {
    record = {
      ...record,
      stoppedAtMs: now(),
      failures: 1,
      lastError: `subscribe failed: ${error instanceof Error ? error.message : String(error)}`,
    };
    writeRecord(options.stateDir, record);
    return null;
  }
  flush();

  return {
    mode,
    adapter,
    snapshot: () => ({ ...record }),
    idle: () => bridge.idle(),
    close: async () => {
      if (flushTimer !== null) {
        clearTimeout(flushTimer);
        flushTimer = null;
      }
      await bridge.close();
      record = { ...record, stoppedAtMs: now() };
      flush();
    },
  };
}

export type WatcherStatusView =
  | { readonly state: "live"; readonly record: WatcherStatusRecord }
  | { readonly state: "last-session"; readonly record: WatcherStatusRecord }
  | { readonly state: "not-active"; readonly reason: string }
  | { readonly state: "unreadable"; readonly file: string; readonly error: string };

function defaultPidAlive(pid: number): boolean {
  try {
    process.kill(pid, 0);
    return true;
  } catch {
    return false;
  }
}

/** doctor's read-only view of the status record; never throws, never writes. */
export function readWatcherStatus(
  stateDir: string,
  pidAlive: (pid: number) => boolean = defaultPidAlive,
): WatcherStatusView {
  const file = join(stateDir, WATCHER_STATUS_FILE);
  if (!existsSync(file)) {
    return {
      state: "not-active",
      reason:
        "no watcher record: watcher.mode is off, or no server of this build has served this workspace yet",
    };
  }
  let record: WatcherStatusRecord;
  try {
    const parsed = JSON.parse(readFileSync(file, "utf8")) as Partial<WatcherStatusRecord>;
    if (
      parsed.v !== 1 ||
      typeof parsed.pid !== "number" ||
      !isWatcherMode(parsed.mode) ||
      (parsed.adapter !== "fs.watch" && parsed.adapter !== "polling")
    ) {
      throw new Error("record is not a v1 watcher status");
    }
    record = parsed as WatcherStatusRecord;
  } catch (error) {
    return {
      state: "unreadable",
      file: `.workbench/${WATCHER_STATUS_FILE}`,
      error: error instanceof Error ? error.message : String(error),
    };
  }
  if (record.stoppedAtMs === undefined && pidAlive(record.pid)) return { state: "live", record };
  return { state: "last-session", record };
}
