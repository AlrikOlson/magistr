/**
 * The `map` tool (chunk m1-map, SDD §9.2).
 *
 * runMap against fake ports: a useful Tier-0 skeleton with no index at all
 * (the chunk's first acceptance criterion), ttlMs keyed to index generation
 * (second), sideloaded plugins labeled untrusted (third), plus depth pruning,
 * detail levels, verified-outline attachment, freshness demotion, budget
 * trims with the full manifest, and BUDGET_TOO_SMALL refusal. Every envelope
 * is validated against the normative schema.
 */

import { describe, expect, test } from "bun:test";
import { mkdirSync, readFileSync, writeFileSync } from "node:fs";
import path from "node:path";
import envelopeSchema from "../schemas/envelope.schema.json";
import type { ResponseEnvelope } from "../src/envelope";
import type {
  CrateScope,
  ImportGraphResult,
  IndexPort,
  IndexStatus,
  OutlineQueryResult,
  ReferenceQueryResult,
  SymbolQueryResult,
} from "../src/ports";
import { PORT_VERSIONS } from "../src/ports";
import {
  type InstalledPlugin,
  MAP_CRATE_ENTRY_POINTS_MAX,
  MAP_CRATE_FEATURES_MAX,
  MAP_CRATE_MANIFESTS_MAX,
  MAP_CRATE_ROWS_MAX,
  MAP_IMPORT_GRAPH_TOP,
  MAP_OUTLINE_SYMBOL_CAP,
  MAP_TTL_COLD_MS,
  MAP_TTL_WARM_MS,
  type MapCrate,
  type MapDeps,
  type MapEntry,
  type MapInput,
  type MapOverview,
  type MapResult,
  runMap,
} from "../src/tools";
import { envelopeValidatorFor } from "./support/ajv";

const { validate, errorsText } = envelopeValidatorFor(envelopeSchema);

function assertValidEnvelope(envelope: ResponseEnvelope<unknown>): void {
  expect(validate(envelope) ? "" : errorsText()).toBe("");
}

interface FakeOptions {
  /** "throw" = no index exists at all (the pure Tier-0 workspace). */
  status?: IndexStatus | "throw";
  /** Symbols per path answered by queryOutline. */
  outlines?: Record<string, readonly { name: string; kind: string; line: number; doc?: string }[]>;
  outlineFreshness?: OutlineQueryResult["freshness"];
  outlineError?: Error;
  /** Answer for queryImportGraph; default empty (nothing to summarize). */
  importGraph?: Pick<ImportGraphResult, "fanIn" | "fanOut">;
  importGraphFreshness?: ImportGraphResult["freshness"];
  importGraphError?: Error;
  /** Per-crate edges answered beside the summary (specs/014); absent = silent adapter. */
  crates?: ImportGraphResult["crates"];
  plugins?: readonly InstalledPlugin[];
  pluginsError?: Error;
}

function fakeDeps(
  files: Record<string, { junk?: boolean; content?: string }>,
  options: FakeOptions = {},
): MapDeps & {
  outlineCalls: string[][];
  importGraphCalls: (number | undefined)[];
  crateScopesSeen: CrateScope[][];
} {
  const outlineCalls: string[][] = [];
  const importGraphCalls: (number | undefined)[] = [];
  const crateScopesSeen: CrateScope[][] = [];
  const index: IndexPort = {
    version: PORT_VERSIONS.index,
    status: (): Promise<IndexStatus> =>
      options.status === "throw"
        ? Promise.reject(new Error("no index exists yet"))
        : Promise.resolve(options.status ?? { tier: 1, generation: 1, building: false }),
    build: () => Promise.reject(new Error("not under test")),
    querySymbols: (): Promise<SymbolQueryResult> =>
      Promise.resolve({ symbols: [], freshness: "warm" }),
    queryReferences: (): Promise<ReferenceQueryResult> =>
      Promise.resolve({ edges: [], freshness: "warm" }),
    queryOutline: ({ paths }): Promise<OutlineQueryResult> => {
      outlineCalls.push([...paths]);
      if (options.outlineError) return Promise.reject(options.outlineError);
      return Promise.resolve({
        files: paths.map((p) => ({
          path: p,
          symbols: (options.outlines?.[p] ?? []).map((s) => ({
            name: s.name,
            kind: s.kind,
            location: { path: p, line: s.line, column: 1 },
            ...(s.doc === undefined ? {} : { doc: s.doc }),
          })),
        })),
        freshness: options.outlineFreshness ?? "warm",
      });
    },
    queryImportGraph: ({ limit, crates }): Promise<ImportGraphResult> => {
      importGraphCalls.push(limit);
      crateScopesSeen.push([...(crates ?? [])]);
      if (options.importGraphError) return Promise.reject(options.importGraphError);
      return Promise.resolve({
        fanIn: options.importGraph?.fanIn ?? [],
        fanOut: options.importGraph?.fanOut ?? [],
        ...(options.crates === undefined ? {} : { crates: options.crates }),
        freshness: options.importGraphFreshness ?? "warm",
      });
    },
    upsertFile: () => Promise.resolve(),
    refreshFile: () => Promise.resolve(),
    removeFile: () => Promise.resolve(),
  };
  return {
    index,
    workspace: {
      list: () =>
        Promise.resolve(
          Object.entries(files).map(([path, f]) => ({ path, junk: f.junk ?? false })),
        ),
      // map reads only walk-listed manifests (entry-point detection).
      read: (path: string) => {
        const content = files[path]?.content;
        return content === undefined
          ? Promise.reject(new Error(`no content faked for ${path}`))
          : Promise.resolve(content);
      },
    },
    plugins: () =>
      options.pluginsError
        ? Promise.reject(options.pluginsError)
        : Promise.resolve(options.plugins ?? []),
    outlineCalls,
    importGraphCalls,
    crateScopesSeen,
  };
}

async function ok(deps: MapDeps, input: MapInput = {}): Promise<readonly MapResult[]> {
  const envelope = await runMap(deps, input, 1);
  assertValidEnvelope(envelope);
  expect(envelope.ok).toBe(true);
  if (!envelope.ok) throw new Error("unreachable");
  return envelope.results;
}

function overviewOf(results: readonly MapResult[]): MapOverview {
  const first = results[0];
  if (first?.kind !== "overview") throw new Error("first row must be the overview");
  return first;
}

function entriesOf(results: readonly MapResult[]): readonly MapEntry[] {
  return results.filter((r): r is MapEntry => r.kind !== "overview");
}

const WORKSPACE = {
  "src/index.ts": {},
  "src/tools/map.ts": {},
  "src/tools/deep/nested/leaf.ts": {},
  "test/map.test.ts": {},
  "node_modules/dep/index.js": { junk: true },
  "README.md": {},
};

describe("Tier-0 skeleton (no index present)", () => {
  test("answers a useful skeleton: dirs with counts, files, cold, no outlines", async () => {
    const deps = fakeDeps(WORKSPACE, { status: "throw" });
    const envelope = await runMap(deps, {}, 1);
    assertValidEnvelope(envelope);
    expect(envelope.ok).toBe(true);
    if (!envelope.ok) return;
    expect(envelope.freshness).toBe("cold");

    const overview = overviewOf(envelope.results);
    expect(overview.index).toBeNull();
    expect(overview.files).toBe(6);
    expect(overview.languages["typescript"]).toBe(4);
    expect(overview.languages["markdown"]).toBe(1);

    const entries = entriesOf(envelope.results);
    const src = entries.find((e) => e.path === "src");
    expect(src?.kind).toBe("dir");
    expect(src?.files).toBe(3); // deep files stay counted even when folded
    const junk = entries.find((e) => e.path === "node_modules/dep/index.js");
    expect(junk?.junk).toBe(true);
    // No index: nothing carries symbols, and the walk was never index-blocked.
    expect(entries.every((e) => e.symbols === undefined)).toBe(true);
    expect(deps.outlineCalls).toHaveLength(0);
    expect(envelope.notes?.join(" ")).toContain("Tier-1 index");
  });

  // chunk:read-walker-gitignore: extension comes from the BASENAME only.
  // A dot in a directory name must never mint a census key, and dotfiles /
  // extensionless files census as nothing at all.
  test("census keys never derive from directory dots, dotfiles or extensionless names", async () => {
    const deps = fakeDeps(
      {
        "src/index.ts": {},
        "sysroot-33.0+m/include/__bit_reference": {},
        "sysroot-33.0+m/include/limits.h": {},
        ".gitattributes": {},
        Makefile: {},
        "notes.": {},
      },
      { status: "throw" },
    );
    const envelope = await runMap(deps, {}, 1);
    assertValidEnvelope(envelope);
    expect(envelope.ok).toBe(true);
    if (!envelope.ok) return;
    const overview = overviewOf(envelope.results);
    expect(overview.languages).toEqual({ typescript: 1, c: 1 });
  });

  test("breadth-first order: shallower rows precede deeper, dirs before files", async () => {
    const deps = fakeDeps(WORKSPACE, { status: "throw" });
    const entries = entriesOf(await ok(deps, { depth: 3 }));
    const depths = entries.map((e) => e.path.split("/").length);
    expect(depths).toEqual([...depths].sort((a, b) => a - b));
    const level1 = entries.filter((e) => e.path.split("/").length === 1);
    const firstFile = level1.findIndex((e) => e.kind === "file");
    const lastDir = level1.map((e) => e.kind).lastIndexOf("dir");
    expect(lastDir).toBeLessThan(firstFile === -1 ? level1.length : firstFile);
  });
});

describe("ttlMs keyed to index generation", () => {
  test("warm index: long-lived answer, generation rides beside it", async () => {
    const deps = fakeDeps(WORKSPACE, { status: { tier: 1, generation: 42, building: false } });
    const overview = overviewOf(await ok(deps));
    expect(overview.ttlMs).toBe(MAP_TTL_WARM_MS);
    expect(overview.index?.generation).toBe(42);
  });

  test("building or absent index: short TTL", async () => {
    const building = fakeDeps(WORKSPACE, { status: { tier: 1, generation: 7, building: true } });
    expect(overviewOf(await ok(building)).ttlMs).toBe(MAP_TTL_COLD_MS);
    const none = fakeDeps(WORKSPACE, { status: "throw" });
    expect(overviewOf(await ok(none)).ttlMs).toBe(MAP_TTL_COLD_MS);
  });
});

describe("plugin inventory trust labels (SDD §8.4)", () => {
  test("sideloaded plugins are labeled untrusted; registry keeps its tier", async () => {
    const deps = fakeDeps(WORKSPACE, {
      status: "throw",
      plugins: [
        { name: "rustls-pack", source: "registry", tier: "blessed" },
        { name: "fmt-extra", source: "registry" },
        { name: "local-hack", source: "sideload", version: "0.1.0" },
      ],
    });
    const envelope = await runMap(deps, {}, 1);
    assertValidEnvelope(envelope);
    if (!envelope.ok) throw new Error("expected ok");
    const { plugins } = overviewOf(envelope.results);
    expect(plugins).toEqual([
      { name: "rustls-pack", source: "registry", trust: "blessed" },
      { name: "fmt-extra", source: "registry", trust: "verified" },
      { name: "local-hack", version: "0.1.0", source: "sideload", trust: "untrusted" },
    ]);
    expect(envelope.notes?.join(" ")).toContain("local-hack");
    expect(envelope.notes?.join(" ")).toContain("untrusted");
  });

  test("an unreadable inventory degrades to empty, with a note", async () => {
    const deps = fakeDeps(WORKSPACE, { status: "throw", pluginsError: new Error("bad json") });
    const envelope = await runMap(deps, {}, 1);
    assertValidEnvelope(envelope);
    if (!envelope.ok) throw new Error("expected ok");
    expect(overviewOf(envelope.results).plugins).toEqual([]);
    expect(envelope.notes?.join(" ")).toContain("plugin inventory unreadable");
  });
});

describe("outlines when the index is up", () => {
  test("file rows carry verified symbols at detail:outline", async () => {
    const deps = fakeDeps(WORKSPACE, {
      outlines: {
        "src/index.ts": [{ name: "main", kind: "function", line: 3 }],
      },
    });
    const entries = entriesOf(await ok(deps, { detail: "outline" }));
    const row = entries.find((e) => e.path === "src/index.ts");
    expect(row?.symbols).toEqual([{ name: "main", kind: "function", line: 3 }]);
    const bare = entries.find((e) => e.path === "README.md");
    expect(bare?.symbols).toBeUndefined();
  });

  test("detail:skeleton never queries the index for outlines", async () => {
    const deps = fakeDeps(WORKSPACE, {});
    entriesOf(await ok(deps, { detail: "skeleton" }));
    expect(deps.outlineCalls).toHaveLength(0);
  });

  test("outline caps at detail:outline and lifts at detail:full", async () => {
    const many = Array.from({ length: MAP_OUTLINE_SYMBOL_CAP + 5 }, (_, i) => ({
      name: `sym${i}`,
      kind: "function",
      line: i + 1,
    }));
    const deps = fakeDeps(WORKSPACE, { outlines: { "src/index.ts": many } });
    const capped = entriesOf(await ok(deps, { detail: "outline", budget: 100_000 })).find(
      (e) => e.path === "src/index.ts",
    );
    expect(capped?.symbols).toHaveLength(MAP_OUTLINE_SYMBOL_CAP);
    expect(capped?.droppedSymbols).toBe(5);
    const full = entriesOf(await ok(deps, { detail: "full", budget: 100_000 })).find(
      (e) => e.path === "src/index.ts",
    );
    expect(full?.symbols).toHaveLength(MAP_OUTLINE_SYMBOL_CAP + 5);
    expect(full?.droppedSymbols).toBeUndefined();
  });

  test("a cold outline answer demotes the envelope's freshness", async () => {
    const deps = fakeDeps(WORKSPACE, { outlineFreshness: "cold" });
    const envelope = await runMap(deps, {}, 1);
    assertValidEnvelope(envelope);
    if (!envelope.ok) throw new Error("expected ok");
    expect(envelope.freshness).toBe("cold");
  });

  test("a failing outline query degrades to the skeleton with a note", async () => {
    const deps = fakeDeps(WORKSPACE, { outlineError: new Error("engine died") });
    const envelope = await runMap(deps, {}, 1);
    assertValidEnvelope(envelope);
    if (!envelope.ok) throw new Error("expected ok");
    expect(entriesOf(envelope.results).length).toBeGreaterThan(0);
    expect(envelope.notes?.join(" ")).toContain("outlines unavailable");
  });
});

describe("import-graph summary on the overview (SDD §9.2)", () => {
  const GRAPH = {
    fanIn: [
      { module: "../ports", count: 9 },
      { module: "node:fs", count: 4 },
    ],
    fanOut: [{ module: "src/tools/map.ts", count: 12 }],
  };

  test("overview carries top fan-in/fan-out when the index answers", async () => {
    const deps = fakeDeps(WORKSPACE, { importGraph: GRAPH });
    const overview = overviewOf(await ok(deps));
    expect(overview.importGraph).toEqual(GRAPH);
    expect(deps.importGraphCalls).toEqual([MAP_IMPORT_GRAPH_TOP]);
  });

  test("no index at all: never queried, overview carries no summary", async () => {
    const deps = fakeDeps(WORKSPACE, { status: "throw" });
    const overview = overviewOf(await ok(deps));
    expect(overview.importGraph).toBeUndefined();
    expect(deps.importGraphCalls).toHaveLength(0);
  });

  test("an edgeless answer is omitted, not attached empty", async () => {
    const deps = fakeDeps(WORKSPACE, {});
    const overview = overviewOf(await ok(deps));
    expect(overview.importGraph).toBeUndefined();
  });

  test("a failing import-graph query degrades with a note", async () => {
    const deps = fakeDeps(WORKSPACE, { importGraphError: new Error("engine died") });
    const envelope = await runMap(deps, {}, 1);
    assertValidEnvelope(envelope);
    if (!envelope.ok) throw new Error("expected ok");
    expect(overviewOf(envelope.results).importGraph).toBeUndefined();
    expect(entriesOf(envelope.results).length).toBeGreaterThan(0);
    expect(envelope.notes?.join(" ")).toContain("import-graph summary unavailable");
  });

  test("a cold import-graph answer demotes the envelope's freshness", async () => {
    const deps = fakeDeps(WORKSPACE, { importGraph: GRAPH, importGraphFreshness: "cold" });
    const envelope = await runMap(deps, {}, 1);
    assertValidEnvelope(envelope);
    if (!envelope.ok) throw new Error("expected ok");
    expect(envelope.freshness).toBe("cold");
  });
});

describe("entry-point detection (SDD §9.2, Tier-0 capable)", () => {
  test("package.json main/bin, Cargo [[bin]], and src/main.* with no index", async () => {
    const deps = fakeDeps(
      {
        ...WORKSPACE,
        "src/main.ts": {},
        "package.json": {
          content: JSON.stringify({
            name: "acme",
            main: "./dist/index.js",
            bin: { acme: "./bin/acme.js" },
          }),
        },
        "Cargo.toml": {
          content: '[package]\nname = "acme"\n\n[[bin]]\nname = "acme-cli"\npath = "src/cli.rs"\n',
        },
      },
      { status: "throw" },
    );
    const overview = overviewOf(await ok(deps));
    expect(overview.entryPoints).toEqual([
      { path: "dist/index.js", kind: "package-main" },
      { path: "bin/acme.js", kind: "package-bin", name: "acme" },
      { path: "src/cli.rs", kind: "cargo-bin", name: "acme-cli" },
      { path: "src/main.ts", kind: "convention" },
    ]);
  });

  test("a string bin wears the package name; a pathless [[bin]] follows convention", async () => {
    const deps = fakeDeps(
      {
        "package.json": { content: JSON.stringify({ name: "solo", bin: "./cli.js" }) },
        "Cargo.toml": { content: '[[bin]]\nname = "tool"\n' },
      },
      { status: "throw" },
    );
    const overview = overviewOf(await ok(deps));
    expect(overview.entryPoints).toEqual([
      { path: "cli.js", kind: "package-bin", name: "solo" },
      { path: "src/bin/tool.rs", kind: "cargo-bin", name: "tool" },
    ]);
  });

  test("duplicate paths dedupe; the manifest's row wins over convention", async () => {
    const deps = fakeDeps(
      {
        "src/main.ts": {},
        "package.json": { content: JSON.stringify({ main: "src/main.ts" }) },
      },
      { status: "throw" },
    );
    const overview = overviewOf(await ok(deps));
    expect(overview.entryPoints).toEqual([{ path: "src/main.ts", kind: "package-main" }]);
  });

  test("malformed or unreadable manifests detect nothing and stay ok", async () => {
    const deps = fakeDeps(
      {
        "package.json": { content: "not json {" },
        "Cargo.toml": {}, // listed but unreadable (no faked content)
        "README.md": {},
      },
      { status: "throw" },
    );
    const envelope = await runMap(deps, {}, 1);
    assertValidEnvelope(envelope);
    if (!envelope.ok) throw new Error("expected ok");
    expect(overviewOf(envelope.results).entryPoints).toBeUndefined();
  });

  test("manifests are read only when the walk listed them", async () => {
    const deps = fakeDeps(WORKSPACE, { status: "throw" });
    // WORKSPACE has no manifests and no src/main.*: nothing detected, no reads.
    const overview = overviewOf(await ok(deps));
    expect(overview.entryPoints).toBeUndefined();
  });
});

describe("depth and path scoping", () => {
  test("depth folds deeper files into ancestor counts", async () => {
    const deps = fakeDeps(WORKSPACE, { status: "throw" });
    const entries = entriesOf(await ok(deps, { depth: 2 }));
    expect(entries.find((e) => e.path === "src/tools/deep/nested/leaf.ts")).toBeUndefined();
    const tools = entries.find((e) => e.path === "src/tools");
    expect(tools?.files).toBe(2);
  });

  test("path scopes the tree; counts and census follow", async () => {
    const deps = fakeDeps(WORKSPACE, { status: "throw" });
    const results = await ok(deps, { path: "src", depth: 16 });
    const overview = overviewOf(results);
    expect(overview.path).toBe("src");
    expect(overview.files).toBe(3);
    expect(entriesOf(results).every((e) => e.path.startsWith("src"))).toBe(true);
  });

  test("a path with no files answers an empty tree plus a note", async () => {
    const deps = fakeDeps(WORKSPACE, { status: "throw" });
    const envelope = await runMap(deps, { path: "no/such/dir" }, 1);
    assertValidEnvelope(envelope);
    if (!envelope.ok) throw new Error("expected ok");
    expect(entriesOf(envelope.results)).toHaveLength(0);
    expect(envelope.notes?.join(" ")).toContain("no files under");
  });
});

describe("budget", () => {
  test("a cut keeps the shallow head and carries the full manifest", async () => {
    const deps = fakeDeps(WORKSPACE, { status: "throw" });
    const envelope = await runMap(deps, { budget: 120 }, 1);
    assertValidEnvelope(envelope);
    if (!envelope.ok) throw new Error("expected ok");
    expect(envelope.results.length).toBeGreaterThan(0);
    expect(envelope.results[0]?.kind).toBe("overview");
    expect(envelope.truncation).toBeDefined();
    expect(envelope.truncation?.reason).toBe("budget");
    expect(envelope.truncation?.dropped).toBeGreaterThan(0);
    expect(envelope.truncation?.narrow.length).toBeGreaterThan(0);
    expect(envelope.truncation?.cursor.startsWith("path:")).toBe(true);
    expect(envelope.budget.used).toBeLessThanOrEqual(120);
  });

  test("a budget too small for even the overview refuses with the taxonomy code", async () => {
    const deps = fakeDeps(WORKSPACE, { status: "throw" });
    const envelope = await runMap(deps, { budget: 1 }, 1);
    assertValidEnvelope(envelope);
    expect(envelope.ok).toBe(false);
    if (envelope.ok) return;
    expect(envelope.error.code).toBe("BUDGET_TOO_SMALL");
  });
});

// ------------------------------ Rust honesty + outlines (specs/009 US3/US4)

describe("map: Rust index honesty and doc summaries", () => {
  test("a Rust workspace is told macro-generated items are not indexed", async () => {
    const results = await ok(fakeDeps({ "src/lib.rs": {}, "Cargo.toml": {} }), { detail: "tree" });
    void results;
    const envelope = await runMap(fakeDeps({ "src/lib.rs": {}, "Cargo.toml": {} }), {}, 1);
    if (!envelope.ok) throw new Error("unreachable");
    const notes = envelope.notes?.join(" ") ?? "";
    expect(notes).toContain("macro-generated items");
    // The note names the fallback rather than leaving the agent stuck.
    expect(notes).toContain("mode:lexical");
  });

  test("a workspace without Rust carries no such note", async () => {
    const envelope = await runMap(fakeDeps({ "src/app.ts": {}, "package.json": {} }), {}, 1);
    if (!envelope.ok) throw new Error("unreachable");
    expect(envelope.notes?.join(" ") ?? "").not.toContain("macro-generated");
  });

  test("outline rows carry doc summaries when the index has them, and omit them otherwise", async () => {
    const results = await ok(
      fakeDeps(
        { "src/lib.rs": {} },
        {
          outlines: {
            "src/lib.rs": [
              { name: "Config", kind: "struct", line: 1, doc: "The configuration." },
              { name: "Config::load", kind: "method", line: 4 },
            ],
          },
        },
      ),
      { detail: "outline" },
    );
    const file = entriesOf(results).find((e) => e.path === "src/lib.rs");
    expect(file?.symbols?.[0]).toEqual({
      name: "Config",
      kind: "struct",
      line: 1,
      doc: "The configuration.",
    });
    expect(file?.symbols?.[1]).toEqual({ name: "Config::load", kind: "method", line: 4 });
    expect(Object.hasOwn(file?.symbols?.[1] ?? {}, "doc")).toBe(false);
  });
});

// ------------------------------ Cargo workspace rows (specs/014)

const CARGO_GOLDEN_DIR = path.join(import.meta.dir, "support", "golden", "map");

function cargoGolden(name: string, actual: unknown): void {
  const file = path.join(CARGO_GOLDEN_DIR, `${name}.expected.json`);
  const plain = JSON.parse(JSON.stringify(actual));
  if (process.env.UPDATE_GOLDEN === "1") {
    mkdirSync(CARGO_GOLDEN_DIR, { recursive: true });
    writeFileSync(file, `${JSON.stringify(plain, null, 2)}\n`);
    return;
  }
  expect(plain).toEqual(JSON.parse(readFileSync(file, "utf8")));
}

/** A workspace exercising every rule the spec names (specs/014 R10). */
const CARGO_WORKSPACE: Record<string, { junk?: boolean; content?: string }> = {
  "Cargo.toml": {
    content: [
      "[workspace]",
      'members = ["crates/*", "tools/cli", "vendor/**"]',
      'exclude = ["crates/skipme"]',
      "",
      "[workspace.dependencies]",
      'alpha = { path = "crates/alpha" }',
      'serde = "1"',
      "",
      "[package]",
      'name = "rootpkg"',
      'version = "0.1.0"',
      'edition = "2024"',
      "",
      "[dependencies]",
      'beta-crate = { path = "crates/beta" }',
      "",
      "[[bin]]",
      'name = "root-tool"',
      'path = "tools/root-tool.rs"',
      "",
    ].join("\n"),
  },
  "tools/root-tool.rs": {},
  "src/lib.rs": {},
  "crates/alpha/Cargo.toml": {
    content: [
      "[package]",
      'name = "alpha"',
      'version = "0.1.0"',
      'edition = "2021"',
      "",
      "[features]",
      'default = ["fast"]',
      "fast = []",
      "slow = []",
      "",
      "[dependencies]",
      'serde = { version = "1", features = ["derive"] }',
      "",
    ].join("\n"),
  },
  "crates/alpha/src/lib.rs": {},
  "crates/alpha/src/main.rs": {},
  "crates/alpha/src/bin/alpha-cli.rs": {},
  "crates/alpha/examples/demo.rs": {},
  "crates/alpha/benches/speed.rs": {},
  "crates/alpha/tests/integration.rs": {},
  "crates/alpha/build.rs": {},
  "crates/alpha/src/nested/not_a_root.rs": {},
  "crates/beta/Cargo.toml": {
    content: [
      "[package]",
      'name = "beta-crate"',
      'version = "0.1.0"',
      "",
      "[dependencies]",
      "alpha = { workspace = true }",
      "serde = { workspace = true }",
      "",
      "[dev-dependencies]",
      'macros = { path = "../macros" }',
      "",
      "[build-dependencies]",
      'cli = { path = "../../tools/cli", package = "cli" }',
      "",
    ].join("\n"),
  },
  "crates/beta/src/lib.rs": {},
  "crates/macros/Cargo.toml": {
    content: [
      "[package]",
      'name = "macros"',
      'version = "0.1.0"',
      "",
      "[lib]",
      "proc-macro = true",
      "",
      "[dependencies]",
      'syn = "2"',
      'quote = "1"',
      "",
    ].join("\n"),
  },
  "crates/macros/src/lib.rs": {},
  "crates/skipme/Cargo.toml": { content: '[package]\nname = "skipme"\n' },
  "crates/skipme/src/lib.rs": {},
  "crates/notacrate/README.md": {},
  "tools/cli/Cargo.toml": {
    content: [
      "[package]",
      'name = "cli"',
      'version = "0.1.0"',
      "",
      "[dependencies]",
      'alpha = { path = "../../crates/alpha" }',
      'renamed = { package = "beta-crate", path = "../../crates/beta" }',
      "",
      "[target.'cfg(windows)'.dependencies]",
      'windows-sys = "0.61"',
      "",
    ].join("\n"),
  },
  "tools/cli/src/main.rs": {},
  "README.md": {},
};

/** Edges the index would answer for the fixture: beta uses alpha from two
 * files, cli uses alpha from one and beta_crate from one; nobody uses macros
 * or root's beta dependency. */
const CARGO_EDGES = [
  { from: "crates/beta", to: "alpha", files: 2 },
  { from: "tools/cli", to: "alpha", files: 1 },
  { from: "tools/cli", to: "beta_crate", files: 1 },
  // A self edge (alpha's integration tests import alpha): never a dependency.
  { from: "crates/alpha", to: "alpha", files: 1 },
];

function cratesOf(results: readonly MapResult[]): readonly MapCrate[] {
  return results.filter((r): r is MapCrate => r.kind === "crate");
}

describe("map: Cargo workspace rows (specs/014)", () => {
  test("crate rows between the overview and the tree, ranked by dependents (golden)", async () => {
    const deps = fakeDeps(CARGO_WORKSPACE, { crates: CARGO_EDGES });
    const results = await ok(deps, { detail: "skeleton" });
    const overview = overviewOf(results);
    const crates = cratesOf(results);
    // Position: every crate row precedes every tree row.
    const kinds = results.map((r) => r.kind);
    expect(kinds.lastIndexOf("crate")).toBeLessThan(kinds.indexOf("dir"));
    // Members: glob-expanded, excluded member gone, no-manifest dir gone,
    // explicit member and the root package present.
    expect(crates.map((c) => c.name)).toEqual(["alpha", "beta-crate", "cli", "macros", "rootpkg"]);
    expect(crates.map((c) => c.dependents)).toEqual([2, 2, 1, 1, 0]);
    const alpha = crates[0] as MapCrate;
    expect(alpha.path).toBe("crates/alpha");
    expect(alpha.edition).toBe("2021");
    expect(alpha.roles).toEqual(["lib", "bin", "build"]);
    expect(alpha.entryPoints).toEqual([
      { path: "crates/alpha/benches/speed.rs", kind: "bench", name: "speed" },
      { path: "crates/alpha/build.rs", kind: "build" },
      { path: "crates/alpha/examples/demo.rs", kind: "example", name: "demo" },
      { path: "crates/alpha/src/bin/alpha-cli.rs", kind: "bin", name: "alpha-cli" },
      { path: "crates/alpha/src/lib.rs", kind: "lib" },
      { path: "crates/alpha/src/main.rs", kind: "bin" },
      { path: "crates/alpha/tests/integration.rs", kind: "test", name: "integration" },
    ]);
    expect(alpha.features).toEqual(["default", "fast", "slow"]);
    expect(alpha.dependencies).toEqual([]);
    expect(alpha.externalDependencies).toBe(1);
    // beta: workspace = true resolves through [workspace.dependencies]; dev
    // and build kinds; the package rename; use counts attached.
    const beta = crates[1] as MapCrate;
    expect(beta.name).toBe("beta-crate");
    expect(beta.dependencies).toEqual([
      { name: "alpha", files: 2 },
      { name: "macros", kind: "dev", files: 0 },
      { name: "cli", kind: "build", files: 0 },
    ]);
    expect(beta.externalDependencies).toBe(1); // serde via workspace = true
    const cli = crates[2] as MapCrate;
    expect(cli.roles).toEqual(["bin"]);
    expect(cli.dependencies).toEqual([
      { name: "alpha", files: 1 },
      { name: "beta-crate", files: 1 },
    ]);
    expect(cli.externalDependencies).toBe(1); // the target-scoped windows-sys
    const macros = crates[3] as MapCrate;
    expect(macros.roles).toEqual(["proc-macro"]);
    expect(macros.externalDependencies).toBe(2);
    const root = crates[4] as MapCrate;
    expect(root.path).toBe(".");
    expect(root.edition).toBe("2024");
    // The manifest target comes first with its name; the root package owns
    // only files no member owns.
    expect(root.entryPoints).toEqual([
      { path: "tools/root-tool.rs", kind: "bin", name: "root-tool" },
      { path: "src/lib.rs", kind: "lib" },
    ]);
    expect(root.dependencies).toEqual([{ name: "beta-crate", files: 0 }]);
    // The summary describes the whole workspace.
    expect(overview.cargo).toEqual({
      kind: "workspace",
      members: 5,
      edges: 6,
      unusedDeps: 3,
      top: [
        { name: "alpha", dependents: 2 },
        { name: "beta-crate", dependents: 2 },
        { name: "cli", dependents: 1 },
        { name: "macros", dependents: 1 },
        { name: "rootpkg", dependents: 0 },
      ],
    });
    // The index was asked ONCE, with one scope per member and use names.
    expect(deps.importGraphCalls).toHaveLength(1);
    expect(deps.crateScopesSeen[0]).toEqual([
      { name: "rootpkg", dir: "." },
      { name: "alpha", dir: "crates/alpha" },
      { name: "beta_crate", dir: "crates/beta" },
      { name: "macros", dir: "crates/macros" },
      { name: "cli", dir: "tools/cli" },
    ]);
    // The unexpanded glob is reported, not dropped silently.
    const envelope = await runMap(deps, { detail: "skeleton" }, 1);
    if (!envelope.ok) throw new Error("unreachable");
    expect(envelope.notes?.join(" ")).toContain("vendor/**");
    cargoGolden("cargo-workspace", { cargo: overview.cargo, crates });
  });

  test("path scopes the crate rows; the summary still covers the workspace", async () => {
    const results = await ok(fakeDeps(CARGO_WORKSPACE, { crates: CARGO_EDGES }), {
      path: "crates",
      detail: "skeleton",
    });
    expect(cratesOf(results).map((c) => c.name)).toEqual(["alpha", "beta-crate", "macros"]);
    expect(overviewOf(results).cargo?.members).toBe(5);
    expect(overviewOf(results).cargo?.omitted).toBe(2);
    const one = await ok(fakeDeps(CARGO_WORKSPACE, { crates: CARGO_EDGES }), {
      path: "tools/cli",
      detail: "skeleton",
    });
    expect(cratesOf(one).map((c) => c.name)).toEqual(["cli"]);
  });

  test("use counts degrade to manifest facts with a note when the index cannot answer", async () => {
    // A silent adapter (no `crates` in the answer): rows without files, a note.
    const silent = fakeDeps(CARGO_WORKSPACE);
    const silentEnvelope = await runMap(silent, { detail: "skeleton" }, 1);
    assertValidEnvelope(silentEnvelope);
    if (!silentEnvelope.ok) throw new Error("unreachable");
    const beta = cratesOf(silentEnvelope.results).find((c) => c.name === "beta-crate");
    expect(beta?.dependencies).toEqual([
      { name: "alpha" },
      { name: "macros", kind: "dev" },
      { name: "cli", kind: "build" },
    ]);
    expect(Object.hasOwn(overviewOf(silentEnvelope.results).cargo ?? {}, "unusedDeps")).toBe(false);
    expect(silentEnvelope.notes?.join(" ")).toContain("use counts unavailable");
    // A throwing index: the same rows, both notes.
    const failing = fakeDeps(CARGO_WORKSPACE, { importGraphError: new Error("down") });
    const failingEnvelope = await runMap(failing, { detail: "skeleton" }, 1);
    if (!failingEnvelope.ok) throw new Error("unreachable");
    expect(cratesOf(failingEnvelope.results)).toHaveLength(5);
    expect(failingEnvelope.notes?.join(" ")).toContain("use counts unavailable");
    expect(failingEnvelope.notes?.join(" ")).toContain("import-graph summary unavailable");
    // No index at all: never asked, rows still there (Tier 0).
    const cold = fakeDeps(CARGO_WORKSPACE, { status: "throw" });
    const coldResults = await ok(cold, { detail: "skeleton" });
    expect(cratesOf(coldResults)).toHaveLength(5);
    expect(cold.importGraphCalls).toHaveLength(0);
    expect(overviewOf(coldResults).cargo?.kind).toBe("workspace");
  });

  test("64 members: 24 ranked rows, the omitted count, the tree still present; a small budget carries the manifest", async () => {
    const files: Record<string, { content?: string }> = {
      "Cargo.toml": { content: '[workspace]\nmembers = ["crates/*"]\n' },
      "README.md": {},
    };
    for (let i = 0; i < 64; i++) {
      const name = `m${String(i).padStart(2, "0")}`;
      // m00 is depended upon by everyone else: it must rank first.
      const deps = i === 0 ? "" : '[dependencies]\nm00 = { path = "../m00" }\n';
      files[`crates/${name}/Cargo.toml`] = { content: `[package]\nname = "${name}"\n${deps}` };
      files[`crates/${name}/src/lib.rs`] = {};
    }
    const results = await ok(fakeDeps(files, { status: "throw" }), { detail: "skeleton" });
    const crates = cratesOf(results);
    expect(crates).toHaveLength(MAP_CRATE_ROWS_MAX);
    expect(crates[0]?.name).toBe("m00");
    expect(crates[0]?.dependents).toBe(63);
    expect(crates[1]?.name).toBe("m01");
    expect(overviewOf(results).cargo?.omitted).toBe(64 - MAP_CRATE_ROWS_MAX);
    expect(results.some((r) => r.kind === "dir")).toBe(true);
    const envelope = await runMap(fakeDeps(files, { status: "throw" }), { detail: "skeleton" }, 1);
    if (!envelope.ok) throw new Error("unreachable");
    expect(envelope.notes?.join(" ")).toContain("map path:<member dir>");
    // Budget: the cut lands inside the crate rows with a path: cursor.
    const cut = await runMap(fakeDeps(files, { status: "throw" }), { budget: 400 }, 1);
    assertValidEnvelope(cut);
    if (!cut.ok) throw new Error("expected ok");
    expect(cut.results[0]?.kind).toBe("overview");
    expect(cut.results[1]?.kind).toBe("crate");
    expect(cut.truncation?.reason).toBe("budget");
    expect(cut.truncation?.cursor.startsWith("path:crates/m")).toBe(true);
    expect(cut.budget.used).toBeLessThanOrEqual(400);
  });

  test("per-row caps carry counts", async () => {
    const featureLines = Array.from({ length: 20 }, (_, i) => `f${i} = []`).join("\n");
    const files: Record<string, { content?: string }> = {
      "Cargo.toml": { content: `[package]\nname = "big"\n\n[features]\n${featureLines}\n` },
    };
    for (let i = 0; i < 10; i++) files[`src/bin/tool${i}.rs`] = {};
    const results = await ok(fakeDeps(files, { status: "throw" }), { detail: "skeleton" });
    const big = cratesOf(results)[0] as MapCrate;
    expect(big.entryPoints).toHaveLength(MAP_CRATE_ENTRY_POINTS_MAX);
    expect(big.moreEntryPoints).toBe(2);
    expect(big.features).toHaveLength(MAP_CRATE_FEATURES_MAX);
    expect(big.moreFeatures).toBe(4);
  });

  test("a single crate answers kind: crate with one row; a non-Cargo workspace is byte-identical", async () => {
    const single = await ok(
      fakeDeps(
        {
          "Cargo.toml": { content: '[package]\nname = "solo"\nedition = "2021"\n' },
          "src/main.rs": {},
        },
        { status: "throw" },
      ),
      { detail: "skeleton" },
    );
    expect(overviewOf(single).cargo).toEqual({
      kind: "crate",
      members: 1,
      edges: 0,
      top: [{ name: "solo", dependents: 0 }],
    });
    expect(cratesOf(single)).toHaveLength(1);
    expect(cratesOf(single)[0]?.roles).toEqual(["bin"]);
    // No Cargo.toml: no cargo key, no crate rows, and the index is asked
    // without scopes.
    const plain = fakeDeps(WORKSPACE, { importGraph: { fanIn: [], fanOut: [] } });
    const envelope = await runMap(plain, {}, 1);
    if (!envelope.ok) throw new Error("unreachable");
    expect(Object.hasOwn(envelope.results[0] ?? {}, "cargo")).toBe(false);
    expect(envelope.results.some((r) => r.kind === "crate")).toBe(false);
    expect(plain.crateScopesSeen[0]).toEqual([]);
    // A listed but unreadable root manifest is the same silence (the
    // specs/009 fixtures fake no content).
    const unreadable = await ok(fakeDeps({ "src/lib.rs": {}, "Cargo.toml": {} }));
    expect(Object.hasOwn(overviewOf(unreadable), "cargo")).toBe(false);
  });
});

describe("map: Cargo member manifest cap (specs/014 R2)", () => {
  test("members beyond the 256 manifests read are counted in a note, never guessed", async () => {
    const files: Record<string, { content?: string }> = {
      "Cargo.toml": { content: '[workspace]\nmembers = ["crates/*"]\n' },
    };
    for (let i = 0; i < 300; i++) {
      files[`crates/m${String(i).padStart(3, "0")}/Cargo.toml`] = {
        content: `[package]\nname = "m${String(i).padStart(3, "0")}"\n`,
      };
    }
    const envelope = await runMap(fakeDeps(files, { status: "throw" }), { detail: "skeleton" }, 1);
    assertValidEnvelope(envelope);
    if (!envelope.ok) throw new Error("unreachable");
    expect(overviewOf(envelope.results).cargo?.members).toBe(MAP_CRATE_MANIFESTS_MAX);
    expect(envelope.notes?.join(" ")).toContain("44 Cargo member manifests beyond the 256 read");
  });
});
