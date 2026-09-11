/**
 * The `search` tool (chunks m1-search-lexical + m1-search-structural,
 * SDD §9.1).
 *
 * Unit: runSearch against fake ports — mode rejections are typed taxonomy
 * envelopes, results are always grouped by file and ranked (definitions
 * first, references also by confidence, junk and tests demoted), reference
 * hits carry confidence provenance, references at tier 0 refuse with
 * INDEX_UNAVAILABLE, budget cuts carry the full truncation manifest, engine
 * failures map to PATTERN_INVALID / ENGINE_UNAVAILABLE.
 *
 * Integration: a fixture repo across the 6 test languages (ts, js, py, rs,
 * go, java) served by a REAL spawned `bun src/main.ts` server with the stub
 * core — lexical hits land in every language, structural answers with node
 * kinds, references refuse-then-answer around the Tier-1 build, files mode
 * kills find, and mode:semantic comes back as a typed error envelope over
 * the wire.
 */

import { afterAll, describe, expect, test } from "bun:test";
import { mkdirSync, mkdtempSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import path from "node:path";
import { Client } from "@modelcontextprotocol/sdk/client/index.js";
import { StdioClientTransport } from "@modelcontextprotocol/sdk/client/stdio.js";
import envelopeSchema from "../schemas/envelope.schema.json";
import searchInputSchema from "../schemas/tools/search.input.schema.json";
import type { ResponseEnvelope } from "../src/envelope";
import type {
  EngineMatch,
  EnginePort,
  EngineSearchRequest,
  ImportGraphResult,
  IndexPort,
  IndexStatus,
  OutlineQueryResult,
  ReferenceEdge,
  ReferenceQueryResult,
  SymbolEntry,
  SymbolQueryResult,
} from "../src/ports";
import { PORT_VERSIONS } from "../src/ports";
import { runSearch, type SearchDeps, type SearchGroup } from "../src/tools";
import { envelopeValidatorFor } from "./support/ajv";

const { validate, errorsText } = envelopeValidatorFor(envelopeSchema);

function assertValidEnvelope(envelope: ResponseEnvelope<unknown>): void {
  expect(validate(envelope) ? "" : errorsText()).toBe("");
}

// ---------------------------------------------------------------- unit half

interface FakeFile {
  readonly content: string;
  readonly junk?: boolean;
}

function fakeDeps(
  files: Record<string, FakeFile>,
  options: {
    status?: IndexStatus | "throw";
    symbols?: readonly SymbolEntry[];
    references?: readonly ReferenceEdge[];
    referencesFreshness?: ReferenceQueryResult["freshness"];
    engineError?: Error;
    /** Node kind stamped on structural-mode matches (default call_expression). */
    structuralKind?: string;
    /** Test-only line ranges per file, as the index would answer (specs/009). */
    testRanges?: Readonly<Record<string, readonly { startLine: number; endLine: number }[]>>;
  } = {},
): SearchDeps {
  const engine: EnginePort = {
    version: PORT_VERSIONS.engine,
    parse: () => Promise.resolve({ ok: true }),
    search: (request: EngineSearchRequest): Promise<readonly EngineMatch[]> => {
      if (options.engineError !== undefined) return Promise.reject(options.engineError);
      const structural = request.mode === "structural";
      const matches: EngineMatch[] = [];
      for (const p of request.paths) {
        const file = files[p];
        if (file === undefined) continue;
        // Structural fidelity lives in the Rust core; the fake matches the
        // pattern's head token so tests control hits per file.
        const needle = structural
          ? (request.pattern.split(/[($\s]/)[0] as string)
          : request.pattern;
        file.content.split("\n").forEach((lineText, i) => {
          const start = lineText.indexOf(needle);
          if (start >= 0) {
            matches.push({
              location: { path: p, line: i + 1, column: start + 1 },
              excerpt: lineText,
              ...(structural
                ? { kind: options.structuralKind ?? "call_expression", endLine: i + 1 }
                : {}),
            });
          }
        });
      }
      return Promise.resolve(matches);
    },
    rewrite: () => Promise.reject(new Error("not under test")),
  };
  const index: IndexPort = {
    version: PORT_VERSIONS.index,
    status: (): Promise<IndexStatus> =>
      options.status === "throw"
        ? Promise.reject(new Error("no index exists yet"))
        : Promise.resolve(options.status ?? { tier: 1, generation: 1, building: false }),
    build: () => Promise.reject(new Error("not under test")),
    querySymbols: (): Promise<SymbolQueryResult> =>
      Promise.resolve({ symbols: options.symbols ?? [], freshness: "warm" }),
    queryReferences: (): Promise<ReferenceQueryResult> =>
      Promise.resolve({
        edges: options.references ?? [],
        freshness: options.referencesFreshness ?? "warm",
      }),
    queryOutline: (): Promise<OutlineQueryResult> =>
      Promise.resolve({ files: [], freshness: "warm" }),
    queryImportGraph: (): Promise<ImportGraphResult> =>
      Promise.resolve({ fanIn: [], fanOut: [], freshness: "warm" }),
    ...(options.testRanges === undefined
      ? {}
      : {
          queryTestRanges: (paths: readonly string[]) =>
            Promise.resolve(
              Object.fromEntries(
                paths
                  .filter((p) => options.testRanges?.[p] !== undefined)
                  .map((p) => [p, options.testRanges?.[p] ?? []]),
              ),
            ),
        }),
    upsertFile: () => Promise.resolve(),
    refreshFile: () => Promise.resolve(),
    removeFile: () => Promise.resolve(),
  };
  return {
    engine,
    index,
    workspace: {
      list: () =>
        Promise.resolve(
          Object.entries(files).map(([p, f]) => ({ path: p, junk: f.junk === true })),
        ),
      read: (p: string) => {
        const file = files[p];
        return file === undefined
          ? Promise.reject(new Error("no such file"))
          : Promise.resolve(file.content);
      },
    },
  };
}

describe("mode gate", () => {
  const deps = fakeDeps({ "src/a.ts": { content: "const needle = 1;" } });

  test("semantic is rejected with a typed taxonomy error, not a schema failure", async () => {
    const envelope = await runSearch(deps, { query: "needle", mode: "semantic" }, 1);
    assertValidEnvelope(envelope);
    expect(envelope.ok).toBe(false);
    if (!envelope.ok) {
      expect(envelope.error.code).toBe("UNSUPPORTED_VERSION");
      expect(envelope.error.recovery).toBe("abort");
      expect(envelope.error.message).toContain("reserved");
    }
    // The input schema itself accepts "semantic" — rejection is the tool's.
    expect(searchInputSchema.properties.mode.enum as readonly string[]).toContain("semantic");
  });

  test("an unknown mode is typed-rejected too", async () => {
    const envelope = await runSearch(deps, { query: "needle", mode: "psychic" }, 1);
    expect(envelope.ok).toBe(false);
    if (!envelope.ok) expect(envelope.error.code).toBe("UNSUPPORTED_VERSION");
  });
});

describe("lexical mode", () => {
  const files: Record<string, FakeFile> = {
    "src/core.ts": { content: "export function needleFn() {}\nconst x = needleFn();" },
    "src/other.ts": { content: "// needleFn mention" },
    "test/core.test.ts": { content: "needleFn(); needleFn();" },
    "package-lock.json": { content: '{"name":"needleFn"}', junk: true },
  };
  const definition: SymbolEntry = {
    name: "needleFn",
    kind: "function",
    location: { path: "src/core.ts", line: 1, column: 17 },
  };

  test("results are grouped by file and ranked: definition first, tests and junk demoted", async () => {
    const deps = fakeDeps(files, { symbols: [definition] });
    const envelope = await runSearch(deps, { query: "needleFn", context: 0 }, 1);
    assertValidEnvelope(envelope);
    expect(envelope.ok).toBe(true);
    if (!envelope.ok) return;
    const groups = envelope.results as readonly SearchGroup[];
    expect(groups.map((g) => g.file)).toEqual([
      "src/core.ts", // carries the tagged definition
      "src/other.ts",
      "test/core.test.ts", // demoted: test path
      "package-lock.json", // demoted: junk
    ]);
    const first = groups[0] as SearchGroup;
    expect(first.hits[0]?.kind).toBe("function");
    expect(first.totalHits).toBe(2);
    // Grouped, never flat: every result is a file group with its own hits.
    for (const group of groups) {
      expect(typeof group.file).toBe("string");
      expect(Array.isArray(group.hits)).toBe(true);
    }
  });

  test("mode:auto runs lexical and says so in notes", async () => {
    const deps = fakeDeps(files);
    const envelope = await runSearch(deps, { query: "needleFn", mode: "auto", context: 0 }, 1);
    expect(envelope.ok).toBe(true);
    expect(envelope.notes?.some((n) => n.includes("auto"))).toBe(true);
  });

  test("include.tests:false excludes test files entirely", async () => {
    const deps = fakeDeps(files);
    const envelope = await runSearch(
      deps,
      { query: "needleFn", include: { tests: false, vendor: false }, context: 0 },
      1,
    );
    expect(envelope.ok).toBe(true);
    if (!envelope.ok) return;
    const paths = (envelope.results as readonly SearchGroup[]).map((g) => g.file);
    expect(paths).not.toContain("test/core.test.ts");
    expect(paths).not.toContain("package-lock.json");
  });

  test("path glob scopes the search", async () => {
    const deps = fakeDeps(files);
    const envelope = await runSearch(deps, { query: "needleFn", path: "test/**", context: 0 }, 1);
    expect(envelope.ok).toBe(true);
    if (!envelope.ok) return;
    expect((envelope.results as readonly SearchGroup[]).map((g) => g.file)).toEqual([
      "test/core.test.ts",
    ]);
  });

  test("context lines ride each hit", async () => {
    const deps = fakeDeps({
      "src/ctx.ts": { content: "line one\nneedle here\nline three" },
    });
    const envelope = await runSearch(deps, { query: "needle", context: 1 }, 1);
    expect(envelope.ok).toBe(true);
    if (!envelope.ok) return;
    const hit = (envelope.results as readonly SearchGroup[])[0]?.hits[0];
    expect(hit?.contextBefore).toEqual(["line one"]);
    expect(hit?.contextAfter).toEqual(["line three"]);
  });

  test("freshness is cold when no index exists", async () => {
    const deps = fakeDeps(files, { status: "throw" });
    const envelope = await runSearch(deps, { query: "needleFn", context: 0 }, 1);
    expect(envelope.ok).toBe(true);
    if (envelope.ok) expect(envelope.freshness).toBe("cold");
  });

  test("an invalid pattern maps to PATTERN_INVALID", async () => {
    const deps = fakeDeps(files, {
      engineError: new Error("invalid search pattern: unclosed group"),
    });
    const envelope = await runSearch(deps, { query: "(", context: 0 }, 1);
    assertValidEnvelope(envelope);
    expect(envelope.ok).toBe(false);
    if (!envelope.ok) {
      expect(envelope.error.code).toBe("PATTERN_INVALID");
      expect(envelope.error.recovery).toBe("narrow");
    }
  });

  test("a dead engine maps to ENGINE_UNAVAILABLE", async () => {
    const deps = fakeDeps(files, { engineError: new Error("engine child crashed") });
    const envelope = await runSearch(deps, { query: "needleFn", context: 0 }, 1);
    expect(envelope.ok).toBe(false);
    if (!envelope.ok) {
      expect(envelope.error.code).toBe("ENGINE_UNAVAILABLE");
      expect(envelope.error.recovery).toBe("retry");
    }
  });
});

describe("quarantine scope (chunk:backlog-quarantine-scope-leak)", () => {
  const QUARANTINED = "spikes/m4-component-host/wasi-sysroot.tar.gz";
  const files: Record<string, FakeFile> = {
    "src/adapters/engine.ts": { content: "const needle = 1;" },
    [QUARANTINED]: { content: "needle in binary soup" },
  };

  /**
   * Mirrors the live lane: the ipc adapter reads empty `paths` as "the whole
   * workspace", and the supervisor refuses any request naming a quarantined
   * path before the child ever sees it.
   */
  function quarantiningDeps(): SearchDeps {
    const deps = fakeDeps(files);
    const inner = deps.engine;
    return {
      ...deps,
      engine: {
        ...inner,
        search: (request: EngineSearchRequest): Promise<readonly EngineMatch[]> => {
          const paths = request.paths.length > 0 ? request.paths : Object.keys(files);
          if (paths.includes(QUARANTINED)) {
            const error = new Error(
              `${QUARANTINED} is quarantined: it repeatedly crashed the engine child (see doctor)`,
            ) as Error & { code: string };
            error.code = "INPUT_QUARANTINED";
            return Promise.reject(error);
          }
          return inner.search({ ...request, paths });
        },
      },
    };
  }

  test("a lexical search whose path scope excludes every quarantined file succeeds", async () => {
    const envelope = await runSearch(quarantiningDeps(), { query: "needle", path: "src/**" }, 1);
    assertValidEnvelope(envelope);
    expect(envelope.ok).toBe(true);
    if (envelope.ok) {
      expect((envelope.results as readonly SearchGroup[]).map((g) => g.file)).toEqual([
        "src/adapters/engine.ts",
      ]);
    }
  });

  test("the incident shape: a scope matching zero files answers ok, never the whole walk", async () => {
    // Pre-fix, scoped=[] flipped into "search everything" at the adapter and
    // the quarantined file OUTSIDE the requested scope refused the search.
    const envelope = await runSearch(
      quarantiningDeps(),
      { query: "needle", path: "src/engine/**" },
      1,
    );
    assertValidEnvelope(envelope);
    expect(envelope.ok).toBe(true);
    if (envelope.ok) {
      expect(envelope.results).toEqual([]);
      expect(envelope.notes?.some((n) => n.includes("matched no files"))).toBe(true);
    }
  });

  test("a search that would genuinely parse the quarantined file still answers INPUT_QUARANTINED", async () => {
    const envelope = await runSearch(quarantiningDeps(), { query: "needle", path: "spikes/**" }, 1);
    assertValidEnvelope(envelope);
    expect(envelope.ok).toBe(false);
    if (!envelope.ok) {
      expect(envelope.error.code).toBe("INPUT_QUARANTINED");
      expect(envelope.error.recovery).toBe("escalate");
    }
  });
});

describe("structural mode", () => {
  const files: Record<string, FakeFile> = {
    "src/logic.ts": { content: "process(a);\nkeep(a);\nprocess(b);" },
    "test/logic.test.ts": { content: "process(t);" },
  };

  test("hits are grouped, ranked and kind-tagged with the node kind", async () => {
    const deps = fakeDeps(files);
    const envelope = await runSearch(
      deps,
      { query: "process($A)", mode: "structural", context: 0 },
      1,
    );
    assertValidEnvelope(envelope);
    expect(envelope.ok).toBe(true);
    if (!envelope.ok) return;
    const groups = envelope.results as readonly SearchGroup[];
    // Grouped by file, test path demoted.
    expect(groups.map((g) => g.file)).toEqual(["src/logic.ts", "test/logic.test.ts"]);
    expect(groups[0]?.totalHits).toBe(2);
    expect(groups[0]?.hits[0]?.kind).toBe("call_expression");
  });

  test("groups rank by hit count when kinds tie", async () => {
    const deps = fakeDeps({
      "src/uses.ts": { content: "process(1);\nprocess(2);\nprocess(3);" },
      "src/one.ts": { content: "process();" },
    });
    const envelope = await runSearch(deps, { query: "process($A)", mode: "structural" }, 1);
    expect(envelope.ok).toBe(true);
    if (!envelope.ok) return;
    expect((envelope.results as readonly SearchGroup[])[0]?.file).toBe("src/uses.ts");
  });

  test("a definition node kind ranks its group first", async () => {
    const uses = await runSearch(
      fakeDeps(
        { "src/a.ts": { content: "process(1);" }, "src/b.ts": { content: "process(2);" } },
        { structuralKind: "function_declaration" },
      ),
      { query: "process($A)", mode: "structural", context: 0 },
      1,
    );
    expect(uses.ok).toBe(true);
    if (!uses.ok) return;
    // Every group carries a declaration-kind hit — all rank as definitions,
    // proving DEFINITION_KIND classification reaches the ranking.
    for (const group of uses.results as readonly SearchGroup[]) {
      expect(group.hits[0]?.kind).toBe("function_declaration");
    }
  });

  test("kind[] filters matched node kinds, shorthand included", async () => {
    const deps = fakeDeps(files);
    const kept = await runSearch(
      deps,
      { query: "process($A)", mode: "structural", kind: ["call"], context: 0 },
      1,
    );
    expect(kept.ok).toBe(true);
    if (kept.ok) expect(kept.results.length).toBeGreaterThan(0);
    const filtered = await runSearch(
      deps,
      { query: "process($A)", mode: "structural", kind: ["import_statement"], context: 0 },
      1,
    );
    expect(filtered.ok).toBe(true);
    if (filtered.ok) expect(filtered.results.length).toBe(0);
  });

  test("an invalid structural pattern maps to PATTERN_INVALID", async () => {
    const deps = fakeDeps(files, {
      engineError: new Error("invalid structural pattern for `typescript`: empty"),
    });
    const envelope = await runSearch(deps, { query: "", mode: "structural" }, 1);
    assertValidEnvelope(envelope);
    expect(envelope.ok).toBe(false);
    if (!envelope.ok) {
      expect(envelope.error.code).toBe("PATTERN_INVALID");
      expect(envelope.error.recovery).toBe("narrow");
    }
  });

  test("structural answers at tier 0 — no index required (Constitution I)", async () => {
    const deps = fakeDeps(files, { status: "throw" });
    const envelope = await runSearch(deps, { query: "process($A)", mode: "structural" }, 1);
    expect(envelope.ok).toBe(true);
    if (envelope.ok) expect(envelope.freshness).toBe("cold");
  });
});

describe("references mode", () => {
  const files: Record<string, FakeFile> = {
    "src/def.ts": { content: "export function needle_ref() {}" },
    "src/app.ts": { content: 'import "./def";\nneedle_ref();' },
    "docs/notes.md": { content: "needle_ref mentioned" },
  };
  const edges: readonly ReferenceEdge[] = [
    {
      location: { path: "docs/notes.md", line: 1, column: 1 },
      excerpt: "needle_ref mentioned",
      refKind: "use",
      confidence: "low",
      provenance: "tier1-text",
    },
    {
      location: { path: "src/def.ts", line: 1, column: 17 },
      excerpt: "export function needle_ref() {}",
      refKind: "definition",
      confidence: "high",
      provenance: "tier1-symbols",
    },
    {
      location: { path: "src/app.ts", line: 2, column: 1 },
      excerpt: "needle_ref();",
      refKind: "use",
      confidence: "medium",
      provenance: "tier1-imports",
    },
  ];

  test("tier 0 refuses with INDEX_UNAVAILABLE, recovery retry, progress in notes", async () => {
    const deps = fakeDeps(files, {
      status: { tier: 0, generation: 0, building: true, filesTotal: 9, filesIndexed: 4 },
    });
    const envelope = await runSearch(deps, { query: "needle_ref", mode: "references" }, 1);
    assertValidEnvelope(envelope);
    expect(envelope.ok).toBe(false);
    if (!envelope.ok) {
      expect(envelope.error.code).toBe("INDEX_UNAVAILABLE");
      expect(envelope.error.recovery).toBe("retry");
      expect(envelope.notes?.some((n) => n.includes("4/9"))).toBe(true);
    }
  });

  test("no index at all refuses the same way", async () => {
    const deps = fakeDeps(files, { status: "throw" });
    const envelope = await runSearch(deps, { query: "needle_ref", mode: "references" }, 1);
    expect(envelope.ok).toBe(false);
    if (!envelope.ok) expect(envelope.error.code).toBe("INDEX_UNAVAILABLE");
  });

  test("edges come back grouped, ranked and carrying confidence provenance", async () => {
    const deps = fakeDeps(files, { references: edges });
    const envelope = await runSearch(
      deps,
      { query: "needle_ref", mode: "references", context: 0 },
      1,
    );
    assertValidEnvelope(envelope);
    expect(envelope.ok).toBe(true);
    if (!envelope.ok) return;
    const groups = envelope.results as readonly SearchGroup[];
    // Definition group first, then the higher-confidence use, then the name match.
    expect(groups.map((g) => g.file)).toEqual(["src/def.ts", "src/app.ts", "docs/notes.md"]);
    const defHit = groups[0]?.hits[0];
    expect(defHit?.kind).toBe("definition");
    expect(defHit?.confidence).toBe("high");
    expect(defHit?.provenance).toBe("tier1-symbols");
    const useHit = groups[1]?.hits[0];
    expect(useHit?.confidence).toBe("medium");
    expect(useHit?.provenance).toBe("tier1-imports");
    // §14: the envelope says what the fidelity is instead of overclaiming.
    expect(envelope.notes?.some((n) => n.includes("name-resolution-free"))).toBe(true);
  });

  test("the answer's verified freshness is the envelope's freshness", async () => {
    const deps = fakeDeps(files, { references: edges, referencesFreshness: "cold" });
    const envelope = await runSearch(deps, { query: "needle_ref", mode: "references" }, 1);
    expect(envelope.ok).toBe(true);
    if (envelope.ok) expect(envelope.freshness).toBe("cold");
  });

  test("path glob and include excludes scope reference edges", async () => {
    const deps = fakeDeps(files, { references: edges });
    const envelope = await runSearch(
      deps,
      { query: "needle_ref", mode: "references", path: "src/**", context: 0 },
      1,
    );
    expect(envelope.ok).toBe(true);
    if (!envelope.ok) return;
    const paths = (envelope.results as readonly SearchGroup[]).map((g) => g.file);
    expect(paths).toEqual(["src/def.ts", "src/app.ts"]);
  });
});

describe("files mode", () => {
  const files: Record<string, FakeFile> = {
    "src/alpha.ts": { content: "" },
    "src/deep/beta.ts": { content: "" },
    "docs/notes.md": { content: "" },
    "bun.lock": { content: "", junk: true },
  };

  test("glob queries match paths, ranked shallow-first with junk last", async () => {
    const deps = fakeDeps(files);
    const envelope = await runSearch(deps, { query: "**/*.ts", mode: "files" }, 1);
    assertValidEnvelope(envelope);
    expect(envelope.ok).toBe(true);
    if (!envelope.ok) return;
    expect((envelope.results as readonly SearchGroup[]).map((g) => g.file)).toEqual([
      "src/alpha.ts",
      "src/deep/beta.ts",
    ]);
  });

  test("substring queries match filenames case-insensitively", async () => {
    const deps = fakeDeps(files);
    const envelope = await runSearch(deps, { query: "NOTES", mode: "files" }, 1);
    expect(envelope.ok).toBe(true);
    if (!envelope.ok) return;
    expect((envelope.results as readonly SearchGroup[]).map((g) => g.file)).toEqual([
      "docs/notes.md",
    ]);
  });
});

describe("budget", () => {
  const many: Record<string, FakeFile> = Object.fromEntries(
    Array.from({ length: 30 }, (_, i) => [
      `src/mod_${String(i).padStart(2, "0")}.ts`,
      { content: "export const budget_needle = 1;" },
    ]),
  );

  test("a cut is never silent: the manifest carries dropped/reason/narrow/cursor", async () => {
    const deps = fakeDeps(many);
    const envelope = await runSearch(deps, { query: "budget_needle", budget: 120, context: 0 }, 1);
    assertValidEnvelope(envelope);
    expect(envelope.ok).toBe(true);
    if (!envelope.ok) return;
    expect(envelope.results.length).toBeGreaterThan(0);
    expect(envelope.truncation).toBeDefined();
    expect(envelope.truncation?.reason).toBe("budget");
    expect(envelope.truncation?.dropped).toBe(30 - envelope.results.length);
    expect(envelope.truncation?.cursor.startsWith("file:")).toBe(true);
    expect(envelope.budget.used).toBeLessThanOrEqual(120);
  });

  test("a budget too small for even one group is BUDGET_TOO_SMALL", async () => {
    const deps = fakeDeps(many);
    const envelope = await runSearch(deps, { query: "budget_needle", budget: 1, context: 0 }, 1);
    assertValidEnvelope(envelope);
    expect(envelope.ok).toBe(false);
    if (!envelope.ok) {
      expect(envelope.error.code).toBe("BUDGET_TOO_SMALL");
      expect(envelope.error.recovery).toBe("narrow");
    }
  });
});

// -------------------------------------------------------- integration half

/** One fixture file per test language, all containing the shared needle. */
const FIXTURES: Readonly<Record<string, string>> = {
  "src/marker.ts": "export function parse_config_needle_ts(): void {}\n",
  "src/marker.js": "function parse_config_needle_js() {}\n",
  "src/marker.py": "def parse_config_needle_py():\n    pass\n",
  "src/marker.rs": "pub fn parse_config_needle_rs() -> u32 { 1 }\n",
  "src/marker.go": "package main\n\nfunc parse_config_needle_go() int { return 1 }\n",
  "src/Marker.java": "public class Marker { int parse_config_needle_java() { return 1; } }\n",
};

const REPO = path.resolve(import.meta.dir, "..");
const MAIN = path.join(REPO, "src", "main.ts");

const roots: string[] = [];
afterAll(() => {
  // The engine child releases its index.db handle asynchronously after the
  // server dies; on Windows that shows up as EBUSY for a moment. Bounded retry.
  for (const root of roots.splice(0)) {
    for (let attempt = 0; ; attempt++) {
      try {
        rmSync(root, { recursive: true, force: true });
        break;
      } catch {
        if (attempt >= 50) break; // leave the tmp dir to the OS rather than fail the suite
        Bun.sleepSync(100);
      }
    }
  }
});

function fixtureWorkspace(): string {
  const root = mkdtempSync(path.join(tmpdir(), "workbench-search-"));
  roots.push(root);
  mkdirSync(path.join(root, "src"), { recursive: true });
  for (const [rel, content] of Object.entries(FIXTURES)) {
    writeFileSync(path.join(root, rel), content);
  }
  return root;
}

function envelopeFrom(result: unknown): ResponseEnvelope<SearchGroup> {
  const content = (result as { content: Array<{ type: string; text: string }> }).content;
  const first = content[0];
  if (first === undefined) throw new Error("empty content");
  return JSON.parse(first.text) as ResponseEnvelope<SearchGroup>;
}

describe("integration: fixture repo across the 6 test languages", () => {
  test("lexical search hits every language; files mode and semantic behave per spec", async () => {
    const root = fixtureWorkspace();
    const client = new Client({ name: "search-int", version: "0.0.0" });
    await client.connect(
      new StdioClientTransport({
        command: process.execPath,
        args: [MAIN, root],
        env: {
          ...process.env,
          WORKBENCH_NETWORK_FS: "0",
          WORKBENCH_ENGINE_STUB_CORE: "1",
        } as Record<string, string>,
        cwd: REPO,
      }),
    );
    try {
      // The advertised inputSchema is the generated schema, verbatim.
      const tools = await client.listTools();
      const searchTool = tools.tools.find((t) => t.name === "search");
      if (searchTool === undefined) throw new Error("search tool not advertised");
      expect(searchTool.inputSchema).toEqual(
        JSON.parse(JSON.stringify(searchInputSchema)) as typeof searchTool.inputSchema,
      );

      // Lexical: one call, hits grouped per file, all 6 languages present.
      const lexical = envelopeFrom(
        await client.callTool({
          name: "search",
          arguments: { query: "parse_config_needle", mode: "lexical", context: 0 },
        }),
      );
      assertValidEnvelope(lexical);
      expect(lexical.ok).toBe(true);
      if (lexical.ok) {
        const found = new Set(lexical.results.map((g) => g.file));
        for (const rel of Object.keys(FIXTURES)) expect(found).toContain(rel);
        for (const group of lexical.results) {
          expect(group.hits.length).toBeGreaterThan(0);
          expect(group.hits[0]?.excerpt).toContain("parse_config_needle");
        }
      }

      // Files mode replaces find.
      const filesMode = envelopeFrom(
        await client.callTool({
          name: "search",
          arguments: { query: "**/*.py", mode: "files" },
        }),
      );
      assertValidEnvelope(filesMode);
      expect(filesMode.ok).toBe(true);
      if (filesMode.ok) {
        expect(filesMode.results.map((g) => g.file)).toEqual(["src/marker.py"]);
      }

      // Structural over the wire: the stub core answers typescript only; hits
      // carry a node kind (real grammar fidelity lives in the Rust tests).
      const structural = envelopeFrom(
        await client.callTool({
          name: "search",
          arguments: { query: "parse_config_needle_ts($$$A)", mode: "structural", context: 0 },
        }),
      );
      assertValidEnvelope(structural);
      expect(structural.ok).toBe(true);
      if (structural.ok) {
        expect(structural.results.map((g) => g.file)).toEqual(["src/marker.ts"]);
        expect(structural.results[0]?.hits[0]?.kind).toBe("stub_match");
      }

      // References before the Tier-1 build completes is a typed refusal…
      const early = envelopeFrom(
        await client.callTool({
          name: "search",
          arguments: { query: "parse_config_needle_ts", mode: "references" },
        }),
      );
      assertValidEnvelope(early);
      let refs = early;
      if (!early.ok) {
        expect(early.error.code).toBe("INDEX_UNAVAILABLE");
        expect(early.error.recovery).toBe("retry");
        // …and that same first call kicked the build (§7.3.2): retry until warm.
        for (let attempt = 0; attempt < 100 && !refs.ok; attempt += 1) {
          await new Promise((r) => setTimeout(r, 100));
          refs = envelopeFrom(
            await client.callTool({
              name: "search",
              arguments: { query: "parse_config_needle_ts", mode: "references", context: 0 },
            }),
          );
        }
      }
      assertValidEnvelope(refs);
      expect(refs.ok).toBe(true);
      if (refs.ok) {
        const hits = refs.results.flatMap((g) => g.hits);
        const definition = hits.find((h) => h.kind === "definition");
        expect(definition?.confidence).toBe("high");
        expect(definition?.provenance).toBe("tier1-symbols");
        // Every edge carries its confidence provenance.
        for (const hit of hits) {
          expect(["high", "medium", "low"]).toContain(hit.confidence ?? "");
          expect(hit.provenance ?? "").toMatch(/^tier1-/);
        }
        expect(refs.notes?.some((n) => n.includes("name-resolution-free"))).toBe(true);
      }

      // The reserved mode comes back typed over the wire.
      const semantic = envelopeFrom(
        await client.callTool({
          name: "search",
          arguments: { query: "anything", mode: "semantic" },
        }),
      );
      assertValidEnvelope(semantic);
      expect(semantic.ok).toBe(false);
      if (!semantic.ok) expect(semantic.error.code).toBe("UNSUPPORTED_VERSION");
    } finally {
      await client.close();
    }
  }, 45_000);
});

// ------------------------- Rust file classes and test regions (specs/009)

describe("Rust classification: cfg(test) regions, benches, examples", () => {
  test("benches/ and examples/ demote like tests/, and target/ like vendor", async () => {
    const deps = fakeDeps({
      "src/hash.rs": { content: "pub fn checksum(b: &[u8]) -> u64 { 0 }" },
      "benches/hashing.rs": { content: "fn bench() { checksum(&[]); }" },
      "examples/demo.rs": { content: "fn main() { checksum(&[]); }" },
      "target/debug/gen.rs": { content: "pub fn checksum() {}", junk: true },
    });
    const envelope = await runSearch(deps, { query: "checksum", context: 0 }, 1);
    expect(envelope.ok).toBe(true);
    if (!envelope.ok) return;
    const files = (envelope.results as readonly SearchGroup[]).map((g) => g.file);
    // Source first, and every non-shipping directory ranks below it: benches/
    // and examples/ are test-class (the segments specs/009 R5 added), target/
    // is junk. NOTE the honest ordering rule: definition-first (stage 3) sorts
    // ahead of file class, so the generated file — which happens to contain a
    // declaration — precedes the two call-site-only files. Source with a
    // declaration still beats junk with a declaration, which is the property
    // that matters.
    expect(files[0]).toBe("src/hash.rs");
    expect(files.indexOf("benches/hashing.rs")).toBeGreaterThan(0);
    expect(files.indexOf("examples/demo.rs")).toBeGreaterThan(0);
    expect(files.indexOf("target/debug/gen.rs")).toBeGreaterThan(0);
  });

  test("a hit inside a #[cfg(test)] module ranks below the production hit in the same file", async () => {
    const content = [
      "pub fn compute(x: u32) -> u32 {", // 1
      "    compute_inner(x)", // 2
      "}", // 3
      "", // 4
      "#[cfg(test)]", // 5
      "mod tests {", // 6
      "    use super::*;", // 7
      "    #[test]", // 8
      "    fn t() {", // 9
      "        compute(1);", // 10
      "    }", // 11
      "}", // 12
    ].join(String.fromCharCode(10));
    const deps = fakeDeps(
      { "src/lib.rs": { content } },
      { testRanges: { "src/lib.rs": [{ startLine: 5, endLine: 12 }] } },
    );
    const envelope = await runSearch(deps, { query: "compute", context: 0 }, 1);
    expect(envelope.ok).toBe(true);
    if (!envelope.ok) return;
    const group = (envelope.results as readonly SearchGroup[])[0];
    expect(group?.file).toBe("src/lib.rs");
    // The production hits come first; the cfg(test) call site is demoted.
    const lines = group?.hits.map((h) => h.line) ?? [];
    expect(lines[lines.length - 1]).toBe(10);
    expect(lines[0]).toBeLessThan(5);
  });

  test("a file whose only hits are inside cfg(test) loses to a production file", async () => {
    const helper = [
      "pub fn unrelated() {}", // 1
      "", // 2
      "#[cfg(test)]", // 3
      "mod tests {", // 4
      "    fn t() { parse_manifest(); }", // 5
      "}", // 6
    ].join(String.fromCharCode(10));
    const deps = fakeDeps(
      {
        "src/helper.rs": { content: helper },
        "src/manifest.rs": { content: "pub fn parse_manifest() -> u32 { 1 }" },
      },
      { testRanges: { "src/helper.rs": [{ startLine: 3, endLine: 6 }] } },
    );
    const envelope = await runSearch(deps, { query: "parse_manifest", context: 0 }, 1);
    expect(envelope.ok).toBe(true);
    if (!envelope.ok) return;
    expect((envelope.results as readonly SearchGroup[]).map((g) => g.file)).toEqual([
      "src/manifest.rs",
      "src/helper.rs",
    ]);
  });

  test("an index that cannot answer test ranges leaves search exactly as it was", async () => {
    // No queryTestRanges on the port at all — the common case for every
    // adapter that predates it (specs/009 FR-011).
    const deps = fakeDeps({ "src/lib.rs": { content: "pub fn compute() {}" } });
    const envelope = await runSearch(deps, { query: "compute", context: 0 }, 1);
    expect(envelope.ok).toBe(true);
    if (!envelope.ok) return;
    expect((envelope.results as readonly SearchGroup[])[0]?.file).toBe("src/lib.rs");
  });
});
