/**
 * The SDD §10.2 migration suite: upgrade and downgrade paths for both data
 * classes through the runner, plus the two wired call sites (workspace.db
 * durable, index.db cache). The acceptance criteria live here verbatim —
 * downgrade against a newer format fails closed with UNSUPPORTED_VERSION,
 * and unreadable durable data is quarantined intact, never deleted.
 */

import { Database } from "bun:sqlite";
import { afterEach, describe, expect, test } from "bun:test";
import { existsSync, mkdirSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import path from "node:path";
import { ChildIndexer, INDEX_SCHEMA_VERSION, stubCoreBinding } from "../src/adapters/engine-ipc";
import {
  type Migration,
  openVersionedDatabase,
  UnsupportedVersionError,
  type VersionedDatabase,
} from "../src/data/versioned-db";
import { openWorkspace, WORKSPACE_SCHEMA_VERSION } from "../src/protocol/workspace";

const LOCAL_ENV = { WORKBENCH_NETWORK_FS: "0" };

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

function tmpDir(): string {
  const dir = mkdtempSync(path.join(tmpdir(), "wb-migrate-"));
  tempDirs.push(dir);
  return dir;
}

const BOOTSTRAP = (db: Database): void => {
  db.run("CREATE TABLE t (k TEXT PRIMARY KEY, v TEXT NOT NULL)");
};

function opened(
  dbPath: string,
  dataClass: "cache" | "durable",
  version: number,
  migrations?: readonly Migration[],
): VersionedDatabase {
  const result = openVersionedDatabase({
    path: dbPath,
    dataClass,
    version,
    ...(migrations === undefined ? {} : { migrations }),
    bootstrap: BOOTSTRAP,
  });
  closers.push(() => result.db.close());
  return result;
}

/** A stamped database with one row, closed again — yesterday's binary. */
function seedDatabase(dbPath: string, version: number): void {
  const db = new Database(dbPath, { create: true });
  BOOTSTRAP(db);
  db.run("INSERT INTO t (k, v) VALUES ('kept', 'row')");
  db.run(`PRAGMA user_version = ${version}`);
  db.close();
}

function userVersionOf(dbPath: string): number {
  const db = new Database(dbPath, { readonly: true });
  try {
    const row = db.query("PRAGMA user_version").get() as { user_version: number } | null;
    return row?.user_version ?? -1;
  } finally {
    db.close();
  }
}

describe("openVersionedDatabase", () => {
  test("a fresh database bootstraps and stamps PRAGMA user_version", () => {
    const dbPath = path.join(tmpDir(), "fresh.db");
    const result = opened(dbPath, "durable", 3);
    expect(result.outcome).toBe("fresh");
    expect(result.notes).toHaveLength(0);
    result.db.run("INSERT INTO t (k, v) VALUES ('a', 'b')");
    result.db.close();
    expect(userVersionOf(dbPath)).toBe(3);
  });

  test("a current database opens as-is with data intact", () => {
    const dbPath = path.join(tmpDir(), "current.db");
    seedDatabase(dbPath, 2);
    const result = opened(dbPath, "durable", 2);
    expect(result.outcome).toBe("current");
    const row = result.db.query("SELECT v FROM t WHERE k = 'kept'").get() as { v: string };
    expect(row.v).toBe("row");
  });

  test("upgrade: the forward chain runs stepwise and preserves rows", () => {
    const dbPath = path.join(tmpDir(), "upgrade.db");
    seedDatabase(dbPath, 1);
    const applied: number[] = [];
    const result = opened(dbPath, "durable", 3, [
      {
        from: 2,
        migrate: (db) => {
          applied.push(2);
          db.run("ALTER TABLE t ADD COLUMN w TEXT");
        },
      },
      {
        from: 1,
        migrate: () => {
          applied.push(1);
        },
      },
    ]);
    expect(result.outcome).toBe("migrated");
    expect(result.fromVersion).toBe(1);
    expect(applied).toEqual([1, 2]);
    const row = result.db.query("SELECT v, w FROM t WHERE k = 'kept'").get() as {
      v: string;
      w: string | null;
    };
    expect(row.v).toBe("row");
    result.db.close();
    expect(userVersionOf(dbPath)).toBe(3);
  });

  test("downgrade, durable: fails closed with UNSUPPORTED_VERSION, file untouched", () => {
    const dbPath = path.join(tmpDir(), "newer.db");
    seedDatabase(dbPath, 9);
    const before = readFileSync(dbPath);
    let thrown: unknown;
    try {
      opened(dbPath, "durable", 1);
    } catch (error) {
      thrown = error;
    }
    expect(thrown).toBeInstanceOf(UnsupportedVersionError);
    expect((thrown as UnsupportedVersionError).code).toBe("UNSUPPORTED_VERSION");
    expect(readFileSync(dbPath).equals(before)).toBe(true);
  });

  test("downgrade, cache: the newer database is discarded and rebuilt", () => {
    const dbPath = path.join(tmpDir(), "newer-cache.db");
    seedDatabase(dbPath, 9);
    const result = opened(dbPath, "cache", 1);
    expect(result.outcome).toBe("rebuilt");
    expect(result.fromVersion).toBe(9);
    expect(result.notes.some((n) => n.includes("newer"))).toBe(true);
    const count = result.db.query("SELECT count(*) AS n FROM t").get() as { n: number };
    expect(count.n).toBe(0);
    result.db.close();
    expect(userVersionOf(dbPath)).toBe(1);
  });

  test("missing chain, durable: fails closed rather than guessing", () => {
    const dbPath = path.join(tmpDir(), "gap.db");
    seedDatabase(dbPath, 1);
    const before = readFileSync(dbPath);
    expect(() => opened(dbPath, "durable", 3, [{ from: 2, migrate: () => {} }])).toThrow(
      UnsupportedVersionError,
    );
    expect(readFileSync(dbPath).equals(before)).toBe(true);
  });

  test("missing chain, cache: rebuild is always legal", () => {
    const dbPath = path.join(tmpDir(), "gap-cache.db");
    seedDatabase(dbPath, 1);
    const result = opened(dbPath, "cache", 3);
    expect(result.outcome).toBe("rebuilt");
    expect(result.fromVersion).toBe(1);
    expect(userVersionOf(dbPath)).toBe(3);
  });

  test("failed migration, durable: the transaction rolls back and the open refuses", () => {
    const dbPath = path.join(tmpDir(), "failing.db");
    seedDatabase(dbPath, 1);
    expect(() =>
      opened(dbPath, "durable", 2, [
        {
          from: 1,
          migrate: (db) => {
            db.run("DELETE FROM t");
            throw new Error("migration bug");
          },
        },
      ]),
    ).toThrow(UnsupportedVersionError);
    // Rolled back: the row survives at the found version.
    expect(userVersionOf(dbPath)).toBe(1);
    const db = new Database(dbPath, { readonly: true });
    const count = db.query("SELECT count(*) AS n FROM t").get() as { n: number };
    db.close();
    expect(count.n).toBe(1);
  });

  test("unreadable cache database is discarded and rebuilt", () => {
    const dbPath = path.join(tmpDir(), "corrupt-cache.db");
    writeFileSync(dbPath, "this is not a sqlite database, not even close");
    const result = opened(dbPath, "cache", 1);
    expect(result.outcome).toBe("rebuilt");
    expect(result.notes.some((n) => n.includes("unreadable"))).toBe(true);
    result.db.run("INSERT INTO t (k, v) VALUES ('a', 'b')");
  });

  test("unreadable durable database is quarantined intact, never deleted", () => {
    const dir = tmpDir();
    const dbPath = path.join(dir, "corrupt.db");
    const garbage = "garbage bytes standing in for a torn database";
    writeFileSync(dbPath, garbage);
    writeFileSync(`${dbPath}-wal`, "wal sidecar");
    const result = opened(dbPath, "durable", 1);
    expect(result.outcome).toBe("quarantined");
    expect(result.quarantinedTo).toBe(`${dbPath}.quarantined`);
    // Intact: byte-identical, sidecar included.
    expect(readFileSync(`${dbPath}.quarantined`, "utf8")).toBe(garbage);
    expect(readFileSync(`${dbPath}.quarantined-wal`, "utf8")).toBe("wal sidecar");
    expect(result.notes.some((n) => n.includes("quarantined intact"))).toBe(true);
    // The replacement is a working, stamped database.
    result.db.run("INSERT INTO t (k, v) VALUES ('a', 'b')");
    result.db.close();
    expect(userVersionOf(dbPath)).toBe(1);
  });

  test("quarantine names are counter-suffixed, never overwritten", () => {
    const dir = tmpDir();
    const dbPath = path.join(dir, "again.db");
    writeFileSync(`${dbPath}.quarantined`, "first casualty");
    writeFileSync(dbPath, "second casualty");
    const result = opened(dbPath, "durable", 1);
    expect(result.quarantinedTo).toBe(`${dbPath}.quarantined.2`);
    expect(readFileSync(`${dbPath}.quarantined`, "utf8")).toBe("first casualty");
    expect(readFileSync(`${dbPath}.quarantined.2`, "utf8")).toBe("second casualty");
  });
});

describe("workspace.db wiring (durable-class)", () => {
  test("a fresh workspace stamps WORKSPACE_SCHEMA_VERSION", () => {
    const root = tmpDir();
    const workspace = openWorkspace(root, LOCAL_ENV);
    workspace.close();
    expect(userVersionOf(path.join(root, ".workbench", "workspace.db"))).toBe(
      WORKSPACE_SCHEMA_VERSION,
    );
  });

  test("a pre-runner workspace.db (unstamped meta table) migrates in place", () => {
    const root = tmpDir();
    const stateDir = path.join(root, ".workbench");
    mkdirSync(stateDir, { recursive: true });
    const legacy = new Database(path.join(stateDir, "workspace.db"), { create: true });
    legacy.run("CREATE TABLE IF NOT EXISTS meta (key TEXT PRIMARY KEY, value TEXT NOT NULL)");
    legacy.run("INSERT INTO meta (key, value) VALUES ('carried', 'over')");
    legacy.close();
    const workspace = openWorkspace(root, LOCAL_ENV);
    expect(workspace.notes).toHaveLength(0);
    workspace.close();
    const db = new Database(path.join(stateDir, "workspace.db"), { readonly: true });
    const row = db.query("SELECT value FROM meta WHERE key = 'carried'").get() as {
      value: string;
    };
    db.close();
    expect(row.value).toBe("over");
    expect(userVersionOf(path.join(stateDir, "workspace.db"))).toBe(WORKSPACE_SCHEMA_VERSION);
  });

  test("a newer workspace.db refuses to open and releases the lock", () => {
    const root = tmpDir();
    const stateDir = path.join(root, ".workbench");
    mkdirSync(stateDir, { recursive: true });
    const future = new Database(path.join(stateDir, "workspace.db"), { create: true });
    future.run("CREATE TABLE IF NOT EXISTS meta (key TEXT PRIMARY KEY, value TEXT NOT NULL)");
    future.run(`PRAGMA user_version = ${WORKSPACE_SCHEMA_VERSION + 1}`);
    future.close();
    expect(() => openWorkspace(root, LOCAL_ENV)).toThrow(UnsupportedVersionError);
    // The advisory lock was released by the refusal, not leaked.
    expect(existsSync(path.join(stateDir, "server.lock"))).toBe(false);
  });

  test("a corrupt workspace.db is quarantined and the note rides the runtime", () => {
    const root = tmpDir();
    const stateDir = path.join(root, ".workbench");
    mkdirSync(stateDir, { recursive: true });
    const garbage = "torn beyond recognition";
    writeFileSync(path.join(stateDir, "workspace.db"), garbage);
    const workspace = openWorkspace(root, LOCAL_ENV);
    expect(workspace.notes.some((n) => n.includes("quarantined intact"))).toBe(true);
    workspace.close();
    expect(readFileSync(path.join(stateDir, "workspace.db.quarantined"), "utf8")).toBe(garbage);
    expect(userVersionOf(path.join(stateDir, "workspace.db"))).toBe(WORKSPACE_SCHEMA_VERSION);
  });
});

describe("index.db wiring (cache-class)", () => {
  test("a fresh index.db stamps INDEX_SCHEMA_VERSION", () => {
    const root = tmpDir();
    const indexer = new ChildIndexer(root, stubCoreBinding());
    closers.push(() => indexer.close());
    expect(userVersionOf(path.join(root, ".workbench", "index.db"))).toBe(INDEX_SCHEMA_VERSION);
  });

  // specs/009: schema 2 added columns and a table a version-0/1 database does
  // not have, so adopting one in place would leave a silently half-shaped
  // index. Cache-class means the honest answer is rebuild — this test moved
  // from "adopted in place, warm" to "discarded and rebuilt" deliberately.
  test("a pre-runner index.db (meta stamp only) is DISCARDED and rebuilt", () => {
    const root = tmpDir();
    const stateDir = path.join(root, ".workbench");
    mkdirSync(stateDir, { recursive: true });
    const legacy = new Database(path.join(stateDir, "index.db"), { create: true });
    legacy.run("CREATE TABLE meta (key TEXT PRIMARY KEY, value TEXT NOT NULL)");
    legacy.run(
      "CREATE TABLE files (path TEXT PRIMARY KEY, language TEXT, size INTEGER NOT NULL, " +
        "mtime_ms INTEGER NOT NULL, junk INTEGER NOT NULL, content_indexed INTEGER NOT NULL)",
    );
    legacy.run(
      "CREATE TABLE symbols (path TEXT NOT NULL, name TEXT NOT NULL, kind TEXT NOT NULL, " +
        "start_line INTEGER NOT NULL, start_col INTEGER NOT NULL, end_line INTEGER NOT NULL)",
    );
    legacy.run("CREATE TABLE imports (path TEXT NOT NULL, target TEXT NOT NULL)");
    legacy.run("CREATE VIRTUAL TABLE content_fts USING fts5(path UNINDEXED, content)");
    legacy.run("INSERT INTO meta (key, value) VALUES ('schema_version', '1')");
    legacy.run("INSERT INTO meta (key, value) VALUES ('generation', '5')");
    legacy.run("INSERT INTO meta (key, value) VALUES ('tier', '1')");
    legacy.close();
    const indexer = new ChildIndexer(root, stubCoreBinding());
    closers.push(() => indexer.close());
    // Nothing of the old cache survives: no warm generation, no tier, and the
    // file now carries the current schema — a rebuild, not an adoption.
    const status = indexer.status();
    expect(status.generation).toBe(0);
    expect(status.tier).toBe(0);
    expect(userVersionOf(path.join(stateDir, "index.db"))).toBe(INDEX_SCHEMA_VERSION);
    // And the rebuilt file really has the v2 shape.
    const db = new Database(path.join(stateDir, "index.db"), { readonly: true });
    try {
      const columns = (db.query("PRAGMA table_info(symbols)").all() as { name: string }[]).map(
        (c) => c.name,
      );
      expect(columns).toContain("detail");
      expect(columns).toContain("doc");
    } finally {
      db.close();
    }
  });

  test("an unrecognizable index.db at version 0 rebuilds cold", () => {
    const root = tmpDir();
    const stateDir = path.join(root, ".workbench");
    mkdirSync(stateDir, { recursive: true });
    const alien = new Database(path.join(stateDir, "index.db"), { create: true });
    alien.run("CREATE TABLE not_our_schema (x INTEGER)");
    alien.close();
    const indexer = new ChildIndexer(root, stubCoreBinding());
    closers.push(() => indexer.close());
    const status = indexer.status();
    expect(status.tier).toBe(0);
    expect(status.generation).toBe(0);
    expect(userVersionOf(path.join(stateDir, "index.db"))).toBe(INDEX_SCHEMA_VERSION);
  });

  test("a corrupt index.db is discarded and rebuilt", () => {
    const root = tmpDir();
    const stateDir = path.join(root, ".workbench");
    mkdirSync(stateDir, { recursive: true });
    writeFileSync(path.join(stateDir, "index.db"), "not sqlite at all");
    const indexer = new ChildIndexer(root, stubCoreBinding());
    closers.push(() => indexer.close());
    expect(indexer.status().tier).toBe(0);
    expect(userVersionOf(path.join(stateDir, "index.db"))).toBe(INDEX_SCHEMA_VERSION);
  });
});
