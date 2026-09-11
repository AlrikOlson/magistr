/**
 * The per-workspace advisory write lock (SDD §7.7) — the production
 * `WriteLock` adapter the write engine (§7.5) serializes through.
 *
 * Two mechanisms compose, because they guard against different intruders:
 *
 * - An in-process FIFO mutex serializes batches from this process in arrival
 *   order — cheap, fair, and independent of filesystem semantics.
 * - An advisory lock file (`<root>/.workbench/write.lock`, created with the
 *   exclusive `wx` flag) serializes against every OTHER entry point: a second
 *   Workbench process, the CLI. The file carries `{pid, acquiredAtMs}` so a
 *   contender can distinguish a live holder from a corpse: a dead PID or an
 *   over-age lock is taken over (crashed holders must not wedge the
 *   workspace forever).
 *
 * Acquisition is bounded (§7.7 typed backpressure): a holder that does not
 * yield within `acquireTimeoutMs` produces `WorkspaceLockBusyError` with
 * `code: "ENGINE_BUSY"` and a suggested `backoffMs` — never a hang. The
 * journal remains the serialization witness; content-hash anchors close the
 * read-modify-write races the lock does not.
 */

import { mkdirSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import type { WriteLock } from "../ports";

/** §7.7: the bounded-wait answer. Maps to taxonomy ENGINE_BUSY (recovery retry). */
export class WorkspaceLockBusyError extends Error {
  readonly code = "ENGINE_BUSY";
  constructor(
    readonly lockPath: string,
    readonly backoffMs: number,
    holder: string,
  ) {
    super(`workspace write lock is held (${holder}); retry after ~${backoffMs}ms`);
    this.name = "WorkspaceLockBusyError";
  }
}

export interface WorkspaceWriteLockOptions {
  /** How long one acquisition will wait before answering ENGINE_BUSY. */
  readonly acquireTimeoutMs?: number;
  /** Poll interval while another process holds the file. */
  readonly pollMs?: number;
  /** A lock older than this is stale even if its PID is unreadable. */
  readonly staleMs?: number;
  /** Test seam: liveness probe for a holder PID. */
  readonly pidAlive?: (pid: number) => boolean;
}

const DEFAULT_ACQUIRE_TIMEOUT_MS = 10_000;
const DEFAULT_POLL_MS = 50;
/** Generous: covers the longest §7.5 pipeline anyone should hold the lock for. */
const DEFAULT_STALE_MS = 5 * 60_000;

interface LockPayload {
  readonly pid: number;
  readonly acquiredAtMs: number;
}

function defaultPidAlive(pid: number): boolean {
  try {
    process.kill(pid, 0);
    return true;
  } catch (error) {
    // EPERM means "alive but not ours" — still a live holder.
    return (error as NodeJS.ErrnoException).code === "EPERM";
  }
}

export class WorkspaceWriteLock implements WriteLock {
  private readonly lockPath: string;
  private readonly acquireTimeoutMs: number;
  private readonly pollMs: number;
  private readonly staleMs: number;
  private readonly pidAlive: (pid: number) => boolean;
  /** The in-process FIFO: each withLock chains behind the previous one. */
  private tail: Promise<void> = Promise.resolve();

  constructor(root: string, options: WorkspaceWriteLockOptions = {}) {
    this.lockPath = join(root, ".workbench", "write.lock");
    this.acquireTimeoutMs = options.acquireTimeoutMs ?? DEFAULT_ACQUIRE_TIMEOUT_MS;
    this.pollMs = options.pollMs ?? DEFAULT_POLL_MS;
    this.staleMs = options.staleMs ?? DEFAULT_STALE_MS;
    this.pidAlive = options.pidAlive ?? defaultPidAlive;
    mkdirSync(join(root, ".workbench"), { recursive: true });
  }

  async withLock<T>(fn: () => Promise<T>): Promise<T> {
    const previous = this.tail;
    let release!: () => void;
    this.tail = new Promise((resolve) => {
      release = resolve;
    });
    // FIFO position is taken synchronously above; errors of earlier holders
    // must not poison the queue, so the wait swallows them.
    await previous.catch(() => {});
    try {
      await this.acquireFile();
      try {
        return await fn();
      } finally {
        this.releaseFile();
      }
    } finally {
      release();
    }
  }

  /** One bounded create-exclusive loop; §7.7 says never hang. */
  private async acquireFile(): Promise<void> {
    const deadline = Date.now() + this.acquireTimeoutMs;
    for (;;) {
      try {
        writeFileSync(
          this.lockPath,
          JSON.stringify({ pid: process.pid, acquiredAtMs: Date.now() } satisfies LockPayload),
          { flag: "wx" },
        );
        return;
      } catch (error) {
        if ((error as NodeJS.ErrnoException).code !== "EEXIST") throw error;
      }
      const holder = this.readHolder();
      if (holder === null || this.isStale(holder)) {
        // Corpse takeover: remove and race for the wx-create; a concurrent
        // contender winning that race just sends us around the loop again.
        rmSync(this.lockPath, { force: true });
        continue;
      }
      if (Date.now() + this.pollMs > deadline) {
        throw new WorkspaceLockBusyError(this.lockPath, this.pollMs * 5, `pid ${holder.pid}`);
      }
      await Bun.sleep(this.pollMs);
    }
  }

  private releaseFile(): void {
    rmSync(this.lockPath, { force: true });
  }

  private readHolder(): LockPayload | null {
    let raw: string;
    try {
      raw = readFileSync(this.lockPath, "utf8");
    } catch {
      // Vanished between EEXIST and here: the holder released; loop retries.
      return null;
    }
    try {
      const parsed = JSON.parse(raw) as Partial<LockPayload>;
      if (typeof parsed.pid === "number" && typeof parsed.acquiredAtMs === "number") {
        return { pid: parsed.pid, acquiredAtMs: parsed.acquiredAtMs };
      }
    } catch {
      // Unparseable lock files are corpses by definition.
    }
    return null;
  }

  private isStale(holder: LockPayload): boolean {
    // Our own PID is always "alive": a second lock instance in this process
    // (two runtimes over one root) must wait like any other contender.
    if (Date.now() - holder.acquiredAtMs > this.staleMs) return true;
    return holder.pid !== process.pid && !this.pidAlive(holder.pid);
  }
}
