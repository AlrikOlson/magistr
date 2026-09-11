/**
 * Engine-child supervision (SDD §6.2). The server spawns `workbench-mcp
 * --engine`, speaks the framed IPC protocol to it, and turns child crashes
 * into degradation instead of outage: every in-flight request is rejected
 * with a typed error (one degraded answer, never a hang), the child is
 * respawned with exponential backoff, and a file whose requests repeatedly
 * kill the child is quarantined from parsing — recorded for `doctor` —
 * rather than crash-looping the engine.
 */

import { appendFileSync } from "node:fs";
import path from "node:path";
import { isErrorCode } from "../../envelope";
import type { Tracer } from "../../ports/tracing";
import {
  type DecodedFrame,
  encodeFrame,
  type Frame,
  FrameDecodeError,
  FrameDecoder,
  IPC_VERSION,
} from "./frames";

/** The child is not answering (crashed, unavailable, or deliberately stopped). */
export class EngineUnavailableError extends Error {
  readonly code = "ENGINE_UNAVAILABLE";

  constructor(message: string) {
    super(message);
    this.name = "EngineUnavailableError";
  }
}

/**
 * The input file is quarantined: it repeatedly crashed the engine child.
 * Carries `INPUT_QUARANTINED` (recovery `escalate`, §4.3 v1.3 amendment) —
 * the refusal is deterministic until the file changes, so `retry` is wrong.
 */
export class InputQuarantinedError extends Error {
  readonly code = "INPUT_QUARANTINED";

  constructor(readonly path: string) {
    super(`${path} is quarantined: it repeatedly crashed the engine child (see doctor)`);
    this.name = "InputQuarantinedError";
  }
}

/** The caller cancelled the request (§7.7 cancellation propagation). */
export class RequestCancelledError extends Error {
  constructor(id: number) {
    super(`request ${id} cancelled`);
    this.name = "RequestCancelledError";
  }
}

/**
 * The bounded request queue is full (§7.7 typed backpressure). Carries
 * `ENGINE_BUSY` (recovery `retry`) plus the suggested backoff — the caller
 * gets a typed answer immediately instead of a hang-then-timeout.
 */
export class EngineBusyError extends Error {
  readonly code = "ENGINE_BUSY";

  constructor(
    pending: number,
    readonly backoffMs: number,
  ) {
    super(`engine queue is full (${pending} in flight); retry after ~${backoffMs}ms`);
    this.name = "EngineBusyError";
  }
}

/** The child answered with an error frame. */
export class EngineIpcError extends Error {
  constructor(
    readonly code: string,
    message: string,
  ) {
    super(message);
    this.name = "EngineIpcError";
  }
}

export interface QuarantineEntry {
  readonly path: string;
  readonly strikes: number;
  readonly quarantined: boolean;
  /** Exit code or signal description of the most recent crash. */
  readonly lastCrash: string;
  /** Epoch milliseconds of the most recent strike. */
  readonly at: number;
}

export interface EngineSupervisorOptions {
  /** Full spawn argv; defaults to `bun <repo>/src/main.ts --engine`. */
  readonly spawn?: readonly string[];
  /** Extra environment for the child (merged over the parent's). */
  readonly env?: Readonly<Record<string, string | undefined>>;
  /**
   * Restart delays in ms, indexed by consecutive-crash count; the last entry
   * repeats. Index 0 is the first (non-crash) start and should be 0.
   */
  readonly backoffMs?: readonly number[];
  /** Crashes attributed to one path before it is quarantined. */
  readonly maxStrikes?: number;
  /** How long to wait for the child's ready frame before declaring a crash. */
  readonly readyTimeoutMs?: number;
  /** JSONL file to append quarantine records to, for `doctor`. */
  readonly quarantineLog?: string;
  /** Notified whenever a path crosses the quarantine threshold. */
  readonly onQuarantine?: (entry: QuarantineEntry) => void;
  /** Bounded queue (§7.7): in-flight requests beyond this answer ENGINE_BUSY. */
  readonly maxPending?: number;
  /**
   * Flight recorder (feature 005): every `call` made while a request span is
   * active records an `engine:<method>` child. Absent ⇒ nothing is recorded;
   * nothing else changes.
   */
  readonly tracer?: Tracer;
}

const DEFAULT_BACKOFF_MS: readonly number[] = [0, 50, 100, 200, 400, 800];
const DEFAULT_MAX_STRIKES = 3;
const DEFAULT_READY_TIMEOUT_MS = 15_000;
const DEFAULT_MAX_PENDING = 128;
/** Suggested retry delay when the queue is full: one restart-backoff notch. */
const BUSY_BACKOFF_MS = 250;

interface PendingRequest {
  readonly method: string;
  readonly paths: readonly string[];
  readonly resolve: (result: unknown) => void;
  readonly reject: (error: Error) => void;
}

interface ChildHandle {
  readonly proc: Bun.Subprocess<"pipe", "pipe", "inherit">;
  ready: boolean;
}

/** Workspace-relative input paths named by a request's params. */
function pathsOf(params: unknown): readonly string[] {
  if (typeof params !== "object" || params === null) return [];
  const record = params as Record<string, unknown>;
  if (typeof record.path === "string") return [record.path];
  if (Array.isArray(record.paths)) {
    return record.paths.filter((p): p is string => typeof p === "string");
  }
  return [];
}

function defaultSpawnArgv(): readonly string[] {
  const main = path.resolve(import.meta.dirname, "../../main.ts");
  return [process.execPath, main, "--engine"];
}

export interface SupervisorStatus {
  readonly state: "idle" | "starting" | "ready" | "crashed" | "stopped";
  readonly restarts: number;
  readonly consecutiveCrashes: number;
  readonly pending: number;
}

export class EngineSupervisor {
  private readonly options: Required<
    Pick<EngineSupervisorOptions, "spawn" | "backoffMs" | "maxStrikes" | "readyTimeoutMs">
  > &
    EngineSupervisorOptions;

  private child: ChildHandle | null = null;
  private startPromise: Promise<void> | null = null;
  private stopped = false;
  private nextId = 1;
  private readonly pending = new Map<number, PendingRequest>();
  private readonly strikes = new Map<string, QuarantineEntry>();
  private consecutiveCrashes = 0;
  private restartCount = 0;

  constructor(options: EngineSupervisorOptions = {}) {
    this.options = {
      ...options,
      spawn: options.spawn ?? defaultSpawnArgv(),
      backoffMs: options.backoffMs ?? DEFAULT_BACKOFF_MS,
      maxStrikes: options.maxStrikes ?? DEFAULT_MAX_STRIKES,
      readyTimeoutMs: options.readyTimeoutMs ?? DEFAULT_READY_TIMEOUT_MS,
    };
  }

  /**
   * Send one request to the engine child. Rejects — never hangs — when the
   * input is quarantined, the child crashes mid-request, or the caller
   * cancels through `signal`.
   */
  call(method: string, params: unknown, opts: { signal?: AbortSignal } = {}): Promise<unknown> {
    // Child span (feature 005, research R2): this is the one door to the
    // engine child, so wrapping here covers every engine IPC. No active
    // request span ⇒ no record; an outcome with no taxonomy code (e.g. a
    // caller cancellation) ends no span, which emits nothing.
    const span = this.options.tracer?.activeSpan()?.engineChild({ method }) ?? null;
    const paths = pathsOf(params);
    for (const p of paths) {
      if (this.strikes.get(p)?.quarantined === true) {
        span?.end({ ok: false, code: "INPUT_QUARANTINED" });
        return Promise.reject(new InputQuarantinedError(p));
      }
    }
    const maxPending = this.options.maxPending ?? DEFAULT_MAX_PENDING;
    if (this.pending.size >= maxPending) {
      span?.end({ ok: false, code: "ENGINE_BUSY" });
      return Promise.reject(new EngineBusyError(this.pending.size, BUSY_BACKOFF_MS));
    }
    const id = this.nextId++;
    const result = new Promise<unknown>((resolve, reject) => {
      const entry: PendingRequest = { method, paths, resolve, reject };
      this.pending.set(id, entry);
      if (opts.signal !== undefined) {
        opts.signal.addEventListener("abort", () => this.cancel(id), { once: true });
      }
      this.ensureReady()
        .then(() => {
          // The crash handler may have rejected us while the child restarted.
          if (!this.pending.has(id)) return;
          this.write({ v: IPC_VERSION, kind: "req", id, method, params });
        })
        .catch((error: unknown) => {
          if (this.pending.delete(id)) {
            reject(error instanceof Error ? error : new EngineUnavailableError(String(error)));
          }
        });
    });
    if (span !== null) {
      void result.then(
        () => span.end({ ok: true }),
        (error: unknown) => {
          const code = (error as { code?: unknown }).code;
          if (isErrorCode(code)) span.end({ ok: false, code });
        },
      );
    }
    return result;
  }

  /** Cancel an in-flight request: reject it locally, tell the child (§7.7). */
  cancel(id: number): void {
    const entry = this.pending.get(id);
    if (entry === undefined) return;
    this.pending.delete(id);
    entry.reject(new RequestCancelledError(id));
    if (this.child !== null) {
      this.write({ v: IPC_VERSION, kind: "cancel", id });
    }
  }

  /** Deliberate shutdown: no restart, pending requests rejected. */
  async stop(): Promise<void> {
    this.stopped = true;
    const child = this.child;
    this.rejectAllPending(new EngineUnavailableError("engine supervisor stopped"));
    if (child !== null) {
      child.proc.kill();
      await child.proc.exited;
    }
    this.child = null;
  }

  /** The quarantine record, as `doctor` will read it. */
  quarantineReport(): readonly QuarantineEntry[] {
    return [...this.strikes.values()];
  }

  status(): SupervisorStatus {
    const state: SupervisorStatus["state"] = this.stopped
      ? "stopped"
      : this.child === null
        ? this.startPromise !== null
          ? "starting"
          : this.consecutiveCrashes > 0
            ? "crashed"
            : "idle"
        : this.child.ready
          ? "ready"
          : "starting";
    return {
      state,
      restarts: this.restartCount,
      consecutiveCrashes: this.consecutiveCrashes,
      pending: this.pending.size,
    };
  }

  private write(frame: Frame): void {
    const child = this.child;
    if (child === null) return;
    child.proc.stdin.write(encodeFrame(frame));
    void child.proc.stdin.flush();
  }

  private ensureReady(): Promise<void> {
    if (this.stopped) {
      return Promise.reject(new EngineUnavailableError("engine supervisor stopped"));
    }
    if (this.child?.ready === true) return Promise.resolve();
    this.startPromise ??= this.start().finally(() => {
      this.startPromise = null;
    });
    return this.startPromise;
  }

  private backoffDelay(): number {
    const ladder = this.options.backoffMs;
    const index = Math.min(this.consecutiveCrashes, ladder.length - 1);
    return ladder[index] ?? 0;
  }

  private async start(): Promise<void> {
    const delay = this.backoffDelay();
    if (delay > 0) await new Promise((r) => setTimeout(r, delay));
    if (this.stopped) throw new EngineUnavailableError("engine supervisor stopped");
    if (this.consecutiveCrashes > 0) this.restartCount += 1;

    const proc = Bun.spawn({
      cmd: [...this.options.spawn],
      env: { ...process.env, ...this.options.env },
      stdin: "pipe",
      stdout: "pipe",
      stderr: "inherit",
    });
    const child: ChildHandle = { proc, ready: false };
    this.child = child;

    let settleReady: { resolve: () => void; reject: (e: Error) => void } | null = null;
    const ready = new Promise<void>((resolve, reject) => {
      settleReady = { resolve, reject };
    });
    const readyTimer = setTimeout(() => {
      if (!child.ready) proc.kill();
    }, this.options.readyTimeoutMs);

    void this.readLoop(child, () => {
      child.ready = true;
      this.consecutiveCrashes = 0;
      settleReady?.resolve();
    });
    void proc.exited.then((exitCode) => {
      clearTimeout(readyTimer);
      this.onExit(child, exitCode);
      settleReady?.reject(
        new EngineUnavailableError(`engine child exited (${exitCode}) before ready`),
      );
    });

    await ready;
  }

  private async readLoop(child: ChildHandle, onReady: () => void): Promise<void> {
    const decoder = new FrameDecoder();
    try {
      for await (const chunk of child.proc.stdout) {
        for (const decoded of decoder.push(chunk)) {
          this.dispatch(decoded, onReady);
        }
      }
    } catch (error) {
      if (error instanceof FrameDecodeError) {
        // Corrupt stream: kill the child; the exit handler degrades and restarts.
        child.proc.kill();
      } else {
        throw error;
      }
    }
  }

  private dispatch(decoded: DecodedFrame, onReady: () => void): void {
    if (!decoded.ok) return; // The child never sends us malformed frames it expects answered.
    const frame = decoded.frame;
    switch (frame.kind) {
      case "ready":
        onReady();
        return;
      case "res": {
        const entry = this.pending.get(frame.id);
        if (entry !== undefined) {
          this.pending.delete(frame.id);
          entry.resolve(frame.result);
        }
        return;
      }
      case "err": {
        const entry = this.pending.get(frame.id);
        if (entry !== undefined) {
          this.pending.delete(frame.id);
          entry.reject(new EngineIpcError(frame.error.code, frame.error.message));
        }
        return;
      }
      default:
        return; // req/cancel only travel server → child.
    }
  }

  private onExit(child: ChildHandle, exitCode: number | null): void {
    if (this.child !== child) return; // A stale handle from before a restart.
    this.child = null;
    if (this.stopped) return;
    this.consecutiveCrashes += 1;
    const crash = `engine child exited (${exitCode ?? "signal"})`;
    for (const [, entry] of this.pending) {
      for (const p of entry.paths) this.strike(p, crash);
    }
    this.rejectAllPending(new EngineUnavailableError(`${crash} mid-request`));
  }

  private rejectAllPending(error: Error): void {
    const entries = [...this.pending.values()];
    this.pending.clear();
    for (const entry of entries) entry.reject(error);
  }

  private strike(inputPath: string, crash: string): void {
    const previous = this.strikes.get(inputPath);
    const strikes = (previous?.strikes ?? 0) + 1;
    const entry: QuarantineEntry = {
      path: inputPath,
      strikes,
      quarantined: strikes >= this.options.maxStrikes,
      lastCrash: crash,
      at: Date.now(),
    };
    this.strikes.set(inputPath, entry);
    if (entry.quarantined && previous?.quarantined !== true) {
      if (this.options.quarantineLog !== undefined) {
        appendFileSync(this.options.quarantineLog, `${JSON.stringify(entry)}\n`);
      }
      this.options.onQuarantine?.(entry);
    }
  }
}
