/** Real SQLite fixtures from the pinned journal, without an addon or server. */
import assert from "node:assert/strict";
import { Database } from "bun:sqlite";
import { mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { JournalDb, openJournalReader } from "../reference/src/adapters/journal-db";
import { contentBytes } from "../reference/src/envelope/file-content";
import type { JournalFileChange, JournalOp } from "../reference/src/ports";

function normalize(op: JournalOp) {
  return {
    seq: op.seq, at: op.at, tool: op.tool, args_hash: op.argsHash,
    session_id: op.sessionId, undo_of: op.undoOf ?? null,
    files: op.files.map(file => ({
      kind: file.kind, path: file.path, before_hash: file.beforeHash, after_hash: file.afterHash,
      before: file.before === null ? null : Array.from(contentBytes(file.before)),
      after: file.after === null ? null : Array.from(contentBytes(file.after)),
    })),
  };
}

function snapshot(path: string) {
  const reader = openJournalReader(path);
  try {
    return reader.recent({ limit: 100 }).map(entry => ({
      op: normalize(entry.op), undoable: entry.undoable, undone_by: entry.undoneBy,
    }));
  } finally { reader.close(); }
}

function tables(path: string) {
  const db = new Database(path, { readonly: true });
  try {
    return {
      ops: db.query("SELECT * FROM ops ORDER BY seq").all(),
      op_files: db.query("SELECT * FROM op_files ORDER BY op_seq, ord").all(),
      undone: db.query("SELECT * FROM undone ORDER BY op_seq").all(),
      blobs: (db.query("SELECT * FROM blobs ORDER BY hash").all() as {
        hash: string, zstd: Uint8Array, bytes: number, last_ref_seq: number,
      }[]).map(blob => ({ ...blob, zstd: Buffer.from(blob.zstd).toString("base64") })),
    };
  } finally { db.close(); }
}

const change = (kind: JournalFileChange["kind"], path: string,
  before: JournalFileChange["before"], after: JournalFileChange["after"]): JournalFileChange =>
  ({ kind, path, before, after });

async function generate(version: number) {
  const root = mkdtempSync(join(tmpdir(), "magistr-journal-fixture-"));
  const path = join(root, "journal.db");
  const originalNow = Date.now;
  let clock = 1_750_000_000_000;
  Date.now = () => clock++;
  try {
    const journal = new JournalDb({ path, journalMode: "TRUNCATE" });
    const append = (sessionId: string, files: readonly JournalFileChange[], undoOf?: number[]) =>
      journal.append({ tool: undoOf ? "undo" : "files", argsHash: `fixture-${clock}`,
        sessionId, files, ...(undoOf ? { undoOf } : {}) });
    await append("alpha", [change("create", "zeta.txt", "", "seed\n"),
      change("create", "alpha.txt", "", "\uFEFFother\n")]);
    await append("beta", [change("write", "zeta.txt", "seed\n", "second\n")]);
    await append("alpha", [change("delete", "alpha.txt", "\uFEFFother\n", "")]);
    await append("alpha", [change("create", "alpha.txt", "", "\uFEFFother\n")], [3]);
    await append("", []);
    if (version === 2) {
      const binary = new Uint8Array([0, 255, 128, 10, ...Array.from({ length: 256 }, (_, i) => i)]);
      await append("alpha", [change("create", "opaque.bin", "", binary)]);
      await append("alpha", [change("write", "zeta.txt", "second\n", new Uint8Array([255, 128, 10]))]);
      await append("alpha", [change("write", "zeta.txt", new Uint8Array([255, 128, 10]), "final\n")]);
      await append("beta", [], [2]);
    }
    journal.close();
    if (version === 1) {
      // The pinned v1 migration test uses this exact production fixture method:
      // v1 and v2 SQL/blob layouts match; v2 broadens byte interpretation.
      const db = new Database(path);
      db.run("PRAGMA user_version = 1");
      db.close();
    }
    return { name: `reference-v${version}`, schema_version: version,
      database_zstd_base64: Buffer.from(Bun.zstdCompressSync(readFileSync(path))).toString("base64"),
      recent: snapshot(path), tables: tables(path) };
  } finally {
    Date.now = originalNow;
    rmSync(root, { recursive: true, force: true });
  }
}

if (process.argv[2] === "--emit") {
  process.stdout.write(`${JSON.stringify({ format: 1, fixtures: [await generate(1), await generate(2)] }, null, 2)}\n`);
} else if (process.argv[2] === "--read") {
  assert.ok(process.argv[3], "--read needs an existing database path");
  process.stdout.write(`${JSON.stringify(snapshot(process.argv[3]))}\n`);
} else {
  const corpus = JSON.parse(readFileSync(join(import.meta.dir, "journal.json"), "utf8"));
  for (const fixture of corpus.fixtures) {
    const root = mkdtempSync(join(tmpdir(), "magistr-journal-reference-"));
    const path = join(root, "journal.db");
    try {
      const bytes = Bun.zstdDecompressSync(Buffer.from(fixture.database_zstd_base64, "base64"));
      writeFileSync(path, bytes);
      assert.deepEqual(snapshot(path), fixture.recent, fixture.name);
      assert.deepEqual(tables(path), fixture.tables, fixture.name);
      assert.deepEqual(readFileSync(path), Buffer.from(bytes), "reader preserves database bytes");
      const writer = new JournalDb({ path, journalMode: "TRUNCATE" });
      writer.close();
      assert.deepEqual(snapshot(path), fixture.recent, `${fixture.name}: migrated records`);
      assert.deepEqual(tables(path), fixture.tables, `${fixture.name}: compressed blobs unchanged`);
    } finally { rmSync(root, { recursive: true, force: true }); }
  }
  process.stdout.write(`Reference journal conformance passed (${corpus.fixtures.length} SQLite fixtures).\n`);
}
