/**
 * The durable journal adapter (SDD §7.6, §10.1) — `JournalPort` over
 * `.workbench/journal.db`, the production op log that powers `undo`.
 *
 * Durable-class by construction (§10.2): the file is opened through the
 * versioned-db runner, so forward-only migration at open, quarantine-intact
 * on an unreadable file, and fail-closed `UNSUPPORTED_VERSION` under
 * downgrade are inherited, not re-implemented. Open-time degradation notes
 * are exposed on `lifecycleNotes` for the composing layer to surface.
 *
 * Blobs are content-addressed by the sha256 of the UNCOMPRESSED content and
 * stored zstd-compressed (§10.1), deduplicated across ops. The store carries
 * a per-workspace byte cap (default 256MB of compressed bytes); appends that
 * push past it evict least-recently-referenced blobs — never the appending
 * op's own — and every eviction rides the append result's notes (§7.6:
 * eviction happens "with a note", never silently). An op whose blob was
 * evicted stays in the log but reads its content as `null`: visible history,
 * no longer undoable.
 */

import { Database } from "bun:sqlite";
import { openVersionedDatabase } from "../data/versioned-db";
import { contentBytes, decodeContent, type FileContent } from "../envelope";
import type {
  JournalAppendResult,
  JournalFileKind,
  JournalLogEntry,
  JournalLogQuery,
  JournalOp,
  JournalOpInit,
  JournalPort,
  UndoCandidateQuery,
} from "../ports";
import { PORT_VERSIONS } from "../ports";

// Same blob layout, broader interpretation: v1 readers decode every blob as
// UTF-8. Stamp v2 before writing binary so a downgrade fails closed.
export const JOURNAL_SCHEMA_VERSION = 2;

/** §7.6 default: 256MB of stored (compressed) blob bytes per workspace. */
export const JOURNAL_CAP_BYTES_DEFAULT = 256 * 1024 * 1024;

export interface JournalDbOptions {
  /** Absolute path of the database file (normally `<stateDir>/journal.db`). */
  readonly path: string;
  /** WAL on local filesystems; TRUNCATE when detection says network FS. */
  readonly journalMode?: "WAL" | "TRUNCATE";
  /** Test seam; production uses the §7.6 default. */
  readonly capBytes?: number;
}

function bootstrap(db: Database): void {
  db.run(`CREATE TABLE ops(
    seq INTEGER PRIMARY KEY AUTOINCREMENT,
    at INTEGER NOT NULL,
    tool TEXT NOT NULL,
    args_hash TEXT NOT NULL,
    session_id TEXT NOT NULL,
    is_undo INTEGER NOT NULL DEFAULT 0)`);
  db.run("CREATE INDEX ops_session ON ops(session_id, seq)");
  db.run(`CREATE TABLE op_files(
    op_seq INTEGER NOT NULL REFERENCES ops(seq),
    ord INTEGER NOT NULL,
    kind TEXT NOT NULL CHECK(kind IN ('create','write','delete')),
    path TEXT NOT NULL,
    before_hash TEXT NOT NULL,
    after_hash TEXT NOT NULL,
    PRIMARY KEY(op_seq, ord))`);
  db.run(`CREATE TABLE blobs(
    hash TEXT PRIMARY KEY,
    zstd BLOB NOT NULL,
    bytes INTEGER NOT NULL,
    last_ref_seq INTEGER NOT NULL)`);
  db.run(`CREATE TABLE undone(
    op_seq INTEGER PRIMARY KEY,
    by_seq INTEGER NOT NULL)`);
}

function sha256Hex(content: FileContent): string {
  return new Bun.CryptoHasher("sha256").update(content).digest("hex");
}

interface OpRow {
  seq: number;
  at: number;
  tool: string;
  args_hash: string;
  session_id: string;
  is_undo: number;
}

interface FileRow {
  kind: string;
  path: string;
  before_hash: string;
  after_hash: string;
}

export class JournalDb implements JournalPort {
  readonly version = PORT_VERSIONS.journal;
  /** Open-time §10.2 degradations (quarantine, migration) for the envelope. */
  readonly lifecycleNotes: readonly string[];
  /** Where the unreadable predecessor went, when one did. */
  readonly quarantinedTo: string | undefined;
  private readonly db: Database;
  private readonly capBytes: number;

  constructor(options: JournalDbOptions) {
    const opened = openVersionedDatabase({
      path: options.path,
      dataClass: "durable",
      version: JOURNAL_SCHEMA_VERSION,
      migrations: [
        {
          from: 1,
          migrate: () => {
            /* v1 text blobs already contain exact UTF-8 bytes. */
          },
        },
      ],
      ...(options.journalMode === undefined ? {} : { journalMode: options.journalMode }),
      bootstrap,
    });
    this.db = opened.db;
    this.lifecycleNotes = opened.notes;
    this.quarantinedTo = opened.quarantinedTo;
    this.capBytes = options.capBytes ?? JOURNAL_CAP_BYTES_DEFAULT;
  }

  close(): void {
    this.db.close();
  }

  append(op: JournalOpInit): Promise<JournalAppendResult> {
    const notes: string[] = [];
    const entry = this.db.transaction((): JournalOp => {
      const at = Date.now();
      const isUndo = op.undoOf !== undefined && op.undoOf.length > 0;
      const seq = Number(
        (
          this.db
            .query(
              "INSERT INTO ops(at, tool, args_hash, session_id, is_undo) VALUES (?, ?, ?, ?, ?) RETURNING seq",
            )
            .get(at, op.tool, op.argsHash, op.sessionId, isUndo ? 1 : 0) as { seq: number }
        ).seq,
      );
      for (const undone of op.undoOf ?? []) {
        this.db.run("INSERT OR IGNORE INTO undone(op_seq, by_seq) VALUES (?, ?)", [undone, seq]);
      }
      const ownHashes = new Set<string>();
      op.files.forEach((file, ord) => {
        const beforeHash = this.storeBlob(file.before, seq);
        const afterHash = this.storeBlob(file.after, seq);
        ownHashes.add(beforeHash).add(afterHash);
        this.db.run(
          "INSERT INTO op_files(op_seq, ord, kind, path, before_hash, after_hash) VALUES (?, ?, ?, ?, ?, ?)",
          [seq, ord, file.kind, file.path, beforeHash, afterHash],
        );
      });
      this.evictOverCap(ownHashes, notes);
      return this.mustGet(seq);
    })();
    return Promise.resolve({ op: entry, notes });
  }

  list(sinceSeq: number): Promise<readonly JournalOp[]> {
    const rows = this.db
      .query("SELECT * FROM ops WHERE seq >= ? ORDER BY seq ASC")
      .all(sinceSeq) as OpRow[];
    return Promise.resolve(rows.map((row) => this.hydrate(row)));
  }

  get(seq: number): Promise<JournalOp | null> {
    const row = this.db.query("SELECT * FROM ops WHERE seq = ?").get(seq) as OpRow | null;
    return Promise.resolve(row === null ? null : this.hydrate(row));
  }

  undoCandidates(query: UndoCandidateQuery): Promise<readonly JournalOp[]> {
    // Eligibility is one predicate, whichever way the query is narrowed: an
    // op named by seq gets no dispensation from it (§7.6 — an undo-op and an
    // already-undone op are never candidates, and there is no redo).
    const where = ["is_undo = 0", "seq NOT IN (SELECT op_seq FROM undone)"];
    const params: (string | number)[] = [];
    if (query.sessionId !== undefined) {
      where.push("session_id = ?");
      params.push(query.sessionId);
    }
    if (query.seqs !== undefined) {
      if (query.seqs.length === 0) return Promise.resolve([]);
      where.push(`seq IN (${query.seqs.map(() => "?").join(", ")})`);
      params.push(...query.seqs);
    }
    params.push(query.limit);
    const rows = this.db
      .query(`SELECT * FROM ops WHERE ${where.join(" AND ")} ORDER BY seq DESC LIMIT ?`)
      .all(...params) as OpRow[];
    return Promise.resolve(rows.map((row) => this.hydrate(row)));
  }

  recent(query: JournalLogQuery): Promise<readonly JournalLogEntry[]> {
    return Promise.resolve(recentEntries(this.db, query));
  }

  /** Store one blob (dedup by content hash), bump its reference recency. */
  private storeBlob(content: FileContent, refSeq: number): string {
    const hash = sha256Hex(content);
    const compressed = Bun.zstdCompressSync(contentBytes(content));
    const inserted = this.db
      .query("INSERT OR IGNORE INTO blobs(hash, zstd, bytes, last_ref_seq) VALUES (?, ?, ?, ?)")
      .run(hash, compressed, compressed.byteLength, refSeq);
    if (inserted.changes === 0) {
      this.db.run("UPDATE blobs SET last_ref_seq = ? WHERE hash = ? AND last_ref_seq < ?", [
        refSeq,
        hash,
        refSeq,
      ]);
    }
    return hash;
  }

  /** §7.6 cap: evict least-recently-referenced blobs until under the cap,
   * never touching the appending op's own blobs, and never silently. */
  private evictOverCap(protect: ReadonlySet<string>, notes: string[]): void {
    const total = (
      this.db.query("SELECT COALESCE(SUM(bytes), 0) AS total FROM blobs").get() as {
        total: number;
      }
    ).total;
    if (total <= this.capBytes) return;
    let freed = 0;
    let evicted = 0;
    const oldest = this.db
      .query("SELECT hash, bytes FROM blobs ORDER BY last_ref_seq ASC, hash ASC")
      .all() as { hash: string; bytes: number }[];
    for (const blob of oldest) {
      if (total - freed <= this.capBytes) break;
      if (protect.has(blob.hash)) continue;
      this.db.run("DELETE FROM blobs WHERE hash = ?", [blob.hash]);
      freed += blob.bytes;
      evicted += 1;
    }
    if (evicted > 0) {
      notes.push(
        `journal: evicted ${evicted} oldest blob(s) (${freed} compressed bytes) to stay ` +
          `under the ${this.capBytes}-byte cap; the ops that referenced them are no longer undoable`,
      );
    }
  }

  private hydrate(row: OpRow): JournalOp {
    return hydrateOp(this.db, row);
  }

  private mustGet(seq: number): JournalOp {
    const row = this.db.query("SELECT * FROM ops WHERE seq = ?").get(seq) as OpRow | null;
    if (row === null) throw new Error(`journal: op ${seq} vanished inside its own transaction`);
    return this.hydrate(row);
  }
}

/**
 * The read side, as free functions over a `Database` — so the lock-free
 * reader below answers with exactly the same SQL the served port does,
 * rather than a second dialect that could drift from it.
 */
function readBlobFrom(db: Database, hash: string): FileContent | null {
  const row = db.query("SELECT zstd FROM blobs WHERE hash = ?").get(hash) as {
    zstd: Uint8Array;
  } | null;
  if (row === null) return null;
  return decodeContent(Bun.zstdDecompressSync(row.zstd));
}

function hydrateOp(db: Database, row: OpRow): JournalOp {
  const files = db
    .query(
      "SELECT kind, path, before_hash, after_hash FROM op_files WHERE op_seq = ? ORDER BY ord ASC",
    )
    .all(row.seq) as FileRow[];
  const undoOf = (
    db.query("SELECT op_seq FROM undone WHERE by_seq = ? ORDER BY op_seq ASC").all(row.seq) as {
      op_seq: number;
    }[]
  ).map((r) => r.op_seq);
  return {
    seq: row.seq,
    at: row.at,
    tool: row.tool,
    argsHash: row.args_hash,
    sessionId: row.session_id,
    ...(row.is_undo === 1 ? { undoOf } : {}),
    files: files.map((file) => ({
      kind: file.kind as JournalFileKind,
      path: file.path,
      beforeHash: file.before_hash,
      afterHash: file.after_hash,
      before: readBlobFrom(db, file.before_hash),
      after: readBlobFrom(db, file.after_hash),
    })),
  };
}

function recentEntries(db: Database, query: JournalLogQuery): readonly JournalLogEntry[] {
  const where: string[] = [];
  const params: number[] = [];
  if (query.sinceSeq !== undefined && query.sinceSeq > 0) {
    where.push("seq >= ?");
    params.push(query.sinceSeq);
  }
  if (query.beforeSeq !== undefined) {
    where.push("seq < ?");
    params.push(query.beforeSeq);
  }
  const sessionId = query.sessionId;
  const clause = [...where, ...(sessionId === undefined ? [] : ["session_id = ?"])];
  const bound: (string | number)[] = [...params];
  if (sessionId !== undefined) bound.push(sessionId);
  bound.push(query.limit);
  const rows = db
    .query(
      `SELECT * FROM ops${clause.length === 0 ? "" : ` WHERE ${clause.join(" AND ")}`} ` +
        "ORDER BY seq DESC LIMIT ?",
    )
    .all(...bound) as OpRow[];
  return rows.map((row) => {
    const undoneBy =
      (
        db.query("SELECT by_seq FROM undone WHERE op_seq = ?").get(row.seq) as {
          by_seq: number;
        } | null
      )?.by_seq ?? null;
    return {
      op: hydrateOp(db, row),
      undoable: row.is_undo === 0 && undoneBy === null,
      undoneBy,
    };
  });
}

/** A journal opened for reading only: no lock, no PRAGMA, no migration. */
export interface JournalReader {
  recent(query: JournalLogQuery): readonly JournalLogEntry[];
  close(): void;
}

/** Thrown when the file exists but is not a journal this binary can read. */
export class JournalUnreadableError extends Error {
  constructor(
    readonly path: string,
    reason: string,
  ) {
    super(`journal at ${path} cannot be read: ${reason}`);
    this.name = "JournalUnreadableError";
  }
}

/**
 * Open `.workbench/journal.db` read-only, beside a live server (SDD §11.2's
 * precedent: `doctor` probes the shipped artifacts without taking the
 * workspace lock). SQLite readers do not contend with the server's writer
 * under WAL, and nothing here writes — no journal-mode PRAGMA, no migration,
 * no stamping — so a reader can never rewrite the header of a database the
 * server owns. A file stamped by a newer binary is refused rather than
 * guessed at, exactly like the read-write path (§10.2).
 */
export function openJournalReader(path: string): JournalReader {
  const db = new Database(path, { readonly: true });
  let version: number;
  try {
    version =
      (db.query("PRAGMA user_version").get() as { user_version: number } | null)?.user_version ?? 0;
  } catch (error) {
    db.close();
    throw new JournalUnreadableError(path, (error as Error).message);
  }
  if (version < 1 || version > JOURNAL_SCHEMA_VERSION) {
    db.close();
    throw new JournalUnreadableError(
      path,
      `schema version ${version}, this binary reads ${JOURNAL_SCHEMA_VERSION}`,
    );
  }
  return {
    recent: (query) => recentEntries(db, query),
    close: () => {
      db.close();
    },
  };
}
