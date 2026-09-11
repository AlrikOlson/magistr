/**
 * The durable journal (SDD §7.6, §10.1) — chunk:m2-journal-undo.
 *
 * Pins the acceptance criteria that live at the adapter: the journal is
 * durable-class data (quarantine-and-recover, fail-closed downgrade), blobs
 * are content-addressed zstd under a byte cap with oldest-blob eviction that
 * is never silent, and ops carry the session identity the request supplied.
 */

import { Database } from "bun:sqlite";
import { afterEach, describe, expect, test } from "bun:test";
import { existsSync, mkdtempSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import {
  JOURNAL_SCHEMA_VERSION,
  JournalDb,
  JournalUnreadableError,
  openJournalReader,
} from "../src/adapters/journal-db";
import { openVersionedDatabase, UnsupportedVersionError } from "../src/data/versioned-db";
import type { JournalFileChange } from "../src/ports";

const roots: string[] = [];

function tmpJournalPath(): string {
  const dir = mkdtempSync(join(tmpdir(), "wb-journal-"));
  roots.push(dir);
  return join(dir, "journal.db");
}

afterEach(() => {
  for (const dir of roots.splice(0)) {
    try {
      rmSync(dir, { recursive: true, force: true });
    } catch {
      // Windows holds handles briefly after a failed test; leak the tempdir.
    }
  }
});

function fileChange(overrides: Partial<JournalFileChange> = {}): JournalFileChange {
  return { kind: "write", path: "src/a.ts", before: "1\n", after: "2\n", ...overrides };
}

function op(sessionId: string, files: readonly JournalFileChange[] = [fileChange()]) {
  return { tool: "edit", argsHash: "abc", sessionId, files };
}

describe("append / read round-trip", () => {
  test("ops carry identity, files carry hashed zstd content, seqs are monotonic", async () => {
    const journal = new JournalDb({ path: tmpJournalPath() });
    const first = await journal.append(op("session-a"));
    const second = await journal.append(op("session-b"));
    expect(first.op.seq).toBe(1);
    expect(second.op.seq).toBe(2);
    expect(first.notes).toEqual([]);

    const got = await journal.get(1);
    expect(got?.tool).toBe("edit");
    expect(got?.argsHash).toBe("abc");
    expect(got?.sessionId).toBe("session-a");
    expect(got?.files[0]?.before).toBe("1\n");
    expect(got?.files[0]?.after).toBe("2\n");
    expect(got?.files[0]?.beforeHash).toHaveLength(64);
    expect(await journal.list(2)).toHaveLength(1);
    expect(await journal.get(99)).toBeNull();
    journal.close();
  });

  test("the journal is durable: a reopened database still holds the log", async () => {
    const path = tmpJournalPath();
    const journal = new JournalDb({ path });
    await journal.append(op("s", [fileChange({ path: "x.txt", before: "", after: "hello\n" })]));
    journal.close();

    const reopened = new JournalDb({ path });
    const ops = await reopened.list(0);
    expect(ops).toHaveLength(1);
    expect(ops[0]?.files[0]?.after).toBe("hello\n");
    reopened.close();
  });
});

describe("undo candidates", () => {
  test("session filter, undo-op exclusion, and undone exclusion all apply", async () => {
    const journal = new JournalDb({ path: tmpJournalPath() });
    await journal.append(op("a")); // seq 1
    await journal.append(op("b")); // seq 2
    await journal.append(op("a")); // seq 3
    // seq 4 undoes 3: an undo op, never a candidate itself.
    await journal.append({ ...op("a"), undoOf: [3] });

    const forA = await journal.undoCandidates({ sessionId: "a", limit: 10 });
    expect(forA.map((o) => o.seq)).toEqual([1]);

    const workspaceWide = await journal.undoCandidates({ limit: 10 });
    expect(workspaceWide.map((o) => o.seq)).toEqual([2, 1]);

    const limited = await journal.undoCandidates({ limit: 1 });
    expect(limited.map((o) => o.seq)).toEqual([2]);
    journal.close();
  });
});

describe("the §7.6 size cap", () => {
  test("oldest blobs evict with a note; the evicted op reads null content", async () => {
    // Cap small enough that the second op's blobs push the first one out.
    // Random bytes so zstd cannot compress below the cap.
    const big = () => Buffer.from(crypto.getRandomValues(new Uint8Array(4096))).toString("hex");
    const journal = new JournalDb({ path: tmpJournalPath(), capBytes: 3000 });
    const first = await journal.append(op("s", [fileChange({ before: big(), after: big() })]));
    expect(first.notes).toEqual([]); // its own blobs are never evicted
    const second = await journal.append(op("s", [fileChange({ before: big(), after: big() })]));
    expect(second.notes.join(" ")).toContain("evicted");
    expect(second.notes.join(" ")).toContain("no longer undoable");

    const evictedOp = await journal.get(first.op.seq);
    expect(evictedOp?.files[0]?.before).toBeNull();
    expect(evictedOp?.files[0]?.after).toBeNull();
    // The hashes remain: history stays visible even when content is gone.
    expect(evictedOp?.files[0]?.beforeHash).toHaveLength(64);

    const survivor = await journal.get(second.op.seq);
    expect(survivor?.files[0]?.before).not.toBeNull();
    journal.close();
  });
});

describe("durable-class lifecycle (§10.2)", () => {
  test("v1 text history migrates intact; binary history prevents a lossy v1 downgrade", async () => {
    const path = tmpJournalPath();
    const old = new JournalDb({ path });
    await old.append(op("legacy"));
    old.close();
    // v1 has the identical SQL/blob layout. Only the interpretation widens.
    const raw = new Database(path);
    raw.run("PRAGMA user_version = 1");
    raw.close();
    const reader = openJournalReader(path);
    expect(reader.recent({ limit: 1 })[0]?.op.files[0]?.before).toBe("1\n");
    reader.close();
    const migrated = new JournalDb({ path });
    expect((await migrated.get(1))?.files[0]?.after).toBe("2\n");
    const versionProbe = new Database(path, { readonly: true });
    expect(versionProbe.query("PRAGMA user_version").get()).toEqual({
      user_version: JOURNAL_SCHEMA_VERSION,
    });
    versionProbe.close();
    const bytes = new Uint8Array([0, 255, 128, 10]);
    await migrated.append(op("binary", [fileChange({ before: bytes, after: bytes })]));
    migrated.close();
    expect(() =>
      openVersionedDatabase({
        path,
        dataClass: "durable",
        version: 1,
        bootstrap: () => {
          throw new Error("must not rebuild");
        },
      }),
    ).toThrow(UnsupportedVersionError);
    const reopened = new JournalDb({ path });
    expect((await reopened.get(2))?.files[0]?.before).toEqual(bytes);
    reopened.close();
  });
  test("journal quarantine-and-recover: an unreadable journal is quarantined intact and a fresh one works", async () => {
    const path = tmpJournalPath();
    writeFileSync(path, "this is not a sqlite database — the dog ate my WAL");

    const journal = new JournalDb({ path });
    expect(journal.quarantinedTo).toBeDefined();
    expect(journal.lifecycleNotes.join(" ")).toContain("quarantined");
    // Quarantined INTACT — the unreadable bytes still exist, never deleted.
    expect(existsSync(journal.quarantinedTo as string)).toBe(true);

    // And the fresh journal is fully operational: recover by appending.
    const appended = await journal.append(op("recovered-session"));
    expect(appended.op.seq).toBe(1);
    expect((await journal.get(1))?.sessionId).toBe("recovered-session");
    journal.close();
  });

  test("a newer journal format fails closed with UNSUPPORTED_VERSION, file untouched", () => {
    const path = tmpJournalPath();
    const raw = new Database(path, { create: true });
    raw.run("CREATE TABLE ops(seq INTEGER PRIMARY KEY)");
    raw.run(`PRAGMA user_version = ${JOURNAL_SCHEMA_VERSION + 1}`);
    raw.close();

    expect(() => new JournalDb({ path })).toThrow(UnsupportedVersionError);
    expect(existsSync(path)).toBe(true);
  });
});

describe("the log a reader tails (recent) — chunk:journal-listing-verb", () => {
  test("newest first, bounded, session-filtered, with undo eligibility resolved", async () => {
    const journal = new JournalDb({ path: tmpJournalPath() });
    await journal.append(op("a"));
    await journal.append(op("b"));
    await journal.append({ ...op("a"), tool: "files" });
    // An undo of op 2: op 2 stops being a candidate, the undo-op never is.
    await journal.append({ ...op("b", []), tool: "undo", undoOf: [2] });

    const all = await journal.recent({ limit: 10 });
    expect(all.map((e) => e.op.seq)).toEqual([4, 3, 2, 1]);
    expect(all.map((e) => e.undoable)).toEqual([false, true, false, true]);
    expect(all.find((e) => e.op.seq === 2)?.undoneBy).toBe(4);
    expect(all.find((e) => e.op.seq === 4)?.op.undoOf).toEqual([2]);

    expect((await journal.recent({ limit: 2 })).map((e) => e.op.seq)).toEqual([4, 3]);
    expect((await journal.recent({ limit: 10, sessionId: "a" })).map((e) => e.op.seq)).toEqual([
      3, 1,
    ]);
    expect((await journal.recent({ limit: 10, sinceSeq: 3 })).map((e) => e.op.seq)).toEqual([4, 3]);
    expect((await journal.recent({ limit: 10, beforeSeq: 3 })).map((e) => e.op.seq)).toEqual([
      2, 1,
    ]);
    journal.close();
  });

  test("undoCandidates(seqs) answers named ops, and never waives eligibility", async () => {
    const journal = new JournalDb({ path: tmpJournalPath() });
    await journal.append(op("a"));
    await journal.append(op("a"));
    await journal.append(op("b"));
    await journal.append({ ...op("a", []), tool: "undo", undoOf: [2] });

    const named = await journal.undoCandidates({ limit: 10, seqs: [1, 3] });
    expect(named.map((o) => o.seq)).toEqual([3, 1]);
    // 2 is undone and 4 is the undo itself: neither is a candidate by name.
    expect((await journal.undoCandidates({ limit: 10, seqs: [2, 4] })).length).toBe(0);
    // Session scope still applies to a named op.
    expect(
      (await journal.undoCandidates({ limit: 10, seqs: [1, 3], sessionId: "a" })).map((o) => o.seq),
    ).toEqual([1]);
    expect((await journal.undoCandidates({ limit: 10, seqs: [] })).length).toBe(0);
    journal.close();
  });

  test("openJournalReader reads beside a live writer and refuses a foreign format", async () => {
    const path = tmpJournalPath();
    const journal = new JournalDb({ path });
    await journal.append(op("a"));

    // The writer stays open — this is the beside-a-live-server case.
    const reader = openJournalReader(path);
    expect(reader.recent({ limit: 10 }).map((e) => e.op.seq)).toEqual([1]);
    await journal.append(op("b"));
    expect(reader.recent({ limit: 10 }).map((e) => e.op.seq)).toEqual([2, 1]);
    expect(reader.recent({ limit: 10 })[0]?.op.files[0]?.after).toBe("2\n");
    reader.close();
    journal.close();

    const foreign = tmpJournalPath();
    const raw = new Database(foreign, { create: true });
    raw.run(`PRAGMA user_version = ${JOURNAL_SCHEMA_VERSION + 1}`);
    raw.close();
    expect(() => openJournalReader(foreign)).toThrow(JournalUnreadableError);
  });
});
