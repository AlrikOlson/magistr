/**
 * `workbench-mcp journal` (SDD §9.8) — chunk:journal-listing-verb.
 *
 * The listing a cockpit reads while an agent owns the workspace: read-only,
 * no lock taken, the same rows `undo action:"list"` returns. What is pinned
 * here is exactly that — the lock is not contended, nothing under
 * `.workbench` is rewritten, and the JSON is one envelope.
 */

import { Database } from "bun:sqlite";
import { afterEach, describe, expect, test } from "bun:test";
import {
  existsSync,
  mkdirSync,
  mkdtempSync,
  readdirSync,
  readFileSync,
  rmSync,
  statSync,
  writeFileSync,
} from "node:fs";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";
import { JournalDb } from "../src/adapters/journal-db";
import type { JournalRow } from "../src/tools/journal-view";

const MAIN = resolve(import.meta.dir, "../src/main.ts");
const roots: string[] = [];

afterEach(() => {
  for (const dir of roots.splice(0)) {
    try {
      rmSync(dir, { recursive: true, force: true });
    } catch {
      // Windows holds handles briefly after a failed test; leak the tempdir.
    }
  }
});

/** A workspace with a journal holding two ops, the second an edit of the first. */
async function seeded(): Promise<string> {
  const root = mkdtempSync(join(tmpdir(), "wb-journal-cli-"));
  roots.push(root);
  mkdirSync(join(root, ".workbench"), { recursive: true });
  const journal = new JournalDb({ path: join(root, ".workbench", "journal.db") });
  await journal.append({
    tool: "edit",
    argsHash: "h1",
    sessionId: "alice",
    files: [{ kind: "create", path: "src/a.ts", before: "", after: "const a = 1\n" }],
  });
  await journal.append({
    tool: "files",
    argsHash: "h2",
    sessionId: "bob",
    files: [{ kind: "write", path: "src/a.ts", before: "const a = 1\n", after: "const a = 2\n" }],
  });
  journal.close();
  return root;
}

function cli(root: string, args: string[]) {
  return Bun.spawnSync([process.execPath, MAIN, "journal", root, ...args], {
    stdout: "pipe",
    stderr: "pipe",
  });
}

/**
 * Hashes of everything under `.workbench` except the WAL shared-memory index
 * (`-shm`): SQLite mutates that sidecar for any reader, read-only opens
 * included, and it holds no durable state. The database and its WAL are what
 * "nothing was rewritten" has to mean.
 */
function stateSnapshot(root: string): Map<string, string> {
  const stateDir = join(root, ".workbench");
  const out = new Map<string, string>();
  for (const name of readdirSync(stateDir)) {
    const full = join(stateDir, name);
    if (name.endsWith("-shm")) continue;
    if (statSync(full).isFile()) out.set(name, Bun.hash(readFileSync(full)).toString());
  }
  return out;
}

describe("journal CLI", () => {
  test("prints the log as one envelope of rows, newest op first", async () => {
    const root = await seeded();
    const run = cli(root, ["--json"]);
    expect(run.exitCode).toBe(0);
    const envelope = JSON.parse(run.stdout.toString()) as {
      ok: boolean;
      results: JournalRow[];
    };
    expect(envelope.ok).toBe(true);
    expect(envelope.results.map((r) => r.seq)).toEqual([2, 1]);
    const newest = envelope.results[0] as JournalRow;
    expect(newest.tool).toBe("files");
    expect(newest.sessionId).toBe("bob");
    expect(newest.path).toBe("src/a.ts");
    expect(newest.added).toBe(1);
    expect(newest.removed).toBe(1);
    expect(newest.undoable).toBe(true);
    expect(newest.diff).toContain("+const a = 2");
  });

  test("runs beside a live server: the lock is neither taken nor disturbed", async () => {
    const root = await seeded();
    // A live instance owns the workspace, exactly as during `magistr run`.
    const lock = join(root, ".workbench", "server.lock");
    writeFileSync(lock, JSON.stringify({ pid: process.pid, acquiredAtMs: Date.now() }));
    // Keep the writer open as a real server does. A fabricated lock beside a
    // closed WAL database lets SQLite create an empty -wal on a read-only
    // open (Windows), which did not test the promised live-writer case.
    const writer = new JournalDb({ path: join(root, ".workbench", "journal.db") });
    try {
      const before = stateSnapshot(root);
      const run = cli(root, ["--json"]);
      expect(run.exitCode).toBe(0);
      expect((JSON.parse(run.stdout.toString()) as { results: unknown[] }).results.length).toBe(2);
      expect(existsSync(lock)).toBe(true);
      expect(stateSnapshot(root)).toEqual(before);
    } finally {
      writer.close();
    }
  });

  test("--limit, --before, --session and --no-diff narrow the listing", async () => {
    const root = await seeded();
    const rows = (flags: string[]) =>
      (JSON.parse(cli(root, ["--json", ...flags]).stdout.toString()) as { results: JournalRow[] })
        .results;
    expect(rows(["--limit", "1"]).map((r) => r.seq)).toEqual([2]);
    expect(rows(["--before", "2"]).map((r) => r.seq)).toEqual([1]);
    expect(rows(["--since", "2"]).map((r) => r.seq)).toEqual([2]);
    expect(rows(["--session", "alice"]).map((r) => r.seq)).toEqual([1]);
    expect(rows(["--no-diff"])[0]?.diff).toBeUndefined();
    expect(rows(["--no-diff"])[0]?.added).toBe(1);
  });

  test("the human rendering names the op, the file and the counts", async () => {
    const root = await seeded();
    const run = cli(root, ["--no-diff"]);
    expect(run.exitCode).toBe(0);
    const out = run.stdout.toString();
    expect(out).toContain("#2");
    expect(out).toContain("files");
    expect(out).toContain("session=bob");
    expect(out).toContain("M src/a.ts  +1 -1");
    expect(out).toContain("A src/a.ts  +1 -0");
  });

  test("a workspace that never wrote says so; a bad flag and a missing root are usage errors", () => {
    const empty = mkdtempSync(join(tmpdir(), "wb-journal-cli-"));
    roots.push(empty);
    const fresh = cli(empty, ["--json"]);
    expect(fresh.exitCode).toBe(0);
    expect((JSON.parse(fresh.stdout.toString()) as { results: unknown[] }).results).toEqual([]);

    expect(cli(empty, ["--nope"]).exitCode).toBe(64);
    expect(cli(empty, ["--limit", "0"]).exitCode).toBe(64);
    expect(cli(join(empty, "missing"), ["--json"]).exitCode).toBe(66);
  });

  test("a journal written by a newer binary fails closed, file untouched", () => {
    const root = mkdtempSync(join(tmpdir(), "wb-journal-cli-"));
    roots.push(root);
    mkdirSync(join(root, ".workbench"), { recursive: true });
    const path = join(root, ".workbench", "journal.db");
    const raw = new Database(path, { create: true });
    raw.run("PRAGMA user_version = 999");
    raw.close();
    const before = readFileSync(path);

    const run = cli(root, ["--json"]);
    expect(run.exitCode).toBe(65);
    expect(run.stderr.toString()).toContain("cannot be read");
    expect(readFileSync(path)).toEqual(before);
  });
});
