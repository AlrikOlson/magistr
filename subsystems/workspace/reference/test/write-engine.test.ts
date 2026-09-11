/**
 * The write engine (SDD §7.5, law #3) — the single mutation choke point.
 * The suite pins the acceptance criteria of chunk:m2-write-engine: no write
 * that introduces a syntax error is ever applied; batch rollback is atomic,
 * including a real engine child killed mid-verify; and the envelope reports
 * the applied diff plus any new diagnostics.
 */

import { afterEach, describe, expect, test } from "bun:test";
import {
  EngineSupervisor,
  type EngineSupervisorOptions,
  IpcEngineAdapter,
} from "../src/adapters/engine-ipc";
import { createInProcessPorts, type InProcessPorts } from "../src/adapters/inprocess";
import type { ResponseEnvelope } from "../src/envelope";
import {
  runWriteBatch,
  type WriteEngineDeps,
  type WriteReport,
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

/** In-process deps over one seeded workspace: bracket-balance parse, the
 * trim-trailing-whitespace formatter, and the in-memory journal. */
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

function ok(envelope: ResponseEnvelope<WriteReport>): readonly WriteReport[] {
  if (!envelope.ok) throw new Error(`expected ok envelope, got ${envelope.error.code}`);
  return envelope.results;
}

function refused(envelope: ResponseEnvelope<WriteReport>) {
  if (envelope.ok) throw new Error("expected an error envelope, got ok");
  return envelope.error;
}

describe("the clean path", () => {
  test("applies a batch, reports diffs, hashes, journal seqs, and freshness warm", async () => {
    const { deps, ports } = makeDeps({ "src/a.ts": "const a = 1\n" });
    const envelope = await runWriteBatch(
      deps,
      {
        writes: [
          { path: "src/a.ts", content: "const a = 2\n" },
          { path: "src/b.ts", content: "const b = 1\n" },
        ],
      },
      1,
    );
    const results = ok(envelope);
    expect(envelope.ok && envelope.freshness).toBe("warm");
    expect(results).toHaveLength(2);

    const [a, b] = results as [WriteReport, WriteReport];
    expect(a.action).toBe("modified");
    expect(a.applied).toBe(true);
    expect(a.diff).toContain("-const a = 1");
    expect(a.diff).toContain("+const a = 2");
    expect(a.contentHash).toBe(sha256("const a = 2\n"));
    expect(b.action).toBe("created");
    expect(b.diff).toContain("--- /dev/null");

    expect(ports.workspace.files.get("src/a.ts")).toBe("const a = 2\n");
    expect(ports.workspace.files.get("src/b.ts")).toBe("const b = 1\n");

    // One op per batch (§10.1): both files ride the same journal seq.
    const journal = await ports.journal.list(0);
    expect(journal).toHaveLength(1);
    expect(journal[0]?.files).toHaveLength(2);
    expect(journal[0]?.files[0]?.kind).toBe("write");
    expect(journal[0]?.files[0]?.before).toBe("const a = 1\n");
    expect(journal[0]?.files[0]?.after).toBe("const a = 2\n");
    expect(journal[0]?.files[1]?.kind).toBe("create");
    expect(a.journalSeq).toBe(1);
    expect(b.journalSeq).toBe(1);
  });

  test("a deletion is journaled with its before-content and reported as an all-minus diff", async () => {
    const { deps, ports } = makeDeps({ "src/gone.ts": "const x = 1\n" });
    const results = ok(
      await runWriteBatch(deps, { writes: [{ path: "src/gone.ts", content: null }] }, 1),
    );
    expect(results[0]?.action).toBe("deleted");
    expect(results[0]?.diff).toContain("+++ /dev/null");
    expect(results[0]?.contentHash).toBeUndefined();
    expect(ports.workspace.files.has("src/gone.ts")).toBe(false);
    const journal = await ports.journal.list(0);
    expect(journal[0]?.files[0]?.kind).toBe("delete");
    expect(journal[0]?.files[0]?.before).toBe("const x = 1\n");
    expect(journal[0]?.files[0]?.after).toBe("");
  });

  test("an unchanged write is reported, not journaled, not re-applied", async () => {
    const { deps, ports } = makeDeps({ "src/a.ts": "const a = 1\n" });
    const results = ok(
      await runWriteBatch(deps, { writes: [{ path: "src/a.ts", content: "const a = 1\n" }] }, 1),
    );
    expect(results[0]?.action).toBe("unchanged");
    expect(results[0]?.applied).toBe(false);
    expect(results[0]?.diff).toBe("");
    expect(await ports.journal.list(0)).toHaveLength(0);
  });

  test("formatting is applied, re-verified, and journaled as the final content", async () => {
    const { deps, ports } = makeDeps({});
    const results = ok(
      await runWriteBatch(deps, { writes: [{ path: "src/n.ts", content: "const n = 1  " }] }, 1),
    );
    // The in-process formatter trims trailing whitespace and adds the newline.
    expect(ports.workspace.files.get("src/n.ts")).toBe("const n = 1\n");
    expect(results[0]?.contentHash).toBe(sha256("const n = 1\n"));
    expect(results[0]?.diff).toContain("+const n = 1");
    expect((await ports.journal.list(0))[0]?.files[0]?.after).toBe("const n = 1\n");
  });

  test("dry run verifies the batch and touches nothing", async () => {
    const { deps, ports } = makeDeps({ "src/a.ts": "const a = 1\n" });
    const envelope = await runWriteBatch(
      deps,
      { writes: [{ path: "src/a.ts", content: "const a = 2\n" }], dryRun: true },
      1,
    );
    const results = ok(envelope);
    expect(results[0]?.applied).toBe(false);
    expect(results[0]?.diff).toContain("+const a = 2");
    expect(results[0]?.journalSeq).toBeUndefined();
    expect(ports.workspace.files.get("src/a.ts")).toBe("const a = 1\n");
    expect(await ports.journal.list(0)).toHaveLength(0);
    expect(envelope.ok && envelope.notes?.some((n) => n.includes("dry run"))).toBe(true);
  });
});

describe("parse-verify (law #3)", () => {
  test("a new syntax error rejects the whole batch with PARSE_REJECTED and touches nothing", async () => {
    const { deps, ports } = makeDeps({
      "src/good.ts": "const g = 1\n",
      "src/bad.ts": "const b = 1\n",
    });
    const error = refused(
      await runWriteBatch(
        deps,
        {
          writes: [
            { path: "src/good.ts", content: "const g = 2\n" },
            { path: "src/bad.ts", content: "const b = (1\n" },
          ],
        },
        1,
      ),
    );
    expect(error.code).toBe("PARSE_REJECTED");
    expect(error.recovery).toBe("narrow");
    expect(error.parserMessage).toBeDefined();
    expect(error.rejectedDiff).toContain("+const b = (1");
    // Atomic rollback: the good file of the batch is untouched too.
    expect(ports.workspace.files.get("src/good.ts")).toBe("const g = 1\n");
    expect(ports.workspace.files.get("src/bad.ts")).toBe("const b = 1\n");
    expect(await ports.journal.list(0)).toHaveLength(0);
  });

  test("a write to an already-broken file is allowed and reports the remaining diagnostics", async () => {
    const { deps, ports } = makeDeps({ "src/broken.ts": "const b = (1\n" });
    const envelope = await runWriteBatch(
      deps,
      { writes: [{ path: "src/broken.ts", content: "const b = (2\n" }] },
      1,
    );
    const results = ok(envelope);
    expect(results[0]?.applied).toBe(true);
    expect(results[0]?.diagnostics?.length).toBe(1);
    expect(envelope.ok && envelope.notes?.some((n) => n.includes("already failed"))).toBe(true);
    expect(ports.workspace.files.get("src/broken.ts")).toBe("const b = (2\n");
  });

  test("a fix that repairs a broken file applies with no diagnostics", async () => {
    const { deps } = makeDeps({ "src/broken.ts": "const b = (1\n" });
    const results = ok(
      await runWriteBatch(
        deps,
        { writes: [{ path: "src/broken.ts", content: "const b = 1\n" }] },
        1,
      ),
    );
    expect(results[0]?.diagnostics).toBeUndefined();
  });

  test("non-source well-formedness: malformed json, yaml and toml reject; prose does not", async () => {
    for (const [path, bad] of [
      ["cfg.json", '{"a":}\n'],
      ["cfg.yaml", "a: b: c\n"],
      ["cfg.toml", "a =\n"],
    ] as const) {
      const { deps, ports } = makeDeps({});
      const error = refused(await runWriteBatch(deps, { writes: [{ path, content: bad }] }, 1));
      expect(error.code).toBe("PARSE_REJECTED");
      expect(ports.workspace.files.has(path)).toBe(false);
    }
    const { deps } = makeDeps({});
    const results = ok(
      await runWriteBatch(deps, { writes: [{ path: "notes.md", content: "hello\n" }] }, 1),
    );
    expect(results[0]?.applied).toBe(true);
  });

  test("a formatter that breaks the parse falls back to the verified content", async () => {
    const { deps, ports } = makeDeps({});
    const sabotaged: WriteEngineDeps = {
      ...deps,
      formatter: {
        version: 1,
        format: () => Promise.resolve({ content: "(((\n", changed: true }),
      },
    };
    const envelope = await runWriteBatch(
      sabotaged,
      { writes: [{ path: "src/n.ts", content: "const n = 1\n" }] },
      1,
    );
    ok(envelope);
    expect(ports.workspace.files.get("src/n.ts")).toBe("const n = 1\n");
    expect(envelope.ok && envelope.notes?.some((n) => n.includes("formatting broke"))).toBe(true);
  });
});

describe("target resolution", () => {
  test("a stale anchor refuses the batch and carries the fresh hash", async () => {
    const { deps, ports } = makeDeps({ "src/a.ts": "const a = 9\n" });
    const error = refused(
      await runWriteBatch(
        deps,
        {
          writes: [
            { path: "src/a.ts", content: "const a = 2\n", expectedHash: sha256("const a = 1\n") },
          ],
        },
        1,
      ),
    );
    expect(error.code).toBe("STALE_ANCHOR");
    expect(error.contentHash).toBe(sha256("const a = 9\n"));
    expect(ports.workspace.files.get("src/a.ts")).toBe("const a = 9\n");
  });

  test("a matching anchor admits the write", async () => {
    const { deps } = makeDeps({ "src/a.ts": "const a = 1\n" });
    const results = ok(
      await runWriteBatch(
        deps,
        {
          writes: [
            { path: "src/a.ts", content: "const a = 2\n", expectedHash: sha256("const a = 1\n") },
          ],
        },
        1,
      ),
    );
    expect(results[0]?.applied).toBe(true);
  });

  test("deleting a missing file refuses with FILE_NOT_FOUND", async () => {
    const { deps } = makeDeps({});
    const error = refused(
      await runWriteBatch(deps, { writes: [{ path: "src/ghost.ts", content: null }] }, 1),
    );
    expect(error.code).toBe("FILE_NOT_FOUND");
  });

  test("duplicate paths and empty batches are schema-level misuse", async () => {
    const { deps } = makeDeps({});
    await expect(runWriteBatch(deps, { writes: [] }, 1)).rejects.toThrow(RangeError);
    await expect(
      runWriteBatch(
        deps,
        {
          writes: [
            { path: "src/a.ts", content: "x\n" },
            { path: "./src/a.ts", content: "y\n" },
          ],
        },
        1,
      ),
    ).rejects.toThrow(RangeError);
  });
});

describe("atomicity beyond verify", () => {
  test("a swap failure mid-batch restores the files already swapped", async () => {
    const { deps, ports } = makeDeps({ "src/a.ts": "const a = 1\n" });
    const failing: WriteEngineDeps = {
      ...deps,
      workspace: {
        ...deps.workspace,
        write: (path, content) => {
          if (path === "src/b.ts") return Promise.reject(new Error("disk full"));
          return deps.workspace.write(path, content);
        },
      },
    };
    await expect(
      runWriteBatch(
        failing,
        {
          writes: [
            { path: "src/a.ts", content: "const a = 2\n" },
            { path: "src/b.ts", content: "const b = 1\n" },
          ],
        },
        1,
      ),
    ).rejects.toThrow("disk full");
    expect(ports.workspace.files.get("src/a.ts")).toBe("const a = 1\n");
    expect(ports.workspace.files.has("src/b.ts")).toBe(false);
  });

  test("the lock seam wraps the whole pipeline exactly once per batch", async () => {
    const { deps } = makeDeps({});
    let held = 0;
    let acquisitions = 0;
    const locked: WriteEngineDeps = {
      ...deps,
      lock: {
        withLock: async (fn) => {
          acquisitions += 1;
          held += 1;
          try {
            return await fn();
          } finally {
            held -= 1;
          }
        },
      },
    };
    const results = ok(
      await runWriteBatch(locked, { writes: [{ path: "src/a.ts", content: "const a = 1\n" }] }, 1),
    );
    expect(results[0]?.applied).toBe(true);
    expect(acquisitions).toBe(1);
    expect(held).toBe(0);
  });
});

describe("engine failure mid-verify (chaos leg)", () => {
  const MARKER = "WORKBENCH_CHAOS_MARKER";
  const supervisors: EngineSupervisor[] = [];

  function makeSupervisor(env: Record<string, string>): EngineSupervisor {
    const options: EngineSupervisorOptions = {
      env: { WORKBENCH_ENGINE_STUB_CORE: "1", ...env },
      backoffMs: [0, 10, 20],
      readyTimeoutMs: 10_000,
    };
    const supervisor = new EngineSupervisor(options);
    supervisors.push(supervisor);
    return supervisor;
  }

  afterEach(async () => {
    await Promise.all(supervisors.splice(0).map((s) => s.stop()));
  });

  test("a real engine child killed mid-verify rolls the batch back untouched", async () => {
    const supervisor = makeSupervisor({
      WORKBENCH_ENGINE_KILL_ON: "parse",
      WORKBENCH_ENGINE_KILL_MATCH: MARKER,
    });
    const files = new Map<string, string>([
      ["clean.ts", "const a = 1\n"],
      ["boom.ts", "const b = 1\n"],
    ]);
    const ports = createInProcessPorts();
    const deps: WriteEngineDeps = {
      engine: new IpcEngineAdapter(supervisor, {
        read: (path) => Promise.reject(enoent(path)),
        list: () => Promise.resolve([]),
      }),
      formatter: ports.formatter,
      journal: ports.journal,
      workspace: accessOf(files),
    };
    const error = refused(
      await runWriteBatch(
        deps,
        {
          writes: [
            { path: "clean.ts", content: "const a = 2\n" },
            { path: "boom.ts", content: `const b = 2 // ${MARKER}\n` },
          ],
        },
        1,
      ),
    );
    expect(error.code).toBe("ENGINE_UNAVAILABLE");
    expect(error.recovery).toBe("retry");
    // Atomic rollback: neither file moved, nothing was journaled.
    expect(files.get("clean.ts")).toBe("const a = 1\n");
    expect(files.get("boom.ts")).toBe("const b = 1\n");
    expect(await ports.journal.list(0)).toHaveLength(0);
  }, 30_000);

  test("a quarantined input surfaces INPUT_QUARANTINED, not a retry loop", async () => {
    const { deps } = makeDeps({ "poison.ts": "const p = 1\n" });
    const quarantining: WriteEngineDeps = {
      ...deps,
      engine: {
        version: 2,
        parse: () => {
          const error = new Error("poison.ts is quarantined") as Error & { code: string };
          error.code = "INPUT_QUARANTINED";
          return Promise.reject(error);
        },
        search: () => Promise.resolve([]),
        rewrite: (r) => Promise.resolve({ content: r.content, changed: false }),
      },
    };
    const error = refused(
      await runWriteBatch(
        quarantining,
        { writes: [{ path: "poison.ts", content: "const p = 2\n" }] },
        1,
      ),
    );
    expect(error.code).toBe("INPUT_QUARANTINED");
    expect(error.recovery).toBe("escalate");
  });
});
