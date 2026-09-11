/**
 * `undo` (SDD §9.8, §7.6) — chunk:m2-journal-undo.
 *
 * Pins the acceptance criteria that live at the tool: session-scoped undo
 * never reverts another session's ops by default, and a conflict surfaces a
 * three-way choice rather than clobbering. Every restoration runs through
 * the §7.5 write engine (constitution III), unformatted (exact bytes).
 */

import { afterEach, describe, expect, test } from "bun:test";
import { mkdtempSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { createInProcessPorts, type InProcessPorts } from "../src/adapters/inprocess";
import { JournalDb } from "../src/adapters/journal-db";
import type { ResponseEnvelope } from "../src/envelope";
import type { JournalRow } from "../src/tools/journal-view";
import { runUndo, type UndoInput, type UndoRow } from "../src/tools/undo";
import {
  runWriteBatch,
  type WriteEngineDeps,
  type WriteWorkspaceAccess,
} from "../src/tools/write-engine";

function sha256(content: string): string {
  return new Bun.CryptoHasher("sha256").update(content).digest("hex");
}

function enoent(path: string): NodeJS.ErrnoException {
  const error = new Error(`ENOENT: no such file: ${path}`) as NodeJS.ErrnoException;
  error.code = "ENOENT";
  return error;
}

function accessOf(files: Map<string, string>): WriteWorkspaceAccess {
  return {
    read: (path) =>
      files.has(path) ? Promise.resolve(files.get(path) as string) : Promise.reject(enoent(path)),
    write: (path, content) => {
      files.set(path, content);
      return Promise.resolve();
    },
    remove: (path) => {
      files.delete(path);
      return Promise.resolve();
    },
  };
}

function makeDeps(seed: Record<string, string>): { deps: WriteEngineDeps; ports: InProcessPorts } {
  const ports = createInProcessPorts();
  for (const [path, content] of Object.entries(seed)) {
    ports.workspace.files.set(path, content);
  }
  const deps: WriteEngineDeps = {
    engine: ports.engine,
    formatter: ports.formatter,
    journal: ports.journal,
    workspace: accessOf(ports.workspace.files),
  };
  return { deps, ports };
}

/** Apply one write as `sessionId` through the choke point, like a tool would. */
async function writeAs(
  deps: WriteEngineDeps,
  sessionId: string,
  path: string,
  content: string | null,
): Promise<void> {
  const envelope = await runWriteBatch(
    deps,
    {
      writes: [{ path, content }],
      origin: { tool: "edit", argsHash: "test", sessionId },
    },
    1,
  );
  if (!envelope.ok) throw new Error(`seed write refused: ${envelope.error.code}`);
}

function ok(envelope: ResponseEnvelope<UndoRow>): {
  results: readonly UndoRow[];
  notes: readonly string[];
} {
  if (!envelope.ok) throw new Error(`expected ok envelope, got ${envelope.error.code}`);
  return { results: envelope.results, notes: envelope.notes ?? [] };
}

function undoAs(
  deps: WriteEngineDeps,
  sessionId: string,
  input: UndoInput = {},
): Promise<ResponseEnvelope<UndoRow>> {
  return runUndo(deps, input, 1, sessionId);
}

describe("session scoping (§7.6)", () => {
  test("undo(1) reverts the calling session's latest op and journals the undo", async () => {
    const { deps, ports } = makeDeps({ "src/a.ts": "const a = 1\n" });
    await writeAs(deps, "alice", "src/a.ts", "const a = 2\n");

    const { results, notes } = ok(await undoAs(deps, "alice"));
    expect(ports.workspace.files.get("src/a.ts")).toBe("const a = 1\n");
    expect(results.some((r) => r.kind === "write")).toBe(true);
    expect(notes.join(" ")).toContain("reverted op(s)");

    const log = await ports.journal.list(0);
    expect(log).toHaveLength(2);
    expect(log[1]?.tool).toBe("undo");
    expect(log[1]?.undoOf).toEqual([1]);
    expect(log[1]?.sessionId).toBe("alice");
  });

  test("session-scoped undo never reverts another session's ops by default", async () => {
    const { deps, ports } = makeDeps({});
    await writeAs(deps, "alice", "src/new.ts", "const fresh = 1\n");

    // Bob's default-scope undo finds nothing of Bob's — Alice's op survives.
    const { results, notes } = ok(await undoAs(deps, "bob"));
    expect(results).toHaveLength(0);
    expect(notes.join(" ")).toContain("nothing to undo in session scope");
    expect(ports.workspace.files.get("src/new.ts")).toBe("const fresh = 1\n");

    // Workspace scope is the explicit, deliberate cross-session undo.
    ok(await undoAs(deps, "bob", { scope: "workspace" }));
    expect(ports.workspace.files.has("src/new.ts")).toBe(false);
  });

  test("an anonymous caller in session scope is warned about the missing identity", async () => {
    const { deps } = makeDeps({});
    const { notes } = ok(await undoAs(deps, ""));
    expect(notes.join(" ")).toContain("workbench/sessionId");
  });
});

describe("rewind semantics", () => {
  test("undo of a create deletes; a second undo is not offered (no redo)", async () => {
    const { deps, ports } = makeDeps({});
    await writeAs(deps, "s", "src/made.ts", "const m = 1\n");
    ok(await undoAs(deps, "s"));
    expect(ports.workspace.files.has("src/made.ts")).toBe(false);

    const { results, notes } = ok(await undoAs(deps, "s"));
    expect(results).toHaveLength(0);
    expect(notes.join(" ")).toContain("nothing to undo");
  });

  test("undo of a delete recreates the file with its journaled bytes", async () => {
    const { deps, ports } = makeDeps({ "src/gone.ts": "const g = 1\n" });
    await writeAs(deps, "s", "src/gone.ts", null);
    expect(ports.workspace.files.has("src/gone.ts")).toBe(false);

    ok(await undoAs(deps, "s"));
    expect(ports.workspace.files.get("src/gone.ts")).toBe("const g = 1\n");
  });

  test("undo(steps: 2) threads content through both ops back to the origin", async () => {
    const { deps, ports } = makeDeps({ "src/a.ts": "v1\n" });
    await writeAs(deps, "s", "src/a.ts", "v2\n");
    await writeAs(deps, "s", "src/a.ts", "v3\n");

    const { notes } = ok(await undoAs(deps, "s", { steps: 2 }));
    expect(ports.workspace.files.get("src/a.ts")).toBe("v1\n");
    expect(notes.join(" ")).toContain("reverted op(s)");

    const log = await ports.journal.list(0);
    const undoOp = log[log.length - 1];
    expect([...(undoOp?.undoOf ?? [])].sort((a, b) => a - b)).toEqual([1, 2]);
  });
});

describe("conflict surfacing (§7.6 three-way choice)", () => {
  test("a conflicting undo surfaces the three-way choice rather than clobbering", async () => {
    const { deps, ports } = makeDeps({ "src/a.ts": "v1\n" });
    await writeAs(deps, "s", "src/a.ts", "v2\n");
    // An outside change the journal knows nothing about.
    ports.workspace.files.set("src/a.ts", "v3\n");

    const { results, notes } = ok(await undoAs(deps, "s"));
    const conflict = results.find((r) => r.kind === "conflict");
    expect(conflict).toBeDefined();
    if (conflict?.kind !== "conflict") throw new Error("unreachable");
    expect(conflict.path).toBe("src/a.ts");
    expect(conflict.expectedHash).toBe(sha256("v2\n"));
    expect(conflict.currentHash).toBe(sha256("v3\n"));
    expect(conflict.restoreHash).toBe(sha256("v1\n"));

    // Nothing was applied, nothing consumed: the file and the candidate remain.
    expect(ports.workspace.files.get("src/a.ts")).toBe("v3\n");
    expect(await ports.journal.list(0)).toHaveLength(1);
    const joined = notes.join(" ");
    expect(joined).toContain("keepCurrent");
    expect(joined).toContain("force");
    expect(joined).toContain("merge by hand");
  });

  test("onConflict force restores the journaled version over the outside change", async () => {
    const { deps, ports } = makeDeps({ "src/a.ts": "v1\n" });
    await writeAs(deps, "s", "src/a.ts", "v2\n");
    ports.workspace.files.set("src/a.ts", "v3\n");

    ok(await undoAs(deps, "s", { onConflict: "force" }));
    expect(ports.workspace.files.get("src/a.ts")).toBe("v1\n");
    // The op is consumed: nothing left to undo.
    const again = ok(await undoAs(deps, "s"));
    expect(again.notes.join(" ")).toContain("nothing to undo");
  });

  test("onConflict keepCurrent leaves the conflicted file and consumes the choice", async () => {
    const { deps, ports } = makeDeps({ "src/a.ts": "v1\n" });
    await writeAs(deps, "s", "src/a.ts", "v2\n");
    ports.workspace.files.set("src/a.ts", "v3\n");

    const { results } = ok(await undoAs(deps, "s", { onConflict: "keepCurrent" }));
    expect(results.find((r) => r.kind === "kept")).toBeDefined();
    expect(ports.workspace.files.get("src/a.ts")).toBe("v3\n");

    // The choice was consumed — the op stops being a candidate.
    const again = ok(await undoAs(deps, "s"));
    expect(again.notes.join(" ")).toContain("nothing to undo");
  });

  test("keepCurrent undoes the clean files of the same op while keeping the conflicted one", async () => {
    const { deps, ports } = makeDeps({ "src/a.ts": "a1\n", "src/b.ts": "b1\n" });
    const envelope = await runWriteBatch(
      deps,
      {
        writes: [
          { path: "src/a.ts", content: "a2\n" },
          { path: "src/b.ts", content: "b2\n" },
        ],
        origin: { tool: "edit", argsHash: "t", sessionId: "s" },
      },
      1,
    );
    if (!envelope.ok) throw new Error("seed batch refused");
    ports.workspace.files.set("src/b.ts", "outside\n");

    ok(await undoAs(deps, "s", { onConflict: "keepCurrent" }));
    expect(ports.workspace.files.get("src/a.ts")).toBe("a1\n");
    expect(ports.workspace.files.get("src/b.ts")).toBe("outside\n");
  });
});

describe("eviction (§7.6 cap) at the undo surface", () => {
  const dirs: string[] = [];
  afterEach(() => {
    for (const dir of dirs.splice(0)) {
      try {
        rmSync(dir, { recursive: true, force: true });
      } catch {
        // Windows may hold the handle briefly; leak the tempdir.
      }
    }
  });

  test("an op whose blobs were evicted is reported, skipped, and never clobbers", async () => {
    const dir = mkdtempSync(join(tmpdir(), "wb-undo-"));
    dirs.push(dir);
    const { deps, ports } = makeDeps({});
    const journal = new JournalDb({ path: join(dir, "journal.db"), capBytes: 3000 });
    const evictingDeps: WriteEngineDeps = { ...deps, journal };

    const big = () =>
      `// ${Buffer.from(crypto.getRandomValues(new Uint8Array(4096))).toString("hex")}\n`;
    const early = big();
    ports.workspace.files.set("src/early.ts", early);
    await writeAs(evictingDeps, "s", "src/early.ts", big());
    // This append evicts the first op's blobs under the 3000-byte cap.
    await writeAs(evictingDeps, "s", "src/late.ts", big());

    const before = ports.workspace.files.get("src/early.ts");
    const { results, notes } = ok(await undoAs(evictingDeps, "s", { steps: 2 }));
    expect(results.some((r) => r.kind === "evicted")).toBe(true);
    expect(notes.join(" ")).toContain("no longer undoable");
    // The late op undid cleanly; the evicted one touched nothing.
    expect(ports.workspace.files.has("src/late.ts")).toBe(false);
    expect(ports.workspace.files.get("src/early.ts")).toBe(before as string);
    journal.close();
  });
});

describe("the journal as rows (action: list) — chunk:journal-listing-verb", () => {
  test("lists every session's ops newest-first with counts, diffs and undo state", async () => {
    const { deps } = makeDeps({ "src/a.ts": "const a = 1\n" });
    await writeAs(deps, "alice", "src/a.ts", "const a = 2\nconst b = 3\n");
    await writeAs(deps, "bob", "src/b.ts", "const b = 1\n");

    const { results } = ok(await undoAs(deps, "alice", { action: "list" }));
    const rows = results as JournalRow[];
    expect(rows.map((r) => r.seq)).toEqual([2, 1]);
    expect(rows.every((r) => r.kind === "op")).toBe(true);
    // Workspace scope by default: reading bob's history reverts nothing.
    expect(rows.map((r) => r.sessionId)).toEqual(["bob", "alice"]);

    const created = rows[0] as JournalRow;
    expect(created.path).toBe("src/b.ts");
    expect(created.change).toBe("create");
    expect(created.added).toBe(1);
    expect(created.removed).toBe(0);
    expect(created.parseOk).toBe(true);
    expect(created.undoable).toBe(true);
    expect(created.diff).toContain("+const b = 1");

    const edited = rows[1] as JournalRow;
    expect(edited.change).toBe("write");
    expect(edited.added).toBe(2);
    expect(edited.removed).toBe(1);
  });

  test("a rewound op is listed as undone by the op that consumed it", async () => {
    const { deps } = makeDeps({ "src/a.ts": "const a = 1\n" });
    await writeAs(deps, "alice", "src/a.ts", "const a = 2\n");
    ok(await undoAs(deps, "alice"));

    const rows = ok(await undoAs(deps, "alice", { action: "list" })).results as JournalRow[];
    const original = rows.find((r) => r.seq === 1) as JournalRow;
    expect(original.undoable).toBe(false);
    expect(original.undoneBy).toBe(2);
    const theUndo = rows.find((r) => r.seq === 2) as JournalRow;
    expect(theUndo.undoOf).toEqual([1]);
    expect(theUndo.undoable).toBe(false);
  });

  test("session scope, sinceSeq, limit and diff:false narrow the listing", async () => {
    const { deps } = makeDeps({});
    await writeAs(deps, "alice", "src/a.ts", "a\n");
    await writeAs(deps, "bob", "src/b.ts", "b\n");
    await writeAs(deps, "alice", "src/c.ts", "c\n");

    const mine = ok(await undoAs(deps, "alice", { action: "list", scope: "session" }))
      .results as JournalRow[];
    expect(mine.map((r) => r.seq)).toEqual([3, 1]);

    const tail = ok(await undoAs(deps, "alice", { action: "list", sinceSeq: 3 }))
      .results as JournalRow[];
    expect(tail.map((r) => r.seq)).toEqual([3]);

    const lean = ok(await undoAs(deps, "alice", { action: "list", limit: 1, diff: false }));
    expect((lean.results as JournalRow[]).map((r) => r.seq)).toEqual([3]);
    expect((lean.results[0] as JournalRow).diff).toBeUndefined();
    expect((lean.results[0] as JournalRow).added).toBe(1);
    expect(lean.notes.join(" ")).toContain("beforeSeq: 3");
  });

  test("a limited listing hands back a real cursor and drops nothing silently", async () => {
    const { deps } = makeDeps({});
    await writeAs(deps, "alice", "src/a.ts", "a\n");
    await writeAs(deps, "alice", "src/b.ts", "b\n");
    const envelope = await undoAs(deps, "alice", { action: "list", limit: 1 });
    if (!envelope.ok) throw new Error("expected ok");
    expect(envelope.truncation?.dropped).toBe(1);
    expect(envelope.truncation?.cursor).toBe("beforeSeq:2");
    const page2 = ok(await undoAs(deps, "alice", { action: "list", beforeSeq: 2 }))
      .results as JournalRow[];
    expect(page2.map((r) => r.seq)).toEqual([1]);
  });

  test("an empty journal says so instead of returning a bare empty list", async () => {
    const { deps } = makeDeps({});
    const { results, notes } = ok(await undoAs(deps, "alice", { action: "list" }));
    expect(results).toEqual([]);
    expect(notes.join(" ")).toContain("no ops");
  });

  test("listing writes nothing: the journal is unchanged and no op is consumed", async () => {
    const { deps, ports } = makeDeps({ "src/a.ts": "const a = 1\n" });
    await writeAs(deps, "alice", "src/a.ts", "const a = 2\n");
    ok(await undoAs(deps, "alice", { action: "list" }));
    expect((await ports.journal.list(0)).length).toBe(1);
    expect(ports.workspace.files.get("src/a.ts")).toBe("const a = 2\n");
    // The op listed is still the op undo(1) rewinds.
    ok(await undoAs(deps, "alice"));
    expect(ports.workspace.files.get("src/a.ts")).toBe("const a = 1\n");
  });

  test("an over-budget listing is cut on an op boundary with the reason said out loud", async () => {
    const { deps } = makeDeps({});
    for (let i = 0; i < 5; i++) {
      await writeAs(deps, "alice", `src/f${i}.ts`, `const x${i} = ${"y".repeat(400)}\n`);
    }
    const envelope = await undoAs(deps, "alice", { action: "list", budget: 200 });
    if (!envelope.ok) throw new Error("expected ok");
    expect(envelope.results.length).toBeGreaterThan(0);
    expect(envelope.results.length).toBeLessThan(5);
    expect(envelope.truncation?.reason).toBe("budget");
    expect(envelope.notes?.join(" ")).toContain("budget");
  });
});

describe("id-addressed rewind (ops) — chunk:journal-listing-verb", () => {
  test("ops rewinds exactly the named op, not the newest one", async () => {
    const { deps, ports } = makeDeps({ "src/a.ts": "a1\n", "src/b.ts": "b1\n" });
    await writeAs(deps, "alice", "src/a.ts", "a2\n");
    await writeAs(deps, "alice", "src/b.ts", "b2\n");

    const { notes } = ok(await undoAs(deps, "alice", { ops: [1] }));
    expect(ports.workspace.files.get("src/a.ts")).toBe("a1\n");
    expect(ports.workspace.files.get("src/b.ts")).toBe("b2\n");
    expect(notes.join(" ")).toContain("reverted op(s) 1");
  });

  test("a named op that is not a candidate is reported, and steps is ignored", async () => {
    const { deps, ports } = makeDeps({ "src/a.ts": "a1\n" });
    await writeAs(deps, "alice", "src/a.ts", "a2\n");
    ok(await undoAs(deps, "alice", { ops: [1] }));

    const again = ok(await undoAs(deps, "alice", { ops: [1], steps: 5 }));
    expect(again.results).toEqual([]);
    expect(again.notes.join(" ")).toContain("not undoable");
    expect(again.notes.join(" ")).toContain("steps is ignored");
    expect(ports.workspace.files.get("src/a.ts")).toBe("a1\n");
  });

  test("session scope still guards a named op", async () => {
    const { deps, ports } = makeDeps({ "src/a.ts": "a1\n" });
    await writeAs(deps, "bob", "src/a.ts", "a2\n");
    const refused = ok(await undoAs(deps, "alice", { ops: [1] }));
    expect(refused.results).toEqual([]);
    expect(ports.workspace.files.get("src/a.ts")).toBe("a2\n");
    ok(await undoAs(deps, "alice", { ops: [1], scope: "workspace" }));
    expect(ports.workspace.files.get("src/a.ts")).toBe("a1\n");
  });

  test("ops rejects an empty or non-seq list at the schema level", async () => {
    const { deps } = makeDeps({});
    expect(undoAs(deps, "alice", { ops: [] })).rejects.toThrow(RangeError);
    expect(undoAs(deps, "alice", { ops: [0] })).rejects.toThrow(RangeError);
  });
});
