/**
 * Tiered-indexer suite (SDD §7.3) over the stub core: the ChildIndexer's
 * build/status/symbols/FTS/junk/upsert/remove behavior against a real
 * on-disk fixture workspace and a real SQLite index.db, plus one IPC
 * round-trip proving Tier-0 answers keep flowing while a build is in
 * flight. Native-grammar fidelity lives in the Rust unit tests; the perf
 * acceptance gate lives in scripts/bench-index.ts against real artifacts.
 */

import { Database } from "bun:sqlite";
import { afterEach, describe, expect, test } from "bun:test";
import { existsSync, mkdirSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import path from "node:path";
import { loadCore, nativeArtifactKey } from "../src/adapters/core";
import {
  ChildIndexer,
  EngineSupervisor,
  fallbackScanWorkspace,
  INDEX_SCHEMA_VERSION,
  IndexerHost,
  IndexMethodError,
  stubCoreBinding,
  VERIFY_REPARSE_BOUND,
} from "../src/adapters/engine-ipc";

const closers: (() => unknown)[] = [];
const tempDirs: string[] = [];

afterEach(async () => {
  // Every handle (database, child process) closes before any dir is removed —
  // Windows refuses to remove a directory whose files are still open.
  for (const close of closers.splice(0).reverse()) {
    await close();
  }
  for (const dir of tempDirs.splice(0)) {
    rmSync(dir, { recursive: true, force: true, maxRetries: 10, retryDelay: 50 });
  }
});

/** A small but representative workspace: source, docs, junk, ignored, binary. */
function makeFixture(): string {
  const root = mkdtempSync(path.join(tmpdir(), "wb-index-"));
  tempDirs.push(root);
  mkdirSync(path.join(root, "src"));
  mkdirSync(path.join(root, "vendor"));
  mkdirSync(path.join(root, "secret"));
  writeFileSync(
    path.join(root, "src", "alpha.ts"),
    'function alpha() {}\nclass Beta {}\nimport "./gamma";\n',
  );
  writeFileSync(path.join(root, "src", "gamma.ts"), "function gamma() {}\n");
  writeFileSync(path.join(root, "README.md"), "docs mention a needleword here\n");
  writeFileSync(path.join(root, "bun.lock"), "lockfile-needleword\n");
  writeFileSync(path.join(root, "vendor", "lib.ts"), "function vendored() {}\n");
  writeFileSync(path.join(root, ".gitignore"), "secret/\n");
  writeFileSync(path.join(root, "secret", "hidden.txt"), "must not index\n");
  writeFileSync(path.join(root, "logo.bin"), Buffer.from([0x89, 0x00, 0x50, 0x4e]));
  return root;
}

async function builtIndexer(root: string): Promise<ChildIndexer> {
  const indexer = new ChildIndexer(root, stubCoreBinding());
  closers.push(() => indexer.close());
  expect(indexer.build().started).toBe(true);
  await indexer.settled();
  return indexer;
}

describe("fallbackScanWorkspace", () => {
  test("honors the root .gitignore, excludes .git/.workbench, flags junk", () => {
    const root = makeFixture();
    mkdirSync(path.join(root, ".git"));
    writeFileSync(path.join(root, ".git", "HEAD"), "ref\n");
    const entries = fallbackScanWorkspace(root);
    const paths = entries.map((e) => e.path);
    expect(paths).toContain("src/alpha.ts");
    expect(paths).toContain("README.md");
    expect(paths).toContain("vendor/lib.ts");
    expect(paths.some((p) => p.startsWith("secret/"))).toBe(false);
    expect(paths.some((p) => p.startsWith(".git/"))).toBe(false);
    expect(entries.find((e) => e.path === "vendor/lib.ts")?.junk).toBe(true);
    expect(entries.find((e) => e.path === "bun.lock")?.junk).toBe(true);
    expect(entries.find((e) => e.path === "src/alpha.ts")?.junk).toBe(false);
    // Deterministic: sorted by path.
    expect(paths).toEqual([...paths].sort());
  });

  // The degraded contract (SDD §7.3.4): full nested-.gitignore honor with
  // compiled globs; negation lines are the one remaining deliberate limit —
  // if the negation assertion starts failing, the fallback gained fidelity
  // and the SDD contract paragraph needs updating in the same change.
  test("honors nested .gitignore files; negations remain unhonored", () => {
    const root = makeFixture();
    writeFileSync(path.join(root, "src", ".gitignore"), "gamma.ts\n");
    writeFileSync(path.join(root, ".gitignore"), "secret/\n*.log\n!keep.log\n");
    writeFileSync(path.join(root, "keep.log"), "negation would rescue me\n");
    writeFileSync(path.join(root, "drop.log"), "ignored either way\n");
    const paths = fallbackScanWorkspace(root).map((e) => e.path);
    // A nested rule now prunes: src/gamma.ts is gone, its sibling survives.
    expect(paths).not.toContain("src/gamma.ts");
    expect(paths).toContain("src/alpha.ts");
    // The nested rule does not leak upward or sideways.
    expect(paths).toContain("src/.gitignore");
    // Negation lines are skipped, not applied: keep.log stays excluded.
    expect(paths).not.toContain("keep.log");
    expect(paths).not.toContain("drop.log");
    expect(paths.some((p) => p.startsWith("secret/"))).toBe(false);
  });

  // The exact shape that motivated chunk:read-walker-gitignore — a nested
  // .gitignore with a prefix-glob dir rule (`wasi-sysroot-*/`), the pattern
  // the old *.ext-only matcher could not express.
  test("nested prefix-glob dir rules prune whole subtrees; anchored path rules work", () => {
    const root = makeFixture();
    mkdirSync(path.join(root, "spikes", "host", "wasi-sysroot-33.0+m", "include"), {
      recursive: true,
    });
    writeFileSync(
      path.join(root, "spikes", "host", ".gitignore"),
      "wasi-sysroot-*/\n*.o\namalgam/\n",
    );
    writeFileSync(
      path.join(root, "spikes", "host", "wasi-sysroot-33.0+m", "include", "__bit_reference"),
      "extensionless header\n",
    );
    writeFileSync(path.join(root, "spikes", "host", "guest.o"), "obj\n");
    writeFileSync(path.join(root, "spikes", "host", "spike.md"), "notes survive\n");
    // Anchored (slash-containing) rule in the ROOT .gitignore.
    mkdirSync(path.join(root, "crates", "core", "artifacts"), { recursive: true });
    writeFileSync(path.join(root, "crates", "core", "artifacts", "core.node"), "bin\n");
    writeFileSync(path.join(root, "crates", "core", "lib.rs"), "pub fn core() {}\n");
    writeFileSync(path.join(root, ".gitignore"), "secret/\ncrates/core/artifacts\n");

    const paths = fallbackScanWorkspace(root).map((e) => e.path);
    expect(paths.some((p) => p.includes("wasi-sysroot"))).toBe(false);
    expect(paths).not.toContain("spikes/host/guest.o");
    expect(paths).toContain("spikes/host/spike.md");
    expect(paths.some((p) => p.startsWith("crates/core/artifacts"))).toBe(false);
    expect(paths).toContain("crates/core/lib.rs");
  });

  // Ignore prunes DISCOVERY, never addressing: an ignored file stays
  // readable by exact path (the read tool never consults the walk).
  test("an ignored file is absent from the walk but readable by exact path", () => {
    const root = makeFixture();
    const ignored = path.join(root, "secret", "hidden.txt");
    expect(fallbackScanWorkspace(root).some((e) => e.path === "secret/hidden.txt")).toBe(false);
    expect(readFileSync(ignored, "utf8")).toBe("must not index\n");
  });
});

describe("ChildIndexer build", () => {
  test("reaches tier 1 with symbols, imports, FTS and junk on disk", async () => {
    const root = makeFixture();
    const indexer = await builtIndexer(root);

    const status = indexer.status();
    expect(status.tier).toBe(1);
    expect(status.building).toBe(false);
    expect(status.error).toBeUndefined();
    expect(status.filesTotal).toBeGreaterThanOrEqual(6);
    expect(status.filesIndexed).toBe(status.filesTotal);
    expect(status.generation).toBeGreaterThanOrEqual(1);

    // Symbols: case-insensitive substring, kind filter.
    expect(indexer.querySymbols("alpha")).toHaveLength(1);
    expect(indexer.querySymbols("ALPHA")).toHaveLength(1);
    expect(indexer.querySymbols("beta", "class")).toHaveLength(1);
    expect(indexer.querySymbols("beta", "function")).toHaveLength(0);
    const alpha = indexer.querySymbols("alpha")[0];
    expect(alpha?.path).toBe("src/alpha.ts");
    expect(alpha?.start_line).toBe(1);

    // Everything non-binary gets FTS5 — docs AND lockfiles (grep parity).
    expect(indexer.contentMatches("needleword")).toBe(2);
    // The gitignored and binary files got nothing.
    expect(indexer.contentMatches("index")).toBe(0);

    // The on-disk schema-v1 contract: junk is flagged, never excluded.
    indexer.close();
    const db = new Database(path.join(root, ".workbench", "index.db"), { readonly: true });
    try {
      // The version stamp is PRAGMA user_version (SDD §10.2), not a meta row.
      const stamped = db.query("PRAGMA user_version").get() as { user_version: number };
      expect(stamped.user_version).toBe(INDEX_SCHEMA_VERSION);
      const vendor = db.query("SELECT junk FROM files WHERE path = 'vendor/lib.ts'").get() as {
        junk: number;
      };
      expect(vendor.junk).toBe(1);
      const src = db
        .query("SELECT junk, content_indexed FROM files WHERE path = 'src/alpha.ts'")
        .get() as {
        junk: number;
        content_indexed: number;
      };
      expect(src.junk).toBe(0);
      expect(src.content_indexed).toBe(1);
      const binary = db
        .query("SELECT content_indexed FROM files WHERE path = 'logo.bin'")
        .get() as {
        content_indexed: number;
      };
      expect(binary.content_indexed).toBe(0);
      const imports = db.query("SELECT target FROM imports WHERE path = 'src/alpha.ts'").all() as {
        target: string;
      }[];
      expect(imports.map((i) => i.target)).toContain("./gamma");
      const secret = db
        .query("SELECT count(*) AS n FROM files WHERE path LIKE 'secret/%'")
        .get() as {
        n: number;
      };
      expect(secret.n).toBe(0);
    } finally {
      db.close();
    }
  });

  test("junk files are indexed for symbols too — demoted, not dropped", async () => {
    const indexer = await builtIndexer(makeFixture());
    expect(indexer.querySymbols("vendored")).toHaveLength(1);
  });

  test("no file is lost across flush boundaries under concurrency", async () => {
    // Regression: a build over many more files than one commit batch, with
    // all workers in flight, must index every file exactly once.
    const root = mkdtempSync(path.join(tmpdir(), "wb-index-many-"));
    tempDirs.push(root);
    mkdirSync(path.join(root, "bulk"));
    const total = 300;
    for (let i = 0; i < total; i += 1) {
      writeFileSync(path.join(root, "bulk", `f_${i}.ts`), `function fn_${i}() {}\n`);
    }
    const indexer = await builtIndexer(root);
    const status = indexer.status();
    expect(status.filesTotal).toBe(total);
    expect(status.filesIndexed).toBe(total);
    expect(indexer.querySymbols("fn_299")).toHaveLength(1);
    indexer.close();
    const db = new Database(path.join(root, ".workbench", "index.db"), { readonly: true });
    try {
      const files = db.query("SELECT count(*) AS n FROM files").get() as { n: number };
      expect(files.n).toBe(total);
    } finally {
      db.close();
    }
  });

  test("a second build while one runs is not doubled", async () => {
    const root = makeFixture();
    const indexer = new ChildIndexer(root, stubCoreBinding());
    closers.push(() => indexer.close());
    const first = indexer.build();
    const second = indexer.build();
    expect(first.started).toBe(true);
    expect(second.started).toBe(false);
    await indexer.settled();
    expect(indexer.status().tier).toBe(1);
  });

  test("tier and generation survive reopen (warm restart)", async () => {
    const root = makeFixture();
    const indexer = await builtIndexer(root);
    const generation = indexer.status().generation;
    indexer.close();

    const reopened = new ChildIndexer(root, stubCoreBinding());
    closers.push(() => reopened.close());
    expect(reopened.status().tier).toBe(1);
    expect(reopened.status().generation).toBe(generation);
    expect(reopened.querySymbols("alpha")).toHaveLength(1);
  });

  test("a rebuild drops files that vanished from the workspace", async () => {
    const root = makeFixture();
    const indexer = await builtIndexer(root);
    expect(indexer.querySymbols("gamma")).toHaveLength(1);
    rmSync(path.join(root, "src", "gamma.ts"));
    indexer.build();
    await indexer.settled();
    expect(indexer.querySymbols("gamma")).toHaveLength(0);
    expect(indexer.querySymbols("alpha")).toHaveLength(1);
  });

  // The Tier-1 build used to re-parse every file on every start, so a server
  // restart over an untouched tree cost a full rebuild (measured 45.3s on a
  // 3.3k-file workspace). Rows carry size+mtime — the same freshness anchor
  // query-time verification trusts (§7.3.3) — so an unmoved file is reused.
  test("an unchanged tree is reused, not re-parsed", async () => {
    const root = makeFixture();
    const indexer = await builtIndexer(root);
    const total = indexer.status().filesTotal;
    expect(total).toBeGreaterThan(0);
    expect(indexer.reusedLastBuild).toBe(0); // first build parses everything

    indexer.build();
    await indexer.settled();
    expect(indexer.reusedLastBuild).toBe(total);
    expect(indexer.status().filesIndexed).toBe(total);
    // Reuse must not mean amnesia: the rows are still queryable.
    expect(indexer.querySymbols("alpha")).toHaveLength(1);
  });

  test("reuse still re-indexes a file whose content changed", async () => {
    const root = makeFixture();
    const indexer = await builtIndexer(root);
    expect(indexer.querySymbols("alpha")).toHaveLength(1);

    // A different size, so the size+mtime anchor moves even if the clock's
    // resolution would not have separated the two writes.
    writeFileSync(
      path.join(root, "src", "alpha.ts"),
      "export function zetaSwapped(): void {}\n// changed\n",
    );
    indexer.build();
    await indexer.settled();

    expect(indexer.querySymbols("alpha")).toHaveLength(0);
    expect(indexer.querySymbols("zetaSwapped")).toHaveLength(1);
    expect(indexer.reusedLastBuild).toBeLessThan(indexer.status().filesTotal);
  });

  test("reuse still indexes a file that appeared since the last build", async () => {
    const root = makeFixture();
    const indexer = await builtIndexer(root);
    expect(indexer.querySymbols("freshlyAdded")).toHaveLength(0);

    writeFileSync(path.join(root, "src", "added.ts"), "export function freshlyAdded(): void {}\n");
    indexer.build();
    await indexer.settled();

    expect(indexer.querySymbols("freshlyAdded")).toHaveLength(1);
    expect(indexer.querySymbols("alpha")).toHaveLength(1);
  });
});

describe("ChildIndexer incremental verbs", () => {
  test("upsert re-extracts from disk and bumps the generation", async () => {
    const root = makeFixture();
    const indexer = await builtIndexer(root);
    const before = indexer.status().generation;
    writeFileSync(path.join(root, "src", "alpha.ts"), "function alpha() {}\nfunction delta() {}\n");
    await indexer.upsert("src/alpha.ts");
    expect(indexer.status().generation).toBe(before + 1);
    expect(indexer.querySymbols("delta")).toHaveLength(1);
  });

  test("upsert accepts caller-provided symbols (the port seam)", async () => {
    const root = makeFixture();
    const indexer = await builtIndexer(root);
    await indexer.upsert("README.md", [
      {
        name: "SeededSymbol",
        kind: "function",
        path: "README.md",
        start_line: 1,
        start_col: 1,
        end_line: 1,
      },
    ]);
    expect(indexer.querySymbols("seededsymbol")).toHaveLength(1);
  });

  test("upsert of a missing file is a typed, answerable error", async () => {
    const indexer = await builtIndexer(makeFixture());
    await expect(indexer.upsert("no/such/file.ts")).rejects.toBeInstanceOf(IndexMethodError);
  });

  test("remove drops all rows and bumps the generation", async () => {
    const root = makeFixture();
    const indexer = await builtIndexer(root);
    const before = indexer.status().generation;
    indexer.remove("src/alpha.ts");
    expect(indexer.status().generation).toBe(before + 1);
    expect(indexer.querySymbols("alpha")).toHaveLength(0);
    expect(indexer.querySymbols("gamma")).toHaveLength(1);
  });
});

describe("IndexerHost", () => {
  test("index/* methods route; status before any build is a typed refusal", async () => {
    const host = new IndexerHost(stubCoreBinding());
    closers.push(() => host.close());
    await expect(host.handle("index/status", {})).rejects.toBeInstanceOf(IndexMethodError);

    const root = makeFixture();
    const kicked = (await host.handle("index/build", { root })) as { started: boolean };
    expect(kicked.started).toBe(true);
    await host.get(root)?.settled();
    const status = (await host.handle("index/status", {})) as { tier: number };
    expect(status.tier).toBe(1);
    const result = (await host.handle("index/symbols", { name: "alpha" })) as {
      symbols: unknown[];
    };
    expect(result.symbols).toHaveLength(1);
  });

  test("malformed params are MALFORMED_FRAME, matching the child's contract", async () => {
    const host = new IndexerHost(stubCoreBinding());
    closers.push(() => host.close());
    await expect(host.handle("index/build", {})).rejects.toMatchObject({
      code: "MALFORMED_FRAME",
    });
  });
});

describe("Tier 0 over IPC while a build runs", () => {
  test("the child answers ping/search during index/build (never blocks on warm)", async () => {
    const root = makeFixture();
    const supervisor = new EngineSupervisor({
      env: { WORKBENCH_ENGINE_STUB_CORE: "1" },
      backoffMs: [0, 10, 20],
      readyTimeoutMs: 10_000,
    });
    closers.push(() => supervisor.stop());

    const kicked = (await supervisor.call("index/build", { root })) as { started: boolean };
    expect(kicked.started).toBe(true);

    // Tier-0 answers flow while the build is (possibly) still in flight.
    expect(await supervisor.call("ping", {})).toEqual({ pong: true });
    const search = (await supervisor.call("search", {
      pattern: "needleword",
      haystack: "a needleword line",
      path: "README.md",
    })) as { hits: unknown[] };
    expect(search.hits).toHaveLength(1);

    // The build completes and reports tier 1 over the same pipe.
    const deadline = Date.now() + 10_000;
    let status: { tier: number; building: boolean } | undefined;
    while (Date.now() < deadline) {
      status = (await supervisor.call("index/status", {})) as {
        tier: number;
        building: boolean;
      };
      if (status.tier === 1 && !status.building) break;
      await new Promise((r) => setTimeout(r, 50));
    }
    expect(status?.tier).toBe(1);
    const symbols = (await supervisor.call("index/symbols", { name: "alpha" })) as {
      symbols: unknown[];
    };
    expect(symbols.symbols).toHaveLength(1);
  });
});

describe("querySymbolsVerified (SDD §7.3.3)", () => {
  test("a behind-the-back edit is re-parsed synchronously; the answer is warm and current", async () => {
    const root = makeFixture();
    const indexer = await builtIndexer(root);

    // Move alpha down two lines behind the indexer's back — no upsert, no
    // watcher. The raw rows are now stale; the verified query must not be.
    writeFileSync(
      path.join(root, "src", "alpha.ts"),
      '// moved\n// moved\nfunction alpha() {}\nclass Beta {}\nimport "./gamma";\n',
    );
    expect(indexer.querySymbols("alpha")[0]?.start_line).toBe(1); // stale, pre-verification
    const verified = await indexer.querySymbolsVerified("alpha");
    expect(verified.freshness).toBe("warm");
    expect(verified.symbols[0]?.start_line).toBe(3);
  });

  test("a vanished file's rows never leave; the answer stays warm", async () => {
    const root = makeFixture();
    const indexer = await builtIndexer(root);
    rmSync(path.join(root, "src", "gamma.ts"));
    const verified = await indexer.querySymbolsVerified("gamma");
    expect(verified.symbols).toHaveLength(0);
    expect(verified.freshness).toBe("warm");
  });

  test("past the re-parse bound the answer degrades to cold and stale rows are dropped", async () => {
    const root = mkdtempSync(path.join(tmpdir(), "wb-verify-"));
    tempDirs.push(root);
    const total = VERIFY_REPARSE_BOUND + 2;
    for (let i = 0; i < total; i += 1) {
      writeFileSync(path.join(root, `f${i}.ts`), `function bound_${i}() {}\n`);
    }
    const indexer = await builtIndexer(root);

    // Invalidate every file: only VERIFY_REPARSE_BOUND get the synchronous
    // re-parse; the rest are dropped from the answer and label it cold.
    for (let i = 0; i < total; i += 1) {
      writeFileSync(path.join(root, `f${i}.ts`), `// pad\nfunction bound_${i}() {}\n`);
    }
    const verified = await indexer.querySymbolsVerified("bound_");
    expect(verified.freshness).toBe("cold");
    expect(verified.symbols).toHaveLength(VERIFY_REPARSE_BOUND);
    // Whatever was answered is current — every surviving row shows the new line.
    for (const row of verified.symbols) {
      expect(row.start_line).toBe(2);
    }
  });
});

describe("queryOutlineVerified (chunk:m1-map)", () => {
  test("answers per-file symbol rows in line order, verified before return", async () => {
    const root = makeFixture();
    const indexer = await builtIndexer(root);
    const outline = await indexer.queryOutlineVerified(["src/alpha.ts", "src/gamma.ts"]);
    expect(outline.freshness).toBe("warm");
    expect(outline.files.map((f) => f.path)).toEqual(["src/alpha.ts", "src/gamma.ts"]);
    const alpha = outline.files[0];
    expect(alpha?.symbols.map((s) => s.name)).toEqual(["alpha", "Beta"]);
    expect(alpha?.symbols[0]?.start_line).toBe(1);
  });

  test("a behind-the-back edit is re-parsed; the outline is warm and current", async () => {
    const root = makeFixture();
    const indexer = await builtIndexer(root);
    writeFileSync(
      path.join(root, "src", "alpha.ts"),
      '// moved\nfunction alpha() {}\nclass Beta {}\nimport "./gamma";\n',
    );
    const outline = await indexer.queryOutlineVerified(["src/alpha.ts"]);
    expect(outline.freshness).toBe("warm");
    expect(outline.files[0]?.symbols[0]?.start_line).toBe(2);
  });

  test("an unindexed path answers empty rather than erroring", async () => {
    const root = makeFixture();
    const indexer = await builtIndexer(root);
    const outline = await indexer.queryOutlineVerified(["no/such/file.ts"]);
    expect(outline.files).toEqual([{ path: "no/such/file.ts", symbols: [] }]);
  });
});

describe("queryReferencesVerified (chunk:m1-search-structural)", () => {
  /** def in util.ts, import-linked use in app.ts, bare name match in notes.md. */
  function makeRefsFixture(): string {
    const root = mkdtempSync(path.join(tmpdir(), "wb-refs-"));
    tempDirs.push(root);
    mkdirSync(path.join(root, "src"));
    mkdirSync(path.join(root, "docs"));
    writeFileSync(path.join(root, "src", "util.ts"), "function needle_ref() {}\n");
    writeFileSync(path.join(root, "src", "app.ts"), 'import "./util";\nconst x = needle_ref();\n');
    writeFileSync(
      path.join(root, "docs", "notes.md"),
      "needle_ref mentioned here\nconst xneedle_refy = 1;\n",
    );
    return root;
  }

  test("edges carry confidence provenance: definition high, import-linked medium, name match low", async () => {
    const root = makeRefsFixture();
    const indexer = await builtIndexer(root);
    const { edges, freshness } = await indexer.queryReferencesVerified("needle_ref");
    expect(freshness).toBe("warm");

    const definition = edges.find((e) => e.ref_kind === "definition");
    expect(definition?.path).toBe("src/util.ts");
    expect(definition?.line).toBe(1);
    expect(definition?.confidence).toBe("high");
    expect(definition?.provenance).toBe("tier1-symbols");
    expect(definition?.excerpt).toContain("function needle_ref");

    const linked = edges.find((e) => e.path === "src/app.ts");
    expect(linked?.ref_kind).toBe("use");
    expect(linked?.line).toBe(2);
    expect(linked?.confidence).toBe("medium");
    expect(linked?.provenance).toBe("tier1-imports");

    const textual = edges.find((e) => e.path === "docs/notes.md");
    expect(textual?.confidence).toBe("low");
    expect(textual?.provenance).toBe("tier1-text");
    // Identifier boundaries: xneedle_refy never matches, so notes.md has one edge.
    expect(edges.filter((e) => e.path === "docs/notes.md")).toHaveLength(1);
    // The definition line is a definition edge, never doubled as a use.
    expect(
      edges.filter((e) => e.path === "src/util.ts" && e.line === 1 && e.ref_kind === "use"),
    ).toHaveLength(0);
    // Definitions sort first.
    expect(edges[0]?.ref_kind).toBe("definition");
  });

  test("a behind-the-back edit is verified before the answer leaves (SDD §7.3.3)", async () => {
    const root = makeRefsFixture();
    const indexer = await builtIndexer(root);
    // Move the definition down one line with no upsert and no watcher.
    writeFileSync(path.join(root, "src", "util.ts"), "// pad\nfunction needle_ref() {}\n");
    const { edges, freshness } = await indexer.queryReferencesVerified("needle_ref");
    expect(freshness).toBe("warm");
    expect(edges.find((e) => e.ref_kind === "definition")?.line).toBe(2);
  });

  test("index/references is served by the IndexerHost", async () => {
    const root = makeRefsFixture();
    const host = new IndexerHost(stubCoreBinding());
    closers.push(() => host.close());
    await host.handle("index/build", { root });
    const indexer = host.get(root);
    if (indexer === undefined) throw new Error("indexer not created");
    await indexer.settled();
    const result = (await host.handle("index/references", { name: "needle_ref" })) as {
      edges: readonly { ref_kind: string }[];
      freshness: string;
    };
    expect(result.edges.length).toBeGreaterThanOrEqual(3);
    expect(result.edges.some((e) => e.ref_kind === "definition")).toBe(true);
  });
});

describe("index schema v2 (specs/009-rust-index-fidelity)", () => {
  test("a version-1 index is DISCARDED and rebuilt, never read half-shaped", async () => {
    const root = makeFixture();
    const dbPath = path.join(root, ".workbench", "index.db");
    mkdirSync(path.join(root, ".workbench"), { recursive: true });
    // A database in the OLD shape: no detail/doc columns, no test_ranges, and
    // a row that must not survive into the rebuilt index.
    const old = new Database(dbPath);
    old.run("PRAGMA user_version = 1");
    old.run("CREATE TABLE meta (key TEXT PRIMARY KEY, value TEXT NOT NULL)");
    old.run("INSERT INTO meta (key, value) VALUES ('schema_version', '1')");
    old.run(
      "CREATE TABLE symbols (path TEXT NOT NULL, name TEXT NOT NULL, kind TEXT NOT NULL, " +
        "start_line INTEGER NOT NULL, start_col INTEGER NOT NULL, end_line INTEGER NOT NULL)",
    );
    old.run(
      "INSERT INTO symbols (path, name, kind, start_line, start_col, end_line) " +
        "VALUES ('src/ghost.ts', 'GhostFromV1', 'function', 1, 1, 1)",
    );
    old.close();

    const indexer = await builtIndexer(root);
    // The stale row is gone: the whole cache was rebuilt from the filesystem.
    expect(indexer.querySymbols("GhostFromV1")).toHaveLength(0);
    expect(indexer.querySymbols("alpha")).toHaveLength(1);
    indexer.close();

    const db = new Database(dbPath, { readonly: true });
    try {
      const stamped = db.query("PRAGMA user_version").get() as { user_version: number };
      expect(stamped.user_version).toBe(INDEX_SCHEMA_VERSION);
      // The v2 shape is actually present, not merely stamped.
      const columns = (db.query("PRAGMA table_info(symbols)").all() as { name: string }[]).map(
        (c) => c.name,
      );
      expect(columns).toContain("detail");
      expect(columns).toContain("doc");
      const tables = (
        db.query("SELECT name FROM sqlite_master WHERE type = 'table'").all() as {
          name: string;
        }[]
      ).map((t) => t.name);
      expect(tables).toContain("test_ranges");
    } finally {
      db.close();
    }
  });
});

/**
 * specs/017-index-fts-delete: the tier-1 build was O(n²) in file count
 * because every file write deleted its full-text row by the UNINDEXED path
 * column (a whole-table scan per file). The full-text row now carries its
 * files row id and is deleted by that id; a path with no files row issues no
 * delete at all.
 */
describe("index schema v3 (specs/017-index-fts-delete)", () => {
  function openIndex(root: string): Database {
    const db = new Database(path.join(root, ".workbench", "index.db"), { readonly: true });
    closers.push(() => db.close());
    return db;
  }

  /** Every content-bearing files row has exactly one FTS row at the same row id, and no other. */
  function expectAligned(db: Database): void {
    const misaligned = db
      .query(
        "SELECT f.path FROM files f WHERE f.content_indexed = 1 AND NOT EXISTS " +
          "(SELECT 1 FROM content_fts c WHERE c.rowid = f.rowid AND c.path = f.path)",
      )
      .all();
    expect(misaligned).toEqual([]);
    const orphans = db
      .query(
        "SELECT c.path FROM content_fts c WHERE NOT EXISTS " +
          "(SELECT 1 FROM files f WHERE f.rowid = c.rowid AND f.path = c.path AND f.content_indexed = 1)",
      )
      .all();
    expect(orphans).toEqual([]);
    const ftsRows = (db.query("SELECT count(*) AS n FROM content_fts").get() as { n: number }).n;
    const contentRows = (
      db.query("SELECT count(*) AS n FROM files WHERE content_indexed = 1").get() as { n: number }
    ).n;
    expect(ftsRows).toBe(contentRows);
  }

  test("full-text rows are aligned to their files row id after every mutation", async () => {
    const root = makeFixture();
    const indexer = await builtIndexer(root);
    const db = openIndex(root);
    expectAligned(db);

    // upsert: the old FTS row is gone, the new one sits at the (new) files row id.
    writeFileSync(path.join(root, "src", "alpha.ts"), "function alpha() {}\nfunction delta() {}\n");
    await indexer.upsert("src/alpha.ts");
    expectAligned(db);
    expect(
      db.query("SELECT count(*) AS n FROM content_fts WHERE path = 'src/alpha.ts'").get(),
    ).toEqual({
      n: 1,
    });
    expect(indexer.contentMatches("delta")).toBe(1);

    // remove: no row of any kind survives, including the full-text row.
    indexer.remove("src/gamma.ts");
    expectAligned(db);
    expect(
      db.query("SELECT count(*) AS n FROM content_fts WHERE path = 'src/gamma.ts'").get(),
    ).toEqual({
      n: 0,
    });
    expect(indexer.contentMatches("gamma")).toBe(0);

    // vanished + changed on the next build: the purge and the re-write both keep the invariant.
    rmSync(path.join(root, "README.md"));
    writeFileSync(
      path.join(root, "src", "gamma.ts"),
      "function gamma() {}\nfunction epsilon() {}\n",
    );
    indexer.build();
    await indexer.settled();
    expectAligned(db);
    expect(indexer.contentMatches("needleword")).toBe(1); // bun.lock only; README.md is gone
    expect(indexer.contentMatches("epsilon")).toBe(1);

    // A removed path that never existed is a no-op, not an error.
    indexer.remove("no/such/file.ts");
    expectAligned(db);
  });

  test("per-file write cost is flat in index size (4N files under 1.5x the per-file cost of N)", async () => {
    const buildOnce = async (files: number): Promise<number> => {
      const root = mkdtempSync(path.join(tmpdir(), "wb-index-flat-"));
      mkdirSync(path.join(root, "src"));
      for (let i = 0; i < files; i += 1) {
        writeFileSync(
          path.join(root, "src", `m${i}.ts`),
          `function fn_${i}() { return ${i}; }\nexport const value_${i} = ${i};\n`,
        );
      }
      const indexer = new ChildIndexer(root, stubCoreBinding());
      const start = Bun.nanoseconds();
      indexer.build();
      await indexer.settled();
      const elapsedMs = (Bun.nanoseconds() - start) / 1e6;
      expect(indexer.status().filesIndexed).toBe(files);
      indexer.close();
      // Removed here, not in afterEach: tens of thousands of files would time the hook out.
      rmSync(root, { recursive: true, force: true, maxRetries: 5, retryDelay: 50 });
      return elapsedMs;
    };
    // Min of three runs: a single sub-second build is at the mercy of GC and
    // FTS5 segment merges; the minimum is the cost floor the shape is about.
    const timeBuild = async (files: number): Promise<number> =>
      Math.min(await buildOnce(files), await buildOnce(files), await buildOnce(files));
    const n = 2000;
    await buildOnce(500); // warm-up: JIT and page cache
    const small = await timeBuild(n);
    const large = await timeBuild(4 * n);
    // Linear: 4x the files costs ~4x the time (measured 12-13k files/s flat
    // from 2k to 16k, 1.15x per file here). The old by-path FTS delete
    // measured ~13x for 4x the files.
    const perFileSmall = small / n;
    const perFileLarge = large / (4 * n);
    expect(perFileLarge).toBeLessThan(perFileSmall * 1.5);
  }, 60_000);

  test("a version-2 index is discarded and rebuilt: its full-text row ids are unaligned", async () => {
    const root = makeFixture();
    const dbPath = path.join(root, ".workbench", "index.db");
    mkdirSync(path.join(root, ".workbench"), { recursive: true });
    const old = new Database(dbPath);
    old.run("PRAGMA user_version = 2");
    old.run("CREATE TABLE meta (key TEXT PRIMARY KEY, value TEXT NOT NULL)");
    old.run(
      "CREATE TABLE files (path TEXT PRIMARY KEY, language TEXT, size INTEGER NOT NULL, " +
        "mtime_ms INTEGER NOT NULL, junk INTEGER NOT NULL, content_indexed INTEGER NOT NULL)",
    );
    old.run("CREATE VIRTUAL TABLE content_fts USING fts5(path UNINDEXED, content)");
    old.run("INSERT INTO files VALUES ('src/ghost.ts', 'typescript', 1, 1, 0, 1)");
    old.run(
      "INSERT INTO content_fts (rowid, path, content) VALUES (99, 'src/ghost.ts', 'ghostword')",
    );
    old.close();

    const indexer = await builtIndexer(root);
    expect(indexer.contentMatches("ghostword")).toBe(0);
    expect(indexer.querySymbols("alpha")).toHaveLength(1);
    const db = openIndex(root);
    expect((db.query("PRAGMA user_version").get() as { user_version: number }).user_version).toBe(
      INDEX_SCHEMA_VERSION,
    );
    expectAligned(db);
  });
});

// The stub core binding is a line-regex emulator: it can never produce
// qualified names, doc summaries or cfg(test) ranges, so the end-to-end Rust
// extraction pins need the REAL native binding (specs/009).
const nativeArtifact = path.join(
  import.meta.dir,
  "..",
  "crates",
  "workbench-core",
  "artifacts",
  `workbench-core.${nativeArtifactKey()}.node`,
);

describe.if(existsSync(nativeArtifact))("Rust extraction reaches the index (specs/009)", () => {
  test("qualified names, doc summaries and cfg(test) ranges land in the index", async () => {
    const root = makeFixture();
    writeFileSync(
      path.join(root, "src", "cfg.rs"),
      [
        "/// Loads configuration.",
        "pub struct Config;",
        "",
        "impl Config {",
        "    /// Reads it.",
        "    pub fn load() -> u32 { 1 }",
        "}",
        "",
        "#[cfg(test)]",
        "mod tests {",
        "    #[test]",
        "    fn works() {}",
        "}",
        "",
      ].join(String.fromCharCode(10)),
    );
    const { binding } = loadCore({ force: "native" });
    const indexer = new ChildIndexer(root, binding);
    closers.push(() => indexer.close());
    expect(indexer.build().started).toBe(true);
    await indexer.settled();

    // Qualified, and still reachable by the BARE member name (substring).
    const qualified = indexer.querySymbols("Config::load");
    expect(qualified).toHaveLength(1);
    expect(indexer.querySymbols("load").map((r) => r.name)).toContain("Config::load");
    expect(qualified[0]?.doc).toBe("Reads it.");
    expect(indexer.querySymbols("Config", "struct")[0]?.doc).toBe("Loads configuration.");

    // The cfg(test) module's range is stored for ranking.
    const ranges = indexer.queryTestRanges(["src/cfg.rs"]);
    expect(ranges["src/cfg.rs"]).toHaveLength(1);
    expect(ranges["src/cfg.rs"]?.[0]?.start_line).toBe(10);
    // A file with no test module simply has no rows.
    expect(indexer.queryTestRanges(["src/alpha.ts"])["src/alpha.ts"]).toBeUndefined();
  });
});

// ------------------------------ per-crate import edges (specs/014 R6)

describe("crate-level import edges over the imports table (specs/014)", () => {
  test("rows attribute to the longest member dir, match name or name::*, count distinct files; no scopes = no crates key", async () => {
    const root = makeFixture();
    // The importing files must EXIST: the fan-out half of the answer is
    // verified against disk and rows of vanished files are purged before
    // the crate aggregation runs (an honest cache, SDD 7.3.3).
    const importers = [
      "crates/alpha/src/lib.rs",
      "crates/beta/src/lib.rs",
      "crates/beta/src/util.rs",
      "crates/beta/tests/t.rs",
      "crates/beta/nested/src/lib.rs",
      "tools/cli/src/main.rs",
      "src/lib.rs",
    ];
    for (const rel of importers) {
      mkdirSync(path.dirname(path.join(root, rel)), { recursive: true });
      writeFileSync(path.join(root, rel), "pub fn item() {}\n");
    }
    const indexer = await builtIndexer(root);
    // Import rows as the Rust extractor stores them: raw `use` targets,
    // inserted directly so the aggregation is pinned independently of the
    // stub core's grammar fidelity (DISTINCT path absorbs any real rows).
    const db = new Database(path.join(root, ".workbench", "index.db"));
    closers.push(() => db.close());
    const rows: [string, string][] = [
      ["crates/beta/src/lib.rs", "alpha::add"],
      ["crates/beta/src/lib.rs", "alpha::sub::x"], // same file: one, not two
      ["crates/beta/src/util.rs", "alpha"], // bare crate name
      ["crates/beta/tests/t.rs", "beta_crate::twice"], // self edge, via the use name
      ["tools/cli/src/main.rs", "alpha::add"],
      ["tools/cli/src/main.rs", "beta_crate::twice"],
      ["crates/beta/nested/src/lib.rs", "alpha::add"], // the NESTED member owns it
      ["src/lib.rs", "alpha::add"], // no member dir: attributed to "."
      ["crates/beta/src/lib.rs", "alphabet::z"], // a different crate, not alpha
      ["crates/beta/src/lib.rs", "std::fmt::Debug"],
    ];
    const insert = db.query("INSERT INTO imports (path, target) VALUES (?, ?)");
    for (const [p, t] of rows) insert.run(p, t);
    const answer = await indexer.queryImportGraphVerified(5, undefined, [
      { name: "alpha", dir: "crates/alpha" },
      { name: "beta_crate", dir: "crates/beta" },
      { name: "nested", dir: "crates/beta/nested" },
      { name: "cli", dir: "tools/cli" },
      { name: "root", dir: "." },
    ]);
    expect(answer.crates).toEqual([
      { from: ".", to: "alpha", files: 1 },
      { from: "crates/beta", to: "alpha", files: 2 },
      { from: "crates/beta", to: "beta_crate", files: 1 },
      { from: "crates/beta/nested", to: "alpha", files: 1 },
      { from: "tools/cli", to: "alpha", files: 1 },
      { from: "tools/cli", to: "beta_crate", files: 1 },
    ]);
    // Asked without scopes: the pre-v9 answer, no `crates` key at all.
    const plain = await indexer.queryImportGraphVerified(5);
    expect(Object.hasOwn(plain, "crates")).toBe(false);
    // Asked with an empty scope list: an empty answer, not an absent one.
    const empty = await indexer.queryImportGraphVerified(5, undefined, []);
    expect(empty.crates).toEqual([]);
  });
});
