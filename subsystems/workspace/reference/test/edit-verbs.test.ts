/**
 * The edit tool (SDD §9.4) — chunk:m2-edit-verbs acceptance:
 *
 *   1. every edit verb exercised against fixture repos in 6 languages
 *      (typescript, javascript, python, rust, go, java);
 *   2. a syntactically broken write is rejected in the same tool call, with
 *      the parser message and the rejected diff;
 *   3. STALE_ANCHOR fires with a fresh content hash when a file changed
 *      since the diff was generated.
 *
 * The bun lane runs the in-process ports: symbol outlines are seeded (real
 * per-language outline extraction is the core lanes' tested domain), the
 * parse-verify guard is the bracket-balance parser, and engine.rewrite is
 * the literal-replace degradation. What this suite pins is the verb
 * resolution, gating, anchoring, and choke-point contract of edit itself.
 */

import { describe, expect, test } from "bun:test";
import { createInProcessPorts, type InProcessPorts } from "../src/adapters/inprocess";
import type { ResponseEnvelope } from "../src/envelope";
import { checkConfirmGrant, KEY_LENGTH, mintConfirmGrant } from "../src/jail";
import type { IndexPort, SymbolEntry } from "../src/ports";
import { type EditDeps, type EditInput, runEdit } from "../src/tools/edit";
import type { WriteEngineDeps, WriteReport } from "../src/tools/write-engine";

function sha256(content: string): string {
  return new Bun.CryptoHasher("sha256").update(content).digest("hex");
}

function enoent(path: string): NodeJS.ErrnoException {
  const error = new Error(`ENOENT: no such file: ${path}`) as NodeJS.ErrnoException;
  error.code = "ENOENT";
  return error;
}

/** One language fixture: a two-function module plus its outline rows. */
interface Fixture {
  readonly lang: string;
  readonly path: string;
  readonly content: string;
  readonly symbols: readonly SymbolEntry[];
  /** A replacement body for `alpha` that still parses (brackets balanced). */
  readonly newBody: string;
  /** A line to insert around `alpha`. */
  readonly inserted: string;
}

function symbol(path: string, name: string, kind: string, line: number, endLine: number) {
  return { name, kind, location: { path, line, column: 1 }, endLine };
}

/** Fixture repos in 6 languages; `alpha` spans lines 1–3, `beta` lines 5–7. */
const FIXTURES: readonly Fixture[] = [
  {
    lang: "typescript",
    path: "src/alpha.ts",
    content:
      "function alpha(): number {\n  return 1;\n}\n\nfunction beta(): number {\n  return alpha() + 1;\n}\n",
    symbols: [
      symbol("src/alpha.ts", "alpha", "function", 1, 3),
      symbol("src/alpha.ts", "beta", "function", 5, 7),
    ],
    newBody: "function alpha(): number {\n  return 42;\n}",
    inserted: "// inserted-line",
  },
  {
    lang: "javascript",
    path: "src/alpha.js",
    content: "function alpha() {\n  return 1;\n}\n\nfunction beta() {\n  return alpha() + 1;\n}\n",
    symbols: [
      symbol("src/alpha.js", "alpha", "function", 1, 3),
      symbol("src/alpha.js", "beta", "function", 5, 7),
    ],
    newBody: "function alpha() {\n  return 42;\n}",
    inserted: "// inserted-line",
  },
  {
    lang: "python",
    path: "src/alpha.py",
    content: "def alpha():\n    return 1\n\n\ndef beta():\n    return alpha() + 1\n",
    symbols: [
      symbol("src/alpha.py", "alpha", "function", 1, 2),
      symbol("src/alpha.py", "beta", "function", 5, 6),
    ],
    newBody: "def alpha():\n    return 42",
    inserted: "# inserted-line",
  },
  {
    lang: "rust",
    path: "src/alpha.rs",
    content: "fn alpha() -> u32 {\n    1\n}\n\nfn beta() -> u32 {\n    alpha() + 1\n}\n",
    symbols: [
      symbol("src/alpha.rs", "alpha", "function", 1, 3),
      symbol("src/alpha.rs", "beta", "function", 5, 7),
    ],
    newBody: "fn alpha() -> u32 {\n    42\n}",
    inserted: "// inserted-line",
  },
  {
    lang: "go",
    path: "src/alpha.go",
    content: "func alpha() int {\n\treturn 1\n}\n\nfunc beta() int {\n\treturn alpha() + 1\n}\n",
    symbols: [
      symbol("src/alpha.go", "alpha", "function", 1, 3),
      symbol("src/alpha.go", "beta", "function", 5, 7),
    ],
    newBody: "func alpha() int {\n\treturn 42\n}",
    inserted: "// inserted-line",
  },
  {
    lang: "java",
    path: "src/Alpha.java",
    content:
      "class Alpha {\n  static int alpha() {\n    return 1;\n  }\n\n  static int beta() {\n    return alpha() + 1;\n  }\n}\n",
    symbols: [
      symbol("src/Alpha.java", "Alpha", "class", 1, 9),
      symbol("src/Alpha.java", "alpha", "method", 2, 4),
      symbol("src/Alpha.java", "beta", "method", 6, 8),
    ],
    newBody: "  static int alpha() {\n    return 42;\n  }",
    inserted: "  // inserted-line",
  },
];

interface Rig {
  readonly deps: EditDeps;
  readonly ports: InProcessPorts;
  readonly files: Map<string, string>;
}

/** In-process rig over seeded files + outlines, mirrored on write-engine.test. */
function makeRig(seed: readonly Fixture[] = FIXTURES, extra: Record<string, string> = {}): Rig {
  const ports = createInProcessPorts();
  const files = ports.workspace.files;
  for (const fixture of seed) {
    files.set(fixture.path, fixture.content);
    void ports.index.upsertFile({ path: fixture.path, symbols: fixture.symbols });
  }
  for (const [path, content] of Object.entries(extra)) files.set(path, content);
  const write: WriteEngineDeps = {
    engine: ports.engine,
    formatter: ports.formatter,
    journal: ports.journal,
    workspace: {
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
    },
  };
  // A fixed key: this suite pins verb/gate behavior; the key lifecycle and
  // the token flows have their own suites (jail-keys, mrtr-confirm).
  const key = Buffer.alloc(KEY_LENGTH, 7);
  const deps: EditDeps = {
    write,
    index: ports.index,
    confirm: {
      mint: (diffHash, contentHash) => mintConfirmGrant(key, diffHash, contentHash),
      check: (token) => checkConfirmGrant(key, token),
    },
    list: () => Promise.resolve([...files.keys()].map((path) => ({ path, junk: false }))),
  };
  return { deps, ports, files };
}

function ok(envelope: ResponseEnvelope<WriteReport>): readonly WriteReport[] {
  if (!envelope.ok) throw new Error(`expected ok envelope, got ${envelope.error.code}`);
  return envelope.results;
}

function refused(envelope: ResponseEnvelope<WriteReport>) {
  if (envelope.ok) throw new Error("expected an error envelope, got ok");
  return envelope.error;
}

describe("every verb against fixture repos in 6 languages", () => {
  for (const fixture of FIXTURES) {
    describe(fixture.lang, () => {
      test("replace_body replaces the symbol's extent and applies", async () => {
        const rig = makeRig();
        const envelope = await runEdit(
          rig.deps,
          { op: "replace_body", file: fixture.path, symbol: "alpha", content: fixture.newBody },
          1,
        );
        const results = ok(envelope);
        expect(results).toHaveLength(1);
        expect(results[0]?.applied).toBe(true);
        expect(results[0]?.diff).toContain("+");
        const after = rig.files.get(fixture.path) as string;
        expect(after).toContain("42");
        expect(after).not.toContain("return 1");
        // beta is untouched.
        expect(after).toContain("beta");
      });

      test("insert_after and insert_before land around the extent", async () => {
        const rig = makeRig();
        ok(
          await runEdit(
            rig.deps,
            { op: "insert_after", file: fixture.path, symbol: "alpha", content: fixture.inserted },
            1,
          ),
        );
        const afterInsert = rig.files.get(fixture.path) as string;
        expect(afterInsert).toContain("inserted-line");
        const alpha = fixture.symbols.find((s) => s.name === "alpha") as SymbolEntry;
        const lines = afterInsert.split("\n");
        expect(lines[alpha.endLine as number]).toContain("inserted-line");

        const rig2 = makeRig();
        ok(
          await runEdit(
            rig2.deps,
            { op: "insert_before", file: fixture.path, symbol: "beta", content: fixture.inserted },
            1,
          ),
        );
        const beta = fixture.symbols.find((s) => s.name === "beta") as SymbolEntry;
        const lines2 = (rig2.files.get(fixture.path) as string).split("\n");
        expect(lines2[beta.location.line - 1]).toContain("inserted-line");
      });

      test("rename rewrites definition and uses on word boundaries (gated → autoApply)", async () => {
        const rig = makeRig();
        const gated = await runEdit(
          rig.deps,
          { op: "rename", file: fixture.path, symbol: "alpha", newName: "gamma" },
          1,
        );
        expect(refused(gated).code).toBe("CONFIRM_REQUIRED");

        const applied = await runEdit(
          rig.deps,
          { op: "rename", file: fixture.path, symbol: "alpha", newName: "gamma", autoApply: true },
          1,
        );
        ok(applied);
        const after = rig.files.get(fixture.path) as string;
        expect(after).not.toMatch(/\balpha\b/);
        expect(after).toContain("gamma");
        // Word boundary: the Alpha class in the java fixture must survive.
        if (fixture.lang === "java") expect(after).toContain("class Alpha");
      });

      test("rewrite applies the pattern across the glob scope (gated → autoApply)", async () => {
        const rig = makeRig();
        const applied = await runEdit(
          rig.deps,
          {
            op: "rewrite",
            pattern: "return 1",
            rewritePattern: "return 7",
            path: `src/**`,
            autoApply: true,
          },
          1,
        );
        const results = ok(applied);
        expect(results.length).toBeGreaterThanOrEqual(1);
        expect(rig.files.get(fixture.path) as string).not.toContain("return 1;");
      });

      test("replace_text replaces the addressed occurrence", async () => {
        const rig = makeRig();
        const envelope = await runEdit(
          rig.deps,
          { op: "replace_text", file: fixture.path, old: "1", new: "9", occurrence: 1 },
          1,
        );
        const results = ok(envelope);
        expect(results[0]?.applied).toBe(true);
        expect((rig.files.get(fixture.path) as string).includes("9")).toBe(true);
      });
    });
  }
});

describe("acceptance: parse-verify rejects in the same call", () => {
  test("a body that breaks syntax answers PARSE_REJECTED with parser message and rejected diff", async () => {
    const rig = makeRig();
    const fixture = FIXTURES[0] as Fixture;
    const before = rig.files.get(fixture.path) as string;
    const envelope = await runEdit(
      rig.deps,
      {
        op: "replace_body",
        file: fixture.path,
        symbol: "alpha",
        content: "function alpha() {\n  return (1;\n}",
      },
      1,
    );
    const error = refused(envelope);
    expect(error.code).toBe("PARSE_REJECTED");
    expect(error.parserMessage).toBeString();
    expect(error.rejectedDiff).toContain("+  return (1;");
    // Same-call rejection: nothing was applied.
    expect(rig.files.get(fixture.path)).toBe(before);
  });
});

describe("acceptance: STALE_ANCHOR carries the fresh content hash", () => {
  test("expectedHash from read, file changed since → STALE_ANCHOR + fresh hash", async () => {
    const rig = makeRig();
    const fixture = FIXTURES[0] as Fixture;
    const readHash = sha256(rig.files.get(fixture.path) as string);
    // The file changes after the agent read it but before the edit call.
    const changed = `// drifted\n${rig.files.get(fixture.path) as string}`;
    rig.files.set(fixture.path, changed);
    const envelope = await runEdit(
      rig.deps,
      {
        op: "replace_text",
        file: fixture.path,
        old: "return 1;",
        new: "return 2;",
        expectedHash: readHash,
      },
      1,
    );
    const error = refused(envelope);
    expect(error.code).toBe("STALE_ANCHOR");
    expect(error.contentHash).toBe(sha256(changed));
  });

  test("replace_text anchor text absent → STALE_ANCHOR + fresh hash", async () => {
    const rig = makeRig();
    const fixture = FIXTURES[0] as Fixture;
    const envelope = await runEdit(
      rig.deps,
      { op: "replace_text", file: fixture.path, old: "no such anchor", new: "x" },
      1,
    );
    const error = refused(envelope);
    expect(error.code).toBe("STALE_ANCHOR");
    expect(error.contentHash).toBe(sha256(rig.files.get(fixture.path) as string));
  });

  test("occurrence out of range → STALE_ANCHOR", async () => {
    const rig = makeRig();
    const fixture = FIXTURES[0] as Fixture;
    const envelope = await runEdit(
      rig.deps,
      { op: "replace_text", file: fixture.path, old: "alpha", new: "x", occurrence: 99 },
      1,
    );
    expect(refused(envelope).code).toBe("STALE_ANCHOR");
  });
});

describe("the confirm gate (§9.4 seam until m2-mrtr-confirm)", () => {
  test("rewrite without autoApply answers CONFIRM_REQUIRED with the aggregated diff in notes", async () => {
    const rig = makeRig();
    const envelope = await runEdit(
      rig.deps,
      { op: "rewrite", pattern: "return 1", rewritePattern: "return 7" },
      1,
    );
    expect(refused(envelope).code).toBe("CONFIRM_REQUIRED");
    expect((envelope.notes ?? []).join("\n")).toContain("return 7");
  });

  test("dryRun previews a gated op without applying", async () => {
    const rig = makeRig();
    const fixture = FIXTURES[0] as Fixture;
    const before = rig.files.get(fixture.path) as string;
    const envelope = await runEdit(
      rig.deps,
      { op: "rewrite", pattern: "return 1", rewritePattern: "return 7", dryRun: true },
      1,
    );
    const results = ok(envelope);
    expect(results.every((r) => r.applied === false)).toBe(true);
    expect(rig.files.get(fixture.path)).toBe(before);
  });

  test("a single-file edit beyond 200 changed lines is gated", async () => {
    const big = `${Array.from({ length: 300 }, (_, i) => `line ${i}`).join("\n")}\n`;
    const rig = makeRig(FIXTURES, { "notes/big.txt": big });
    const envelope = await runEdit(
      rig.deps,
      { op: "replace_text", file: "notes/big.txt", old: "line", new: "row" },
      1,
    );
    expect(refused(envelope).code).toBe("CONFIRM_REQUIRED");
  });

  test("single-file symbol edits auto-apply — parse-verify is the guard", async () => {
    const rig = makeRig();
    const fixture = FIXTURES[0] as Fixture;
    const envelope = await runEdit(
      rig.deps,
      { op: "replace_body", file: fixture.path, symbol: "alpha", content: fixture.newBody },
      1,
    );
    expect(ok(envelope)[0]?.applied).toBe(true);
  });
});

describe("taxonomy edges", () => {
  test("symbol verbs without a warm index answer INDEX_UNAVAILABLE", async () => {
    const rig = makeRig();
    const coldIndex: IndexPort = {
      ...rig.ports.index,
      status: () =>
        Promise.resolve({ tier: 0 as const, generation: 0, building: true, filesIndexed: 1 }),
      queryOutline: (q) => rig.ports.index.queryOutline(q),
      queryReferences: (q) => rig.ports.index.queryReferences(q),
    } as IndexPort;
    const deps: EditDeps = { ...rig.deps, index: coldIndex };
    const envelope = await runEdit(
      deps,
      { op: "replace_body", file: (FIXTURES[0] as Fixture).path, symbol: "alpha", content: "x" },
      1,
    );
    expect(refused(envelope).code).toBe("INDEX_UNAVAILABLE");
  });

  test("an unknown symbol answers SYMBOL_NOT_FOUND; an ambiguous one AMBIGUOUS_SYMBOL", async () => {
    const rig = makeRig();
    const fixture = FIXTURES[0] as Fixture;
    const missing = await runEdit(
      rig.deps,
      { op: "replace_body", file: fixture.path, symbol: "nope", content: "x" },
      1,
    );
    expect(refused(missing).code).toBe("SYMBOL_NOT_FOUND");

    void rig.ports.index.upsertFile({
      path: fixture.path,
      symbols: [...fixture.symbols, symbol(fixture.path, "alpha", "function", 9, 11)],
    });
    const ambiguous = await runEdit(
      rig.deps,
      { op: "replace_body", file: fixture.path, symbol: "alpha", content: "x" },
      1,
    );
    expect(refused(ambiguous).code).toBe("AMBIGUOUS_SYMBOL");
  });

  test("a missing file answers FILE_NOT_FOUND", async () => {
    const rig = makeRig();
    const envelope = await runEdit(
      rig.deps,
      { op: "replace_text", file: "no/such.ts", old: "a", new: "b" },
      1,
    );
    expect(refused(envelope).code).toBe("FILE_NOT_FOUND");
  });

  test("rename to a non-identifier answers PATTERN_INVALID", async () => {
    const rig = makeRig();
    const envelope = await runEdit(
      rig.deps,
      { op: "rename", file: (FIXTURES[0] as Fixture).path, symbol: "alpha", newName: "not a name" },
      1,
    );
    expect(refused(envelope).code).toBe("PATTERN_INVALID");
  });

  test("a rewrite matching nothing answers ok with no writes", async () => {
    const rig = makeRig();
    const envelope = await runEdit(
      rig.deps,
      { op: "rewrite", pattern: "no such text anywhere", rewritePattern: "x", autoApply: true },
      1,
    );
    expect(ok(envelope)).toHaveLength(0);
    expect((envelope.notes ?? []).join("\n")).toContain("nothing to change");
  });

  test("schema-level misuse throws RangeError", () => {
    const rig = makeRig();
    expect(runEdit(rig.deps, { op: "replace_body", file: "a.ts" } as EditInput, 1)).rejects.toThrow(
      RangeError,
    );
    expect(runEdit(rig.deps, { op: "bogus" } as unknown as EditInput, 1)).rejects.toThrow(
      RangeError,
    );
  });
});

describe("cross-file rename through the choke point", () => {
  test("rename touches the definition and reference files in one journaled batch", async () => {
    const fixture = FIXTURES[0] as Fixture;
    const rig = makeRig(FIXTURES, {
      "src/caller.ts": "import { alpha } from './alpha';\nexport const x = alpha();\n",
    });
    const envelope = await runEdit(
      rig.deps,
      { op: "rename", file: fixture.path, symbol: "alpha", newName: "gamma", autoApply: true },
      1,
      "session-a",
    );
    const results = ok(envelope);
    expect(results.length).toBeGreaterThanOrEqual(2);
    expect(rig.files.get("src/caller.ts")).toContain("gamma()");
    // One batch = one journal seq for every file (undo rewinds the whole op).
    const seqs = new Set(results.map((r) => r.journalSeq));
    expect(seqs.size).toBe(1);
    // The provenance note is honest about being name-resolution-free.
    expect((envelope.notes ?? []).join("\n")).toContain("name-resolution-free");
  });
});
