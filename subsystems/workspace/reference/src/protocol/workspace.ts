/**
 * The workspace runtime (SDD §5.2): everything the server must settle about
 * its workspace before it may answer a single request.
 *
 * - `.workbench/` bootstrap under the workspace root.
 * - Advisory lock: exactly one server instance per workspace. A second
 *   instance is refused before any transport exists — the refusal is a
 *   process-level event (stderr + exit), not an envelope, because no
 *   negotiated request is in flight yet.
 * - Network-filesystem detection: SQLite WAL's shared-memory coordination is
 *   unsafe on network filesystems, so detection flips the journal mode to
 *   TRUNCATE and the degradation rides every envelope's `notes`. The server
 *   never silently runs in a configuration that can corrupt its own index.
 * - The journal-mode decision is applied to a real database at startup and
 *   read back via PRAGMA, so "flipped" is a measured fact, not a config value.
 *
 * Everything derived here is deterministic for a given workspace and
 * environment: no pids, no timestamps, no counters. The statelessness gate
 * compares envelopes byte-for-byte across server processes, and these notes
 * are part of those bytes.
 */

import type { Database } from "bun:sqlite";
import { existsSync, mkdirSync, readFileSync, rmSync, unlinkSync, writeFileSync } from "node:fs";
import { hostname } from "node:os";
import { isAbsolute, join, resolve } from "node:path";
import { configNotes, type ResolvedConfig, resolveConfig } from "../config";
import { openVersionedDatabase } from "../data/versioned-db";
import { ensureWorkspaceKey } from "../jail";

/** Env override for network-FS detection: "1" forces network, "0" forces local. */
export const NETWORK_FS_ENV = "WORKBENCH_NETWORK_FS";

/** workspace.db format version, stamped as `PRAGMA user_version` (SDD §10.2). */
export const WORKSPACE_SCHEMA_VERSION = 1;

export type JournalMode = "WAL" | "TRUNCATE";

export type NetworkFsMethod = "env-override" | "unc-path" | "proc-mounts" | "default-local";

export interface NetworkFsDetection {
  readonly networkFs: boolean;
  readonly method: NetworkFsMethod;
  /** The matched filesystem type, when a mount table named one. */
  readonly fstype?: string;
}

/** Linux mount-table filesystem types that mean "network" for WAL purposes. */
const NETWORK_FSTYPES = new Set([
  "nfs",
  "nfs4",
  "cifs",
  "smbfs",
  "smb3",
  "sshfs",
  "9p",
  "afs",
  "ncpfs",
]);

function isNetworkFstype(fstype: string): boolean {
  // FUSE-cloud mounts (rclone, s3fs, gcsfuse, …) report as "fuse.<driver>".
  return NETWORK_FSTYPES.has(fstype) || fstype.startsWith("fuse.");
}

/** Longest-mount-point-prefix lookup over /proc/mounts (Linux only). */
function detectFromProcMounts(absPath: string): NetworkFsDetection | undefined {
  let table: string;
  try {
    table = readFileSync("/proc/mounts", "utf8");
  } catch {
    return undefined;
  }
  let bestLen = -1;
  let bestFstype: string | undefined;
  for (const line of table.split("\n")) {
    const fields = line.split(" ");
    const mountPoint = fields[1];
    const fstype = fields[2];
    if (mountPoint === undefined || fstype === undefined) continue;
    // /proc/mounts octal-escapes spaces as \040.
    const decoded = mountPoint.replaceAll("\\040", " ");
    if (
      absPath === decoded ||
      absPath.startsWith(decoded.endsWith("/") ? decoded : `${decoded}/`)
    ) {
      if (decoded.length > bestLen) {
        bestLen = decoded.length;
        bestFstype = fstype;
      }
    }
  }
  if (bestFstype === undefined) return undefined;
  return isNetworkFstype(bestFstype)
    ? { networkFs: true, method: "proc-mounts", fstype: bestFstype }
    : { networkFs: false, method: "proc-mounts", fstype: bestFstype };
}

/**
 * Detect whether `absPath` lives on a network filesystem. Precedence: the env
 * override (tests, operators), Windows UNC paths, the Linux mount table, then
 * local-by-default. On platforms with no readable mount table (macOS) an
 * undetected network mount stays "local" — an honest limit of v1 detection,
 * carried in the method field rather than hidden.
 */
export function detectNetworkFs(
  absPath: string,
  env: Record<string, string | undefined> = process.env,
): NetworkFsDetection {
  const override = env[NETWORK_FS_ENV];
  if (override === "1") return { networkFs: true, method: "env-override" };
  if (override === "0") return { networkFs: false, method: "env-override" };
  if (absPath.startsWith("\\\\") || absPath.startsWith("//")) {
    return { networkFs: true, method: "unc-path" };
  }
  return detectFromProcMounts(absPath) ?? { networkFs: false, method: "default-local" };
}

export interface WorkspaceRuntime {
  /** Canonical absolute workspace root. */
  readonly root: string;
  /** `<root>/.workbench` */
  readonly stateDir: string;
  readonly detection: NetworkFsDetection;
  readonly journalMode: JournalMode;
  /** The journal mode SQLite actually reports after the PRAGMA was applied. */
  readonly appliedJournalMode: string;
  /** The five-layer resolved configuration with per-key provenance (§7.2). */
  readonly config: ResolvedConfig;
  /**
   * Deterministic degradation notes for every envelope this server emits.
   * Empty on a healthy local workspace.
   */
  readonly notes: readonly string[];
  /** Release the advisory lock and close the workspace database. */
  close(): void;
}

/** Thrown when another live server instance already owns the workspace. */
export class WorkspaceLockedError extends Error {
  readonly holderPid: number | undefined;
  constructor(lockPath: string, holderPid: number | undefined) {
    super(
      `another workbench-mcp instance${holderPid === undefined ? "" : ` (pid ${holderPid})`} ` +
        `already owns this workspace (lock: ${lockPath})`,
    );
    this.name = "WorkspaceLockedError";
    this.holderPid = holderPid;
  }
}

interface LockRecord {
  readonly pid: number;
  readonly host: string;
}

function readLockRecord(lockPath: string): LockRecord | undefined {
  try {
    const parsed: unknown = JSON.parse(readFileSync(lockPath, "utf8"));
    if (
      typeof parsed === "object" &&
      parsed !== null &&
      "pid" in parsed &&
      typeof (parsed as { pid: unknown }).pid === "number"
    ) {
      return parsed as LockRecord;
    }
  } catch {
    // Unreadable or torn lock file: treated as stale below.
  }
  return undefined;
}

function pidAlive(pid: number): boolean {
  try {
    process.kill(pid, 0);
    return true;
  } catch (error) {
    // EPERM means the pid exists but belongs to someone else — still alive.
    return (error as NodeJS.ErrnoException).code === "EPERM";
  }
}

/**
 * Take the advisory lock via exclusive create. A lock file naming a dead pid
 * on this host is stale (a crashed server) and is broken exactly once;
 * a live pid — or any pid from another host — refuses.
 */
function acquireLock(lockPath: string): void {
  const record: LockRecord = { pid: process.pid, host: hostname() };
  for (let attempt = 0; attempt < 2; attempt++) {
    try {
      writeFileSync(lockPath, JSON.stringify(record), { flag: "wx" });
      return;
    } catch (error) {
      if ((error as NodeJS.ErrnoException).code !== "EEXIST") throw error;
      const holder = readLockRecord(lockPath);
      const stale = holder === undefined || (holder.host === record.host && !pidAlive(holder.pid));
      if (!stale || attempt > 0) {
        throw new WorkspaceLockedError(lockPath, holder?.pid);
      }
      rmSync(lockPath, { force: true });
    }
  }
}

/**
 * Open the workspace: bootstrap `.workbench/`, take the advisory lock, detect
 * the filesystem, and open the workspace database in the journal mode the
 * detection decided — then read the mode back so the flip is proven, not
 * assumed.
 */
export function openWorkspace(
  rootArg: string,
  env: Record<string, string | undefined> = process.env,
  sessionFlags: Readonly<Record<string, unknown>> = {},
): WorkspaceRuntime {
  const root = isAbsolute(rootArg) ? resolve(rootArg) : resolve(process.cwd(), rootArg);
  if (!existsSync(root)) {
    throw new Error(`workspace root does not exist: ${root}`);
  }
  const stateDir = join(root, ".workbench");
  mkdirSync(stateDir, { recursive: true });

  const detection = detectNetworkFs(root, env);
  const journalMode: JournalMode = detection.networkFs ? "TRUNCATE" : "WAL";

  const lockPath = join(stateDir, "server.lock");
  acquireLock(lockPath);

  let db: Database;
  let config: ResolvedConfig;
  const lifecycleNotes: string[] = [];
  try {
    // Five-layer config (§7.2): resolved before the first request, unknown
    // keys and policy denials become deterministic envelope notes. A layer
    // written by a newer binary fails closed here (§10.2), like the db.
    config = resolveConfig({ root, env, sessionFlags });
    // Init-time key generation (SDD §7.1): the MRTR confirm-token key exists
    // with 0600 under .workbench/keys/ before the first request is answered.
    ensureWorkspaceKey(stateDir);
    // Durable-class (SDD §10.2): workspace.db is user data. A newer format
    // fails closed (UNSUPPORTED_VERSION, thrown here as a process-level
    // refusal like the lock), an unreadable file is quarantined intact —
    // never deleted — and the quarantine rides this server's notes.
    const opened = openVersionedDatabase({
      path: join(stateDir, "workspace.db"),
      dataClass: "durable",
      version: WORKSPACE_SCHEMA_VERSION,
      journalMode,
      migrations: [
        {
          // Pre-runner databases: the version-1 meta table, unstamped.
          from: 0,
          migrate: (legacy) => {
            legacy.run(
              "CREATE TABLE IF NOT EXISTS meta (key TEXT PRIMARY KEY, value TEXT NOT NULL)",
            );
          },
        },
      ],
      bootstrap: (fresh) => {
        fresh.run("CREATE TABLE IF NOT EXISTS meta (key TEXT PRIMARY KEY, value TEXT NOT NULL)");
      },
    });
    db = opened.db;
    lifecycleNotes.push(...opened.notes);
  } catch (error) {
    unlinkSync(lockPath);
    throw error;
  }
  const applied = db.query("PRAGMA journal_mode").get() as { journal_mode: string } | null;
  const appliedJournalMode = applied?.journal_mode ?? "unknown";

  const notes: string[] = [...lifecycleNotes];
  if (detection.networkFs) {
    notes.push(
      `workspace is on a network filesystem (detected via ${detection.method}` +
        `${detection.fstype === undefined ? "" : `: ${detection.fstype}`}): ` +
        "SQLite journal mode degraded WAL→TRUNCATE; " +
        "concurrent instances are refused by the advisory lock",
    );
  }
  notes.push(...configNotes(config));

  let closed = false;
  return {
    root,
    stateDir,
    detection,
    journalMode,
    appliedJournalMode,
    config,
    notes,
    close() {
      if (closed) return;
      closed = true;
      db.close();
      rmSync(lockPath, { force: true });
    },
  };
}
