/**
 * The schema migration runner (SDD §10.2) — every SQLite database opens
 * through here, stamped with `PRAGMA user_version` and classified:
 *
 * - **cache-class** (index, telemetry): a cache of the filesystem, which is
 *   the truth. A newer or unreadable or unmigratable database is discarded
 *   and rebuilt, never misread; forward migrations run when a registered
 *   chain makes them cheaper than rebuild.
 * - **durable-class** (workspace, journal, config): user data. Forward-only
 *   migrations run at open; an unreadable database is quarantined intact
 *   (renamed, never deleted) and a fresh one is created; a newer database —
 *   or a version gap the registered chain cannot bridge — fails closed with
 *   `UNSUPPORTED_VERSION` and the file is left untouched.
 *
 * journal.db (M2) and the config store (M3) adopt this runner when they are
 * built. Two on-disk formats are deliberately *not* versioned here:
 * `.workbench/server.lock` (transient advisory-lock JSON, rewritten every
 * boot) and `.workbench/keys/` (raw key material with no schema to migrate).
 *
 * Determinism: quarantine names and every note are counter-suffixed and
 * timestamp-free — workspace notes ride envelopes, and the statelessness
 * gate compares those bytes across server processes.
 */

import { Database } from "bun:sqlite";
import { existsSync, renameSync, rmSync } from "node:fs";
import { basename } from "node:path";

export type DataClass = "cache" | "durable";

/** One forward step: transforms a version-`from` schema into `from + 1`. */
export interface Migration {
  readonly from: number;
  readonly migrate: (db: Database) => void;
}

export interface OpenVersionedOptions {
  /** Absolute path of the database file. */
  readonly path: string;
  readonly dataClass: DataClass;
  /** The schema version this binary writes; a positive integer. */
  readonly version: number;
  /** Applied before anything else touches the file; defaults to WAL. */
  readonly journalMode?: "WAL" | "TRUNCATE";
  /** Forward chain; only steps in `[found, version)` are consulted. */
  readonly migrations?: readonly Migration[];
  /** Creates the current-version schema on a fresh, empty database. */
  readonly bootstrap: (db: Database) => void;
}

export type OpenOutcome = "fresh" | "current" | "migrated" | "rebuilt" | "quarantined";

export interface VersionedDatabase {
  readonly db: Database;
  readonly outcome: OpenOutcome;
  /** The stamped version found on disk, when one was (migrated/rebuilt). */
  readonly fromVersion?: number;
  /** Where the unreadable durable file went, for the note and for doctor. */
  readonly quarantinedTo?: string;
  /** Deterministic degradation notes; empty on the healthy paths. */
  readonly notes: readonly string[];
}

/** Durable-class refusal (SDD §10.2): never guess at a version we cannot
 * hold. Carries the taxonomy code so the protocol layer maps it 1:1. */
export class UnsupportedVersionError extends Error {
  readonly code = "UNSUPPORTED_VERSION";
  constructor(message: string, options?: { cause?: unknown }) {
    super(message, options);
    this.name = "UnsupportedVersionError";
  }
}

/** SQLite sidecar suffixes that must travel (or die) with the main file. */
const SIDECARS = ["-wal", "-shm", "-journal"] as const;

function removeDatabaseFiles(path: string): void {
  rmSync(path, { force: true });
  for (const suffix of SIDECARS) rmSync(path + suffix, { force: true });
}

/**
 * Rename the unreadable durable file (and sidecars) aside, intact. The name
 * is counter-suffixed, not timestamped: `<file>.quarantined`, then
 * `<file>.quarantined.2`, … — deterministic given the directory contents.
 */
function quarantineDatabaseFiles(path: string): string {
  let target = `${path}.quarantined`;
  for (let n = 2; existsSync(target); n++) target = `${path}.quarantined.${n}`;
  renameSync(path, target);
  for (const suffix of SIDECARS) {
    if (existsSync(path + suffix)) renameSync(path + suffix, target + suffix);
  }
  return target;
}

interface Probe {
  readonly userVersion: number;
  readonly empty: boolean;
}

/**
 * Read the version and schema presence without writing a byte. The probe is
 * read-only on purpose: a read-write open applies the journal-mode PRAGMA
 * (which rewrites the header) and lets SQLite unlink stale sidecars — both
 * of which would corrupt the "fail closed, file untouched" and "quarantined
 * intact" guarantees before any decision was made. Opening a corrupt file
 * succeeds lazily; the two reads are what surface SQLITE_NOTADB, so
 * "readable" is a measured fact.
 */
function probeDatabase(path: string): Probe | undefined {
  let db: Database | undefined;
  try {
    db = new Database(path, { readonly: true });
    const version = db.query("PRAGMA user_version").get() as { user_version: number } | null;
    const master = db.query("SELECT count(*) AS n FROM sqlite_master").get() as {
      n: number;
    } | null;
    return {
      userVersion: version?.user_version ?? 0,
      empty: (master?.n ?? 0) === 0,
    };
  } catch {
    return undefined;
  } finally {
    db?.close();
  }
}

function openReadWrite(path: string, journalMode: "WAL" | "TRUNCATE"): Database {
  const db = new Database(path, { create: true });
  db.run(`PRAGMA journal_mode = ${journalMode}`);
  return db;
}

function openFresh(
  path: string,
  journalMode: "WAL" | "TRUNCATE",
  options: OpenVersionedOptions,
): Database {
  const db = openReadWrite(path, journalMode);
  options.bootstrap(db);
  db.run(`PRAGMA user_version = ${options.version}`);
  return db;
}

/** The complete forward chain `found → target`, or undefined if any step is missing. */
function chainFrom(
  migrations: readonly Migration[] | undefined,
  found: number,
  target: number,
): readonly Migration[] | undefined {
  const chain: Migration[] = [];
  for (let v = found; v < target; v++) {
    const step = migrations?.find((m) => m.from === v);
    if (step === undefined) return undefined;
    chain.push(step);
  }
  return chain;
}

/** Open `options.path` under the SDD §10.2 lifecycle contract. */
export function openVersionedDatabase(options: OpenVersionedOptions): VersionedDatabase {
  const { path, dataClass, version } = options;
  if (!Number.isInteger(version) || version < 1) {
    throw new Error(`schema version must be a positive integer, got ${version}`);
  }
  const journalMode = options.journalMode ?? "WAL";
  const name = basename(path);

  if (!existsSync(path)) {
    return { db: openFresh(path, journalMode, options), outcome: "fresh", notes: [] };
  }

  const probe = probeDatabase(path);
  if (probe === undefined) {
    // Unreadable. Cache is discarded and rebuilt; durable is set aside intact.
    if (dataClass === "cache") {
      removeDatabaseFiles(path);
      return {
        db: openFresh(path, journalMode, options),
        outcome: "rebuilt",
        notes: [`${name}: unreadable cache database discarded and rebuilt`],
      };
    }
    const quarantinedTo = quarantineDatabaseFiles(path);
    return {
      db: openFresh(path, journalMode, options),
      outcome: "quarantined",
      quarantinedTo,
      notes: [
        `${name}: unreadable durable database quarantined intact as ` +
          `${basename(quarantinedTo)}; a fresh database was created`,
      ],
    };
  }

  const found = probe.userVersion;

  // An empty database file: nothing on disk to migrate or misread.
  if (probe.empty) {
    return { db: openFresh(path, journalMode, options), outcome: "fresh", notes: [] };
  }

  if (found === version) {
    return { db: openReadWrite(path, journalMode), outcome: "current", notes: [] };
  }

  if (found > version) {
    // Downgrade: an older binary must never guess at a newer format.
    if (dataClass === "durable") {
      throw new UnsupportedVersionError(
        `${name} is format version ${found}, newer than the supported ${version}; ` +
          "refusing to open durable data written by a newer binary",
      );
    }
    removeDatabaseFiles(path);
    return {
      db: openFresh(path, journalMode, options),
      outcome: "rebuilt",
      fromVersion: found,
      notes: [`${name}: cache format version ${found} is newer than ${version}; rebuilt`],
    };
  }

  // found < version: forward-migrate along the registered chain.
  const chain = chainFrom(options.migrations, found, version);
  if (chain === undefined) {
    if (dataClass === "durable") {
      throw new UnsupportedVersionError(
        `${name} is format version ${found} and no forward migration chain reaches ` +
          `${version}; refusing to touch durable data`,
      );
    }
    removeDatabaseFiles(path);
    return {
      db: openFresh(path, journalMode, options),
      outcome: "rebuilt",
      fromVersion: found,
      notes: [
        `${name}: cache format version ${found} has no migration chain to ${version}; rebuilt`,
      ],
    };
  }

  const db = openReadWrite(path, journalMode);
  try {
    db.transaction(() => {
      for (const step of chain) {
        step.migrate(db);
        db.run(`PRAGMA user_version = ${step.from + 1}`);
      }
    })();
  } catch (error) {
    // The transaction rolled back; the file still holds version `found`.
    db.close();
    if (dataClass === "durable") {
      throw new UnsupportedVersionError(
        `${name}: forward migration from version ${found} to ${version} failed; ` +
          "durable data left untouched",
        { cause: error },
      );
    }
    removeDatabaseFiles(path);
    return {
      db: openFresh(path, journalMode, options),
      outcome: "rebuilt",
      fromVersion: found,
      notes: [`${name}: cache migration from version ${found} failed; rebuilt`],
    };
  }
  return { db, outcome: "migrated", fromVersion: found, notes: [] };
}
