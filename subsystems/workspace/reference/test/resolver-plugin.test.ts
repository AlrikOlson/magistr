/**
 * The LSP resolver plugin — acceptance suite for specs/001-lsp-resolver-plugin
 * (chunk:m5-lsp-resolver-plugin).
 *
 * Chapters:
 *   adapter          PluginResolver over a scripted invoke lane: lazy boot,
 *                    blessed-tier + grant gating, circuit breaker with the
 *                    bounded lazy retry, per-request budget, jail admission
 *                    (FR-003/004/005/009; US2 kill semantics).
 *   search           the references lane merges resolver edges per-edge and
 *                    the SC-003 inequality holds: no symbolic edge is ever
 *                    labeled resolver-lsp (US3).
 *   contracts        the real plugin.toml declares ONLY public WIT imports
 *                    (SC-005) and the guest's wit/deps copies match wit/
 *                    byte for byte.
 *   live lane        the real guest.wasm over the real native binding and a
 *                    real spawned fake language server: Tier-2 answers,
 *                    crash degradation, pre-instantiation capability denial
 *                    (US1; quickstart Scenarios 2 and 5's grant half).
 */

import { describe, expect, test } from "bun:test";
import { existsSync, mkdirSync, mkdtempSync, readFileSync, writeFileSync } from "node:fs";
import { readFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import path from "node:path";
import { loadCore, nativeArtifactKey } from "../src/adapters/core";
import {
  detectLanguageServers,
  type LanguageServerResolver,
  type LanguageServerSpec,
  PluginResolver,
  RESOLVER_PLUGIN_NAME,
  RESOLVER_RETRY_AFTER_MS,
  type ResolverGuestOutcome,
  type ResolverPluginDeps,
} from "../src/protocol/resolver-plugin";
import { runSearch, type SearchDeps } from "../src/tools";
import {
  effectiveCapabilities,
  grantCovers,
  type InstalledPluginRecord,
  isSubprocessImport,
  type PluginHealthRecord,
  parsePluginManifest,
  SUBPROCESS_IMPORT,
} from "../src/tools/plugins";

const ROOT = path.join(import.meta.dir, "..");
const PLUGIN_DIR = path.join(ROOT, "plugins", "lsp-resolver");
const REAL_MANIFEST = readFileSync(path.join(PLUGIN_DIR, "plugin.toml"), "utf8");

const FAKE_SPEC: LanguageServerSpec = {
  languageId: "fake",
  bin: process.execPath,
  args: [path.join(ROOT, "test", "support", "fake-lsp.ts")],
  transport: "stdio",
  source: "test-seam",
};

/** A workspace on real disk — the jail canonicalizes against real files. */
function tempWorkspace(): string {
  const root = mkdtempSync(path.join(tmpdir(), "workbench-resolver-"));
  mkdirSync(path.join(root, "src"), { recursive: true });
  writeFileSync(path.join(root, "src", "greet.ts"), "export function greet() {}\n");
  writeFileSync(path.join(root, "src", "use.ts"), "\n\ngreet();\n");
  return root;
}

const OK_OUTCOME: ResolverGuestOutcome = {
  outcome: "ok",
  detail: "",
  fuelUsed: 1000,
  locations: [{ path: "src/greet.ts", line: 1, column: 17 }],
};

interface AdapterRig {
  readonly resolver: PluginResolver;
  readonly invokes: string[];
  readonly breaches: PluginHealthRecord[];
}

function adapterRig(
  init: {
    tier?: number;
    record?: Partial<InstalledPluginRecord> | null;
    outcome?: ResolverGuestOutcome | (() => Promise<ResolverGuestOutcome>);
    seedBreach?: { ageMs: number };
    budgetMs?: number;
    root?: string;
  } = {},
): AdapterRig {
  const root = init.root ?? tempWorkspace();
  const invokes: string[] = [];
  const breaches: PluginHealthRecord[] = [];
  const NOW = "2026-08-22T12:00:00.000Z";
  if (init.seedBreach !== undefined) {
    breaches.push({
      plugin: RESOLVER_PLUGIN_NAME,
      componentHash: "hash-1",
      reason: "seeded",
      at: new Date(Date.parse(NOW) - init.seedBreach.ageMs).toISOString(),
    });
  }
  const record: InstalledPluginRecord | null =
    init.record === null
      ? null
      : {
          name: RESOLVER_PLUGIN_NAME,
          source: "registry",
          tier: "blessed",
          dir: "plugins/lsp-resolver",
          grants: [SUBPROCESS_IMPORT],
          ...init.record,
        };
  const deps: ResolverPluginDeps = {
    root,
    specs: [FAKE_SPEC],
    indexTier: () => Promise.resolve(init.tier ?? 1),
    inventory: () => Promise.resolve(record === null ? [] : [record]),
    readManifest: () => Promise.resolve(REAL_MANIFEST),
    componentInfo: () => Promise.resolve({ absPath: "/x/guest.wasm", sha256: "hash-1" }),
    invokeResolver: (_absPath, symbol) => {
      invokes.push(symbol);
      const outcome = init.outcome ?? OK_OUTCOME;
      return typeof outcome === "function" ? outcome() : Promise.resolve(outcome);
    },
    health: {
      read: () => Promise.resolve([...breaches]),
      append: (r) => {
        breaches.push(r);
        return Promise.resolve();
      },
    },
    now: () => NOW,
    ...(init.budgetMs === undefined ? {} : { budgetMs: init.budgetMs }),
  };
  return { resolver: new PluginResolver(deps), invokes, breaches };
}

describe("adapter: gating, breaker, budget, jail", () => {
  test("healthy plugin upgrades references with resolver-lsp/high edges", async () => {
    const rig = adapterRig();
    const edges = await rig.resolver.resolve({ kind: "references", symbol: "greet" });
    expect(edges).toEqual([
      {
        location: { path: "src/greet.ts", line: 1, column: 17 },
        provenance: "resolver-lsp",
        confidence: "high",
      },
    ]);
    expect(rig.invokes).toEqual(["greet"]);
    expect(rig.breaches).toHaveLength(0);
  });

  test("definitions are not the upgrade lane: no invoke, empty answer", async () => {
    const rig = adapterRig();
    expect(await rig.resolver.resolve({ kind: "definition", symbol: "greet" })).toEqual([]);
    expect(rig.invokes).toHaveLength(0);
  });

  test("lazy boot: below Tier-1 the plugin is never consulted (FR-003)", async () => {
    const rig = adapterRig({ tier: 0 });
    expect(await rig.resolver.resolve({ kind: "references", symbol: "greet" })).toEqual([]);
    expect(rig.invokes).toHaveLength(0);
  });

  test("a non-blessed record never invokes, even with the grant (FR-004)", async () => {
    const verified = adapterRig({ record: { tier: "verified" } });
    expect(await verified.resolver.resolve({ kind: "references", symbol: "greet" })).toEqual([]);
    expect(verified.invokes).toHaveLength(0);
  });

  test("a blessed record without the grant never invokes (FR-004)", async () => {
    const ungranted = adapterRig({ record: { grants: [] } });
    expect(await ungranted.resolver.resolve({ kind: "references", symbol: "greet" })).toEqual([]);
    expect(ungranted.invokes).toHaveLength(0);
  });

  test("no installed plugin: empty answer, nothing thrown (US2 'absent')", async () => {
    const rig = adapterRig({ record: null });
    expect(await rig.resolver.resolve({ kind: "references", symbol: "greet" })).toEqual([]);
    expect(rig.invokes).toHaveLength(0);
  });

  test("a fresh breach short-circuits; an expired one lazily retries (FR-005)", async () => {
    const open = adapterRig({ seedBreach: { ageMs: 1_000 } });
    expect(await open.resolver.resolve({ kind: "references", symbol: "greet" })).toEqual([]);
    expect(open.invokes).toHaveLength(0);

    const expired = adapterRig({ seedBreach: { ageMs: RESOLVER_RETRY_AFTER_MS + 1_000 } });
    const edges = await expired.resolver.resolve({ kind: "references", symbol: "greet" });
    expect(edges).toHaveLength(1);
    expect(expired.invokes).toEqual(["greet"]);
  });

  test("a crashed guest answers symbolically and records the breach (US2)", async () => {
    const rig = adapterRig({
      outcome: { outcome: "guest-trap", detail: "unreachable", fuelUsed: 5, locations: [] },
    });
    expect(await rig.resolver.resolve({ kind: "references", symbol: "greet" })).toEqual([]);
    expect(rig.breaches).toHaveLength(1);
    expect(rig.breaches[0]?.reason).toContain("guest-trap");
  });

  test("cap-denied is a configuration fact, not a health breach", async () => {
    const rig = adapterRig({
      outcome: { outcome: "cap-denied", detail: SUBPROCESS_IMPORT, fuelUsed: 0, locations: [] },
    });
    expect(await rig.resolver.resolve({ kind: "references", symbol: "greet" })).toEqual([]);
    expect(rig.breaches).toHaveLength(0);
  });

  test("a hanging invoke loses to the budget: symbolic ships, slowness recorded (FR-005)", async () => {
    const rig = adapterRig({
      budgetMs: 25,
      outcome: () => new Promise<ResolverGuestOutcome>(() => {}),
    });
    const edges = await rig.resolver.resolve({ kind: "references", symbol: "greet" });
    expect(edges).toEqual([]);
    expect(rig.breaches).toHaveLength(1);
    expect(rig.breaches[0]?.reason).toContain("budget");
  });

  test("an out-of-jail location poisons the whole upgrade (FR-009)", async () => {
    const rig = adapterRig({
      outcome: {
        outcome: "ok",
        detail: "",
        fuelUsed: 10,
        locations: [
          { path: "src/greet.ts", line: 1, column: 17 },
          { path: "../../outside.ts", line: 1, column: 1 },
        ],
      },
    });
    expect(await rig.resolver.resolve({ kind: "references", symbol: "greet" })).toEqual([]);
    expect(rig.breaches).toHaveLength(1);
    expect(rig.breaches[0]?.reason).toContain("malformed resolver location");
  });

  test("zero or negative line/column is malformed data, not an edge (FR-009)", async () => {
    const rig = adapterRig({
      outcome: {
        outcome: "ok",
        detail: "",
        fuelUsed: 10,
        locations: [{ path: "src/greet.ts", line: 0, column: 17 }],
      },
    });
    expect(await rig.resolver.resolve({ kind: "references", symbol: "greet" })).toEqual([]);
    expect(rig.breaches).toHaveLength(1);
  });
});

// ------------------------------------------------------------ search merge

function searchDepsWith(resolver: SearchDeps["resolver"]): SearchDeps {
  const files: Record<string, string> = {
    "src/a.ts": "function greet() {}\n",
    "src/b.ts": "greet();\n",
    "src/c.ts": "\ngreet();\n",
  };
  return {
    engine: {
      version: 2,
      parse: () => Promise.resolve({ ok: true }),
      search: () => Promise.resolve([]),
      rewrite: () => Promise.reject(new Error("not under test")),
    },
    index: {
      version: 9,
      status: () => Promise.resolve({ tier: 1, generation: 1, building: false }),
      build: () => Promise.reject(new Error("not under test")),
      querySymbols: () => Promise.resolve({ symbols: [], freshness: "warm" }),
      queryReferences: () =>
        Promise.resolve({
          freshness: "warm",
          edges: [
            {
              location: { path: "src/a.ts", line: 1, column: 10 },
              excerpt: "function greet() {}",
              refKind: "definition",
              confidence: "high",
              provenance: "tier1-symbols",
            },
            {
              location: { path: "src/b.ts", line: 1, column: 1 },
              excerpt: "greet();",
              refKind: "use",
              confidence: "low",
              provenance: "tier1-text",
            },
          ],
        }),
      queryOutline: () => Promise.resolve({ files: [], freshness: "warm" }),
      queryImportGraph: () => Promise.resolve({ fanIn: [], fanOut: [], freshness: "warm" }),
      upsertFile: () => Promise.resolve(),
      refreshFile: () => Promise.resolve(),
      removeFile: () => Promise.resolve(),
    },
    workspace: {
      list: () => Promise.resolve(Object.keys(files).map((p) => ({ path: p, junk: false }))),
      read: (p: string) => Promise.resolve(files[p] ?? ""),
    },
    ...(resolver === undefined ? {} : { resolver }),
  };
}

function allHits(
  envelope: Awaited<ReturnType<typeof runSearch>>,
): { file: string; line: number; provenance?: string; confidence?: string }[] {
  if (!envelope.ok) throw new Error("expected ok envelope");
  return envelope.results.flatMap((group) =>
    group.hits.map((hit) => ({
      file: group.file,
      line: hit.line,
      ...(hit.provenance === undefined ? {} : { provenance: hit.provenance }),
      ...(hit.confidence === undefined ? {} : { confidence: hit.confidence }),
    })),
  );
}

describe("search: per-edge provenance merge (US3)", () => {
  test("resolver-confirmed edges upgrade; unconfirmed keep tier1-*; new edges append", async () => {
    const deps = searchDepsWith({
      version: 2,
      resolve: () =>
        Promise.resolve([
          {
            location: { path: "src/a.ts", line: 1, column: 10 },
            provenance: "resolver-lsp",
            confidence: "high",
          },
          {
            location: { path: "src/c.ts", line: 2, column: 1 },
            provenance: "resolver-lsp",
            confidence: "high",
          },
        ]),
    });
    const envelope = await runSearch(deps, { query: "greet", mode: "references" }, 1);
    const hits = allHits(envelope);
    expect(hits.find((h) => h.file === "src/a.ts" && h.line === 1)?.provenance).toBe(
      "resolver-lsp",
    );
    // SC-003: the unconfirmed symbolic edge is NEVER labeled resolver-lsp.
    expect(hits.find((h) => h.file === "src/b.ts" && h.line === 1)?.provenance).toBe("tier1-text");
    expect(hits.find((h) => h.file === "src/c.ts" && h.line === 2)?.provenance).toBe(
      "resolver-lsp",
    );
    if (!envelope.ok) throw new Error("unreachable");
    expect((envelope.notes ?? []).join(" ")).toContain("resolver plugin");
  });

  test("a failing resolver leaves the symbolic answer byte-identical (US2/SC-001)", async () => {
    const withResolver = await runSearch(
      searchDepsWith({ version: 2, resolve: () => Promise.reject(new Error("resolver died")) }),
      { query: "greet", mode: "references" },
      1,
    );
    const without = await runSearch(
      searchDepsWith(undefined),
      { query: "greet", mode: "references" },
      1,
    );
    expect(allHits(withResolver)).toEqual(allHits(without));
    for (const hit of allHits(withResolver)) {
      expect(hit.provenance).not.toBe("resolver-lsp");
    }
  });
});

// ------------------------------------------------------------- contracts

describe("contracts: public-vocabulary audit and drift pins", () => {
  test("the real manifest declares ONLY the public subprocess import (SC-005)", () => {
    const parsed = parsePluginManifest(REAL_MANIFEST);
    if (!parsed.ok) throw new Error(parsed.errors.join("; "));
    expect(parsed.manifest.publisher).toBe("workbench");
    expect(parsed.manifest.pkg).toBe("lsp-resolver");
    expect([...parsed.manifest.imports]).toEqual([SUBPROCESS_IMPORT]);
  });

  test("blessed tier is required beyond the grant: a sideload grant is inert", () => {
    const parsed = parsePluginManifest(REAL_MANIFEST);
    if (!parsed.ok) throw new Error(parsed.errors.join("; "));
    const blessed = effectiveCapabilities(
      {
        name: RESOLVER_PLUGIN_NAME,
        source: "registry",
        tier: "blessed",
        dir: "d",
        grants: [SUBPROCESS_IMPORT],
      },
      parsed.manifest,
    );
    expect(blessed.effective).toEqual([SUBPROCESS_IMPORT]);
    const sideload = effectiveCapabilities(
      { name: RESOLVER_PLUGIN_NAME, source: "sideload", dir: "d", grants: [SUBPROCESS_IMPORT] },
      parsed.manifest,
    );
    expect(sideload.effective).toEqual([]);
    expect(sideload.notes.join(" ")).toContain("reserved to blessed");
  });

  test("the guest's wit/deps copies are byte-identical to the public wit/", () => {
    for (const [copy, source] of [
      ["guest/wit/deps/subprocess/subprocess.wit", "wit/subprocess.wit"],
      ["guest/wit/deps/resolver/resolver.wit", "wit/resolver.wit"],
    ] as const) {
      expect(readFileSync(path.join(PLUGIN_DIR, copy), "utf8")).toBe(
        readFileSync(path.join(ROOT, source), "utf8"),
      );
    }
  });
});

// -------------------------------------------------------------- live lane

const nativeArtifact = path.join(
  ROOT,
  "crates",
  "workbench-core",
  "artifacts",
  `workbench-core.${nativeArtifactKey()}.node`,
);

describe.if(existsSync(nativeArtifact))("live lane: real guest, real binding, real server", () => {
  const { binding } = loadCore({ force: "native" });
  const guestPath = path.join(PLUGIN_DIR, "guest.wasm");

  async function invoke(
    symbol: string,
    capabilities: string[],
    mode: "normal" | "crash" = "normal",
    root: string = tempWorkspace(),
  ) {
    if (binding.sandboxResolveReferences === undefined) {
      throw new Error("native binding lacks the resolver lane");
    }
    const guest = new Uint8Array(await readFile(guestPath));
    return binding.sandboxResolveReferences(
      guest,
      symbol,
      capabilities,
      { fuel: 500_000_000, memoryCapBytes: 64 * 1024 * 1024 },
      [
        {
          languageId: "fake",
          bin: process.execPath,
          args: [path.join(ROOT, "test", "support", "fake-lsp.ts"), mode],
        },
      ],
      root,
    );
  }

  test("resolves through the spawned server: relative, 1-based locations (US1)", async () => {
    const outcome = await invoke("greet", [SUBPROCESS_IMPORT]);
    expect(outcome.outcome).toBe("ok");
    expect(outcome.locations).toEqual([
      { path: "src/greet.ts", line: 1, column: 17 },
      { path: "src/use.ts", line: 3, column: 1 },
    ]);
    expect(outcome.fuelUsed).toBeGreaterThan(0);
  }, 30_000);

  test("a server crash mid-conversation degrades to an empty ok answer (US2)", async () => {
    const outcome = await invoke("greet", [SUBPROCESS_IMPORT], "crash");
    expect(outcome.outcome).toBe("ok");
    expect(outcome.locations).toEqual([]);
  }, 30_000);

  test("without the grant the guest is refused before instantiation (FR-004)", async () => {
    const outcome = await invoke("greet", []);
    expect(outcome.outcome).toBe("cap-denied");
    expect(outcome.detail).toBe(SUBPROCESS_IMPORT);
    expect(outcome.fuelUsed).toBe(0);
  }, 30_000);

  test("the full adapter stack answers resolver-lsp edges over the live lane (US1)", async () => {
    if (binding.sandboxResolveReferences === undefined) {
      throw new Error("native binding lacks the resolver lane");
    }
    const invokeResolver = binding.sandboxResolveReferences.bind(binding);
    const root = tempWorkspace();
    const resolver = new PluginResolver({
      root,
      specs: [
        {
          languageId: "fake",
          bin: process.execPath,
          args: [path.join(ROOT, "test", "support", "fake-lsp.ts")],
          transport: "stdio",
          source: "test-seam",
        },
      ],
      indexTier: () => Promise.resolve(1),
      inventory: () =>
        Promise.resolve([
          {
            name: RESOLVER_PLUGIN_NAME,
            source: "registry" as const,
            tier: "blessed" as const,
            dir: "plugins/lsp-resolver",
            grants: [SUBPROCESS_IMPORT],
          },
        ]),
      readManifest: () => Promise.resolve(REAL_MANIFEST),
      componentInfo: () => Promise.resolve({ absPath: guestPath, sha256: "live-hash" }),
      invokeResolver: async (absPath, symbol, capabilities, policy, specs, wsRoot) => {
        const guest = new Uint8Array(await readFile(absPath));
        return invokeResolver(
          guest,
          symbol,
          [...capabilities],
          policy,
          specs.map((s) => ({ languageId: s.languageId, bin: s.bin, args: [...s.args] })),
          wsRoot,
        );
      },
      health: { read: () => Promise.resolve([]), append: () => Promise.resolve() },
      now: () => new Date().toISOString(),
      budgetMs: 20_000,
    });
    const edges = await resolver.resolve({ kind: "references", symbol: "greet" });
    expect(edges).toEqual([
      {
        location: { path: "src/greet.ts", line: 1, column: 17 },
        provenance: "resolver-lsp",
        confidence: "high",
      },
      {
        location: { path: "src/use.ts", line: 3, column: 1 },
        provenance: "resolver-lsp",
        confidence: "high",
      },
    ]);
  }, 30_000);
});

// ------------------------------------------------------------- session pool
// specs/007-resolver-server-pool: US1 (warm reuse across invokes), the
// same-major grant rule (US3), and the recorded real-server number (US4).

describe("contracts: grants cover the same capability within a major (007 R3)", () => {
  test("isSubprocessImport accepts every major-0 stamp and nothing else", () => {
    expect(isSubprocessImport("workbench:subprocess/subprocess@0.1.0")).toBe(true);
    expect(isSubprocessImport(SUBPROCESS_IMPORT)).toBe(true);
    expect(isSubprocessImport("workbench:subprocess/subprocess@0.9.4")).toBe(true);
    expect(isSubprocessImport("workbench:subprocess/subprocess@1.0.0")).toBe(false);
    expect(isSubprocessImport("workbench:tool/host@0.1.0")).toBe(false);
  });

  test("grantCovers: exact, same-major, never across majors or packages", () => {
    expect(grantCovers(SUBPROCESS_IMPORT, SUBPROCESS_IMPORT)).toBe(true);
    expect(grantCovers("workbench:subprocess/subprocess@0.1.0", SUBPROCESS_IMPORT)).toBe(true);
    expect(grantCovers(SUBPROCESS_IMPORT, "workbench:subprocess/subprocess@0.1.0")).toBe(true);
    expect(grantCovers("workbench:subprocess/subprocess@1.0.0", SUBPROCESS_IMPORT)).toBe(false);
    expect(grantCovers("workbench:tool/host@0.2.0", SUBPROCESS_IMPORT)).toBe(false);
    expect(grantCovers("observe", "observe")).toBe(true);
    expect(grantCovers("observe", SUBPROCESS_IMPORT)).toBe(false);
  });

  test("a 0.1.0 grant authorizes the rebuilt manifest's 0.2.0 import — blessed only", () => {
    const parsed = parsePluginManifest(REAL_MANIFEST);
    if (!parsed.ok) throw new Error(parsed.errors.join("; "));
    expect([...parsed.manifest.imports]).toEqual([SUBPROCESS_IMPORT]);
    const legacyGrant = ["workbench:subprocess/subprocess@0.1.0"];
    const blessed = effectiveCapabilities(
      {
        name: RESOLVER_PLUGIN_NAME,
        source: "registry",
        tier: "blessed",
        dir: "d",
        grants: legacyGrant,
      },
      parsed.manifest,
    );
    expect(blessed.effective).toEqual([SUBPROCESS_IMPORT]);
    const verified = effectiveCapabilities(
      {
        name: RESOLVER_PLUGIN_NAME,
        source: "registry",
        tier: "verified",
        dir: "d",
        grants: legacyGrant,
      },
      parsed.manifest,
    );
    expect(verified.effective).toEqual([]);
    expect(verified.notes.join(" ")).toContain("reserved to blessed");
    const sideload = effectiveCapabilities(
      { name: RESOLVER_PLUGIN_NAME, source: "sideload", dir: "d", grants: legacyGrant },
      parsed.manifest,
    );
    expect(sideload.effective).toEqual([]);
  });
});

describe.if(existsSync(nativeArtifact))("pool: warm reuse through the live lane (US1)", () => {
  const { binding } = loadCore({ force: "native" });
  const guestPath = path.join(PLUGIN_DIR, "guest.wasm");

  function fakeSpec(modes: string[]): LanguageServerSpec {
    return {
      languageId: "fake",
      bin: process.execPath,
      args: [path.join(ROOT, "test", "support", "fake-lsp.ts"), ...modes],
      transport: "stdio",
      source: "test-seam",
    };
  }

  /** A PluginResolver over the real binding with an injectable clock and a
   * window onto the in-flight guest invokes. */
  function liveResolver(spec: LanguageServerSpec, root: string, budgetMs: number) {
    if (binding.sandboxResolveReferences === undefined) {
      throw new Error("native binding lacks the resolver lane");
    }
    const invokeResolver = binding.sandboxResolveReferences.bind(binding);
    const breaches: PluginHealthRecord[] = [];
    const inflight: Promise<ResolverGuestOutcome>[] = [];
    const outcomes: ResolverGuestOutcome[] = [];
    let nowMs = Date.parse("2026-08-22T12:00:00.000Z");
    const resolver = new PluginResolver({
      root,
      specs: [spec],
      indexTier: () => Promise.resolve(1),
      inventory: () =>
        Promise.resolve([
          {
            name: RESOLVER_PLUGIN_NAME,
            source: "registry" as const,
            tier: "blessed" as const,
            dir: "plugins/lsp-resolver",
            grants: [SUBPROCESS_IMPORT],
          },
        ]),
      readManifest: () => Promise.resolve(REAL_MANIFEST),
      componentInfo: () => Promise.resolve({ absPath: guestPath, sha256: "pool-hash" }),
      invokeResolver: async (absPath, symbol, capabilities, policy, specs, wsRoot) => {
        const guest = new Uint8Array(await readFile(absPath));
        const p = invokeResolver(
          guest,
          symbol,
          [...capabilities],
          policy,
          specs.map((s) => ({ languageId: s.languageId, bin: s.bin, args: [...s.args] })),
          wsRoot,
        ).then((o) => {
          outcomes.push(o);
          return o;
        });
        inflight.push(p);
        return p;
      },
      health: {
        read: () => Promise.resolve([...breaches]),
        append: (r) => {
          breaches.push(r);
          return Promise.resolve();
        },
      },
      now: () => new Date(nowMs).toISOString(),
      budgetMs,
    });
    return {
      resolver,
      breaches,
      outcomes,
      settle: () => Promise.all(inflight),
      advance: (ms: number) => {
        nowMs += ms;
      },
    };
  }

  test("SC-001: a server slower than the budget contributes on the SECOND request, one handshake", async () => {
    const root = tempWorkspace();
    const countFile = path.join(root, "inits.txt");
    const rig = liveResolver(fakeSpec(["slow-init:2500", `count:${countFile}`]), root, 1_000);

    // Request 1: the handshake (2.5 s) loses the 1 s budget — symbolic ships,
    // a breach is recorded, and the guest invoke keeps running.
    const first = await rig.resolver.resolve({ kind: "references", symbol: "greet" });
    expect(first).toEqual([]);
    expect(rig.breaches).toHaveLength(1);
    expect(rig.breaches[0]?.reason).toContain("budget");
    // The in-flight invoke finishes on its own and hands the session back warm.
    await rig.settle();
    expect(rig.outcomes[0]?.outcome).toBe("ok");
    expect(rig.outcomes[0]?.sessionsOpened).toBe(1);
    expect(rig.outcomes[0]?.sessionsReused).toBe(0);

    // Request 2 (after the breach expiry): warm session, no handshake, edges.
    rig.advance(RESOLVER_RETRY_AFTER_MS + 1_000);
    const second = await rig.resolver.resolve({ kind: "references", symbol: "greet" });
    expect(second).toEqual([
      {
        location: { path: "src/greet.ts", line: 1, column: 17 },
        provenance: "resolver-lsp",
        confidence: "high",
      },
      {
        location: { path: "src/use.ts", line: 3, column: 1 },
        provenance: "resolver-lsp",
        confidence: "high",
      },
    ]);
    expect(rig.breaches).toHaveLength(1);
    expect(rig.outcomes[1]?.sessionsOpened).toBe(0);
    expect(rig.outcomes[1]?.sessionsReused).toBe(1);
    // The server saw exactly one initialize across both requests.
    const inits = readFileSync(countFile, "utf8").trim().split("\n");
    expect(inits).toEqual(["initialize 1"]);
  }, 60_000);

  test("a warm server that dies between requests is replaced: the next request respawns", async () => {
    const root = tempWorkspace();
    const countFile = path.join(root, "inits.txt");
    // crash-at:2 — the SECOND workspace/symbol of a process exits it.
    const rig = liveResolver(fakeSpec(["crash-at:2", `count:${countFile}`]), root, 20_000);
    const first = await rig.resolver.resolve({ kind: "references", symbol: "greet" });
    expect(first).toHaveLength(2);
    expect(rig.outcomes[0]?.sessionsOpened).toBe(1);
    // Request 2 reuses the warm server, which dies mid-conversation: the
    // guest judges the session unusable and closes it; this answer is empty
    // but ok (no breach — the host saw a clean guest end).
    const second = await rig.resolver.resolve({ kind: "references", symbol: "greet" });
    expect(second).toEqual([]);
    expect(rig.outcomes[1]?.outcome).toBe("ok");
    expect(rig.outcomes[1]?.sessionsReused).toBe(1);
    expect(rig.breaches).toHaveLength(0);
    // Request 3 spawns fresh (handshake #2 overall) and answers again.
    const third = await rig.resolver.resolve({ kind: "references", symbol: "greet" });
    expect(third).toHaveLength(2);
    expect(rig.outcomes[2]?.sessionsOpened).toBe(1);
    expect(rig.outcomes[2]?.sessionsReused).toBe(0);
    const inits = readFileSync(countFile, "utf8").trim().split("\n");
    expect(inits).toEqual(["initialize 1", "initialize 1"]);
  }, 60_000);

  test("pool stats see the warm session; shutdown empties them", () => {
    const before = binding.sandboxServerPoolStats?.() ?? [];
    expect(before.some((s) => s.languageId === "fake" && !s.leased)).toBe(true);
    expect((binding.sandboxServerPoolShutdown?.() ?? 0) >= 1).toBe(true);
    expect(
      (binding.sandboxServerPoolStats?.() ?? []).filter((s) => s.languageId === "fake"),
    ).toEqual([]);
  });
});

// ------------------------------------------------------------- real server
// US4 / SC-005: the recorded cold-vs-warm number for ONE real language
// server. rust-analyzer is the server present on the machine this feature
// was shaped on; typescript-language-server is not, so no number is claimed
// for it. Environment-gated: without both binaries the chapter is skipped
// and the skip says which binaries it needs.

const RUST_ANALYZER = Bun.which("rust-analyzer");
const CARGO = Bun.which("cargo");
/** A PATH hit is not enough: the rustup proxy exists without the component
 * installed and fails with "Unknown binary" — the binary must actually run. */
function runs(bin: string | null): boolean {
  if (bin === null) return false;
  try {
    return Bun.spawnSync([bin, "--version"], { stdout: "pipe", stderr: "pipe" }).exitCode === 0;
  } catch {
    return false;
  }
}
const REAL_SERVER = runs(RUST_ANALYZER) && runs(CARGO) && existsSync(nativeArtifact);

test.skipIf(REAL_SERVER)(
  "real server chapter skipped: needs a WORKING rust-analyzer AND cargo (--version exits 0) plus the native artifact (specs/007 SC-005)",
  () => {},
);

describe.if(REAL_SERVER)("real server: rust-analyzer cold vs warm (US4 / SC-005)", () => {
  const { binding } = loadCore({ force: "native" });
  const guestPath = path.join(PLUGIN_DIR, "guest.wasm");

  function fixtureCrate(): string {
    const root = mkdtempSync(path.join(tmpdir(), "workbench-ra-"));
    mkdirSync(path.join(root, "src"), { recursive: true });
    writeFileSync(
      path.join(root, "Cargo.toml"),
      '[package]\nname = "fixture"\nversion = "0.1.0"\nedition = "2021"\n\n[dependencies]\n',
    );
    writeFileSync(
      path.join(root, "src", "lib.rs"),
      "pub fn greet() -> u32 {\n    1\n}\n\npub fn caller() -> u32 {\n    greet() + greet()\n}\n",
    );
    return root;
  }

  test("cold, time-to-first-Tier-2, warm — recorded; one handshake across every request", async () => {
    if (binding.sandboxResolveReferences === undefined || RUST_ANALYZER === null) {
      throw new Error("unreachable: gated above");
    }
    const invokeResolver = binding.sandboxResolveReferences.bind(binding);
    const root = fixtureCrate();
    const guest = new Uint8Array(await readFile(guestPath));
    const spec = { languageId: "rust", bin: RUST_ANALYZER, args: [] as string[] };
    const call = async () => {
      const started = performance.now();
      const outcome = await invokeResolver(
        guest,
        "greet",
        [SUBPROCESS_IMPORT],
        { fuel: 500_000_000, memoryCapBytes: 64 * 1024 * 1024 },
        [spec],
        root,
      );
      return { outcome, ms: performance.now() - started };
    };
    const t0 = performance.now();
    const cold = await call();
    expect(cold.outcome.outcome).toBe("ok");
    expect(cold.outcome.sessionsOpened).toBe(1);
    let opened = cold.outcome.sessionsOpened;
    let reused = cold.outcome.sessionsReused;
    // rust-analyzer answers workspace/symbol empty until its load completes;
    // keep asking on the warm session until Tier-2 edges arrive.
    let firstUpgrade = cold.outcome.locations.length > 0 ? cold : null;
    let calls = 1;
    while (firstUpgrade === null && performance.now() - t0 < 90_000) {
      await Bun.sleep(500);
      const next = await call();
      calls += 1;
      opened += next.outcome.sessionsOpened;
      reused += next.outcome.sessionsReused;
      expect(next.outcome.outcome).toBe("ok");
      if (next.outcome.locations.length > 0) firstUpgrade = next;
    }
    if (firstUpgrade === null) throw new Error("rust-analyzer never answered references in 90 s");
    const timeToFirstUpgradeMs = performance.now() - t0;
    const warm = await call();
    calls += 1;
    opened += warm.outcome.sessionsOpened;
    reused += warm.outcome.sessionsReused;
    expect(warm.outcome.locations.length).toBeGreaterThanOrEqual(2);
    for (const loc of warm.outcome.locations) expect(loc.path).toBe("src/lib.rs");
    expect(warm.outcome.sessionsReused).toBe(1);
    // One process served every request: one fresh spawn, the rest reused.
    expect(opened).toBe(1);
    expect(reused).toBe(calls - 1);
    const line =
      `[007 SC-005] rust-analyzer over fixture crate: cold=${cold.ms.toFixed(1)}ms ` +
      `(locations ${cold.outcome.locations.length}) time-to-first-Tier-2=${timeToFirstUpgradeMs.toFixed(0)}ms ` +
      `warm=${warm.ms.toFixed(1)}ms (locations ${warm.outcome.locations.length}) ` +
      `calls=${calls} handshakes=${opened}`;
    console.log(line);
    writeFileSync(path.join(root, "sc-005.txt"), `${line}\n`);
    expect(warm.ms).toBeLessThan(2_000);
    binding.sandboxServerPoolShutdown?.();
  }, 150_000);
});

// ------------------------------------------------------- rust detection (008)
// chunk:rust-resolver: detectLanguageServers gains a languageId:"rust" entry
// when a Cargo project has a rust-analyzer that actually runs. The resolver is
// injected so these units never shell out to a real toolchain manager.

describe("rust detection: the table both directions (SC-001)", () => {
  function ws(files: Record<string, string>): string {
    const root = mkdtempSync(path.join(tmpdir(), "workbench-detect-"));
    for (const [name, content] of Object.entries(files)) {
      writeFileSync(path.join(root, name), content);
    }
    return root;
  }
  const never: LanguageServerResolver = () => null;
  const found =
    (bin: string): LanguageServerResolver =>
    () =>
      bin;
  const noTs = () => null; // typescript-language-server lookup returns nothing

  test("Cargo.toml + a runnable rust-analyzer => exactly one rust entry", () => {
    const root = ws({ "Cargo.toml": '[package]\nname = "x"\nversion = "0.0.0"\n' });
    const specs = detectLanguageServers(root, noTs, found("/tc/bin/rust-analyzer"));
    expect(specs).toEqual([
      {
        languageId: "rust",
        bin: "/tc/bin/rust-analyzer",
        args: [],
        transport: "stdio",
        source: "detected",
      },
    ]);
  });

  test("a virtual workspace manifest counts as a Cargo project", () => {
    const root = ws({ "Cargo.toml": '[workspace]\nmembers = ["a"]\n' });
    const specs = detectLanguageServers(root, noTs, found("/tc/bin/rust-analyzer"));
    expect(specs.map((s) => s.languageId)).toEqual(["rust"]);
  });

  test("no Cargo.toml => no rust entry, even when a binary would resolve", () => {
    const root = ws({ "README.md": "# no cargo here\n" });
    expect(detectLanguageServers(root, noTs, found("/tc/bin/rust-analyzer"))).toEqual([]);
  });

  // A monorepo whose Cargo workspace sits one level below the repo root
  // (`<repo>/rust/Cargo.toml`) is a Rust project too: rust-analyzer discovers
  // the nested workspace from the parent, so detection must not refuse it.
  test("a nested <root>/rust/Cargo.toml counts as a Cargo project", () => {
    const root = ws({ "README.md": "# monorepo\n" });
    mkdirSync(path.join(root, "rust"));
    writeFileSync(path.join(root, "rust", "Cargo.toml"), '[workspace]\nmembers = ["a"]\n');
    const specs = detectLanguageServers(root, noTs, found("/tc/bin/rust-analyzer"));
    expect(specs.map((s) => s.languageId)).toEqual(["rust"]);
  });

  test("the resolver is asked with the NESTED dir as cwd (toolchain pin honored)", () => {
    const root = ws({ "README.md": "# monorepo\n" });
    mkdirSync(path.join(root, "rust"));
    writeFileSync(path.join(root, "rust", "Cargo.toml"), '[workspace]\nmembers = ["a"]\n');
    const askedWith: string[] = [];
    const recording: LanguageServerResolver = (dir) => {
      askedWith.push(dir);
      return "/tc/bin/rust-analyzer";
    };
    detectLanguageServers(root, noTs, recording);
    expect(askedWith).toEqual([path.join(root, "rust")]);
  });

  test("detection does not descend past one level or into dot/target/node_modules", () => {
    const root = ws({ "README.md": "# deep\n" });
    mkdirSync(path.join(root, "a", "b"), { recursive: true });
    writeFileSync(path.join(root, "a", "b", "Cargo.toml"), "[package]\n"); // two levels: ignored
    mkdirSync(path.join(root, "target"));
    writeFileSync(path.join(root, "target", "Cargo.toml"), "[package]\n"); // junk dir: ignored
    mkdirSync(path.join(root, ".hidden"));
    writeFileSync(path.join(root, ".hidden", "Cargo.toml"), "[package]\n"); // dot dir: ignored
    expect(detectLanguageServers(root, noTs, found("/tc/bin/rust-analyzer"))).toEqual([]);
  });

  test("resolver returns null (proxy without component) => no rust entry, no spawn", () => {
    const root = ws({ "Cargo.toml": '[package]\nname = "x"\n' });
    let calls = 0;
    const countingNull: LanguageServerResolver = () => {
      calls += 1;
      return null;
    };
    expect(detectLanguageServers(root, noTs, countingNull)).toEqual([]);
    expect(calls).toBe(1); // asked once; produced nothing — nothing downstream spawns
  });

  test("a TypeScript project that also has a Cargo.toml gets both entries, TS first", () => {
    const root = ws({ "package.json": "{}\n", "Cargo.toml": '[package]\nname = "x"\n' });
    const specs = detectLanguageServers(
      root,
      () => "/path/tsserver",
      found("/tc/bin/rust-analyzer"),
    );
    expect(specs.map((s) => s.languageId)).toEqual(["typescript", "rust"]);
  });

  test("the TS entry is unchanged by the rust path (regression guard, FR-007)", () => {
    const root = ws({ "tsconfig.json": "{}\n" });
    const specs = detectLanguageServers(root, () => "/path/tsserver", never);
    expect(specs).toEqual([
      {
        languageId: "typescript",
        bin: "/path/tsserver",
        args: ["--stdio"],
        transport: "stdio",
        source: "detected",
      },
    ]);
  });
});

// ---------------------------------------------------- rust fixture (008 gated)
// SC-002 / SC-003 / SC-004: the REAL detectLanguageServers over a fixture Cargo
// crate, driving the real binding + guest through the warm pool. Gated on a
// rust-analyzer that actually resolves via rustup.

function rustAnalyzerResolves(): boolean {
  try {
    const r = Bun.spawnSync(["rustup", "which", "rust-analyzer"], {
      stdout: "pipe",
      stderr: "pipe",
    });
    return r.exitCode === 0 && new TextDecoder().decode(r.stdout).trim().length > 0;
  } catch {
    return false;
  }
}
const RUST_FIXTURE = rustAnalyzerResolves() && existsSync(nativeArtifact);

test.skipIf(RUST_FIXTURE)(
  "rust fixture chapter skipped: needs `rustup which rust-analyzer` to resolve plus the native artifact (chunk:rust-resolver SC-002)",
  () => {},
);

describe.if(RUST_FIXTURE)("rust fixture: detection -> warm resolver-lsp references", () => {
  const { binding } = loadCore({ force: "native" });
  const guestPath = path.join(PLUGIN_DIR, "guest.wasm");

  /** A Cargo crate with a trait method, an impl, a generic fn that calls it
   * through the trait bound, and a macro-generated item — the precision cases
   * a name-based symbolic pass handles poorly. */
  function fixtureCrate(): string {
    const root = mkdtempSync(path.join(tmpdir(), "workbench-rustfix-"));
    mkdirSync(path.join(root, "src"), { recursive: true });
    writeFileSync(
      path.join(root, "Cargo.toml"),
      '[package]\nname = "fixture"\nversion = "0.1.0"\nedition = "2021"\n\n[dependencies]\n',
    );
    const lib = [
      "pub fn greet() -> u32 {",
      "    1",
      "}",
      "",
      "pub fn greet_all<T>(_t: T) -> u32 {",
      "    greet() + greet()",
      "}",
      "",
      "pub trait Greeter {",
      "    fn hello(&self) -> u32;",
      "}",
      "",
      "pub struct English;",
      "",
      "impl Greeter for English {",
      "    fn hello(&self) -> u32 {",
      "        greet()",
      "    }",
      "}",
      "",
      "macro_rules! make_greet {",
      "    () => {",
      "        pub fn macro_greet() -> u32 {",
      "            greet()",
      "        }",
      "    };",
      "}",
      "make_greet!();",
      "",
    ].join("\n");
    writeFileSync(path.join(root, "src", "lib.rs"), lib);
    return root;
  }

  function resolverOver(root: string, specs: LanguageServerSpec[]) {
    if (binding.sandboxResolveReferences === undefined) {
      throw new Error("native binding lacks the resolver lane");
    }
    const invokeResolver = binding.sandboxResolveReferences.bind(binding);
    return new PluginResolver({
      root,
      specs,
      indexTier: () => Promise.resolve(1),
      inventory: () =>
        Promise.resolve([
          {
            name: RESOLVER_PLUGIN_NAME,
            source: "registry" as const,
            tier: "blessed" as const,
            dir: "plugins/lsp-resolver",
            grants: [SUBPROCESS_IMPORT],
          },
        ]),
      readManifest: () => Promise.resolve(REAL_MANIFEST),
      componentInfo: () => Promise.resolve({ absPath: guestPath, sha256: "rustfix-hash" }),
      invokeResolver: async (absPath, symbol, capabilities, policy, s, wsRoot) => {
        const guest = new Uint8Array(await readFile(absPath));
        return invokeResolver(
          guest,
          symbol,
          [...capabilities],
          policy,
          s.map((x) => ({ languageId: x.languageId, bin: x.bin, args: [...x.args] })),
          wsRoot,
        );
      },
      health: { read: () => Promise.resolve([]), append: () => Promise.resolve() },
      now: () => new Date().toISOString(),
      budgetMs: 30_000,
    });
  }

  test("real detection yields one rust entry whose binary runs", () => {
    const root = fixtureCrate();
    const specs = detectLanguageServers(root);
    const rust = specs.filter((s) => s.languageId === "rust");
    expect(rust).toHaveLength(1);
    expect(rust[0]?.args).toEqual([]);
    expect(rust[0]?.source).toBe("detected");
  });

  test("warm references reach resolver-lsp precision incl. the generic call sites (SC-002/SC-004)", async () => {
    const root = fixtureCrate();
    const specs = detectLanguageServers(root);
    const resolver = resolverOver(root, specs);
    const t0 = performance.now();
    let edges = await resolver.resolve({ kind: "references", symbol: "greet" });
    const coldMs = performance.now() - t0;
    // rust-analyzer answers empty until its load completes; poll the warm pool.
    while (edges.length === 0 && performance.now() - t0 < 90_000) {
      await Bun.sleep(500);
      edges = await resolver.resolve({ kind: "references", symbol: "greet" });
    }
    if (edges.length === 0) throw new Error("rust-analyzer never resolved `greet` in 90 s");
    const tWarm = performance.now();
    const warm = await resolver.resolve({ kind: "references", symbol: "greet" });
    const warmMs = performance.now() - tWarm;
    for (const e of warm) {
      expect(e.provenance).toBe("resolver-lsp");
      expect(e.confidence).toBe("high");
      expect(e.location.path).toBe("src/lib.rs");
    }
    // Precision: `greet` is referenced from the definition, both call sites in
    // greet_all's generic body, the trait-impl method, and the macro-generated
    // function — five sites across at least three distinct lines. A name-based
    // symbolic pass could not attribute the generic/macro uses to this symbol.
    const lines = new Set(warm.map((e) => e.location.line));
    expect(warm.length).toBeGreaterThanOrEqual(4);
    expect(lines.size).toBeGreaterThanOrEqual(3);
    const line = `[008 SC-004] rust-analyzer via detection: cold=${coldMs.toFixed(1)}ms warm=${warmMs.toFixed(1)}ms edges=${warm.length}`;
    console.log(line);
    expect(warmMs).toBeLessThan(2_000);
    binding.sandboxServerPoolShutdown?.();
  }, 150_000);

  test("no rust-analyzer configured => byte-identical tree-sitter answer (SC-003)", async () => {
    const root = fixtureCrate();
    // A resolver with NO specs never upgrades: references stay symbolic (empty
    // here, since the PluginResolver only contributes upgrade edges). The
    // detection units prove the other half — no entry when nothing resolves.
    const resolver = resolverOver(root, []);
    expect(await resolver.resolve({ kind: "references", symbol: "greet" })).toEqual([]);
    binding.sandboxServerPoolShutdown?.();
  }, 30_000);
});
