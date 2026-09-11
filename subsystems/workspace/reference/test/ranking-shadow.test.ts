/**
 * Ranking-signal shadow/promotion suites (chunk:m5-ranking-shadow, feature
 * 003-ranking-shadow). US1 here pins the leash: plugin values reach scores
 * ONLY through RANKING_WEIGHTS.plugin (quickstart Scenario 1/2 pins live in
 * this file as the suites land per story).
 */

import { describe, expect, test } from "bun:test";
import { readFileSync } from "node:fs";
import path from "node:path";
import type { EnginePort, IndexPort } from "../src/ports";
import { PORT_VERSIONS } from "../src/ports";
import {
  hitContextsOf,
  type RankingGuestOutcome,
  type RankingHitContext,
  RankingSignals,
  type RankingSignalsDeps,
  type RawFeature,
  sanitizeContributions,
} from "../src/protocol/ranking-plugin";
import type { PluginHealthRecord } from "../src/tools/plugins";
import {
  type CandidateGroup,
  type PluginFeature,
  pluginFeatureKey,
  RANKING_WEIGHTS,
  rankGroups,
} from "../src/tools/ranking";
import { runSearch, type SearchDeps } from "../src/tools/search";

const REAL_MANIFEST = readFileSync(
  path.join(import.meta.dir, "..", "plugins", "ranking-visibility", "plugin.toml"),
  "utf8",
);

/** Wait for a fire-and-forget lane to settle. */
async function until(check: () => boolean, tries = 200): Promise<void> {
  for (let i = 0; i < tries; i += 1) {
    if (check()) return;
    await new Promise((resolve) => setTimeout(resolve, 5));
  }
  throw new Error("condition never settled");
}

/** The fake guest: exported=1.0 for export/pub-leading declarations. */
function fakeContribute(hits: readonly RankingHitContext[]): RankingGuestOutcome {
  return {
    outcome: "ok",
    detail: "",
    fuelUsed: 1,
    contributions: hits.map((hit) => ({
      features:
        hit.kindClass === "declaration"
          ? [
              {
                name: "exported",
                value: /^\s*(export|pub|public)\b/.test(hit.excerpt) ? 1.0 : 0.0,
              },
            ]
          : [],
    })),
  };
}

interface Env {
  readonly deps: RankingSignalsDeps;
  readonly runtime: RankingSignals;
  shadowLines(): string[];
  promotionsText(): string | null;
  readonly health: PluginHealthRecord[];
  readonly contributeCalls: RankingHitContext[][];
}

function makeEnv(
  options: {
    tier?: "blessed" | "verified";
    sha256?: string;
    promotionsSeed?: string | null;
    contribute?: RankingSignalsDeps["contribute"];
  } = {},
): Env {
  let shadow = "";
  let promotions: string | null = options.promotionsSeed ?? null;
  const health: PluginHealthRecord[] = [];
  const contributeCalls: RankingHitContext[][] = [];
  const deps: RankingSignalsDeps = {
    inventory: () =>
      Promise.resolve([
        {
          name: "workbench.ranking-visibility",
          source: "sideload" as const,
          dir: "plugins/ranking-visibility",
          ...(options.tier === undefined ? {} : { tier: options.tier }),
        },
      ]),
    readManifest: () => Promise.resolve(REAL_MANIFEST),
    componentInfo: () =>
      Promise.resolve({ absPath: "/x/guest.wasm", sha256: options.sha256 ?? "hash-1" }),
    contribute:
      options.contribute ??
      ((_absPath, hits) => {
        contributeCalls.push([...hits]);
        return Promise.resolve(fakeContribute(hits));
      }),
    health: {
      read: () => Promise.resolve([...health]),
      append: (record) => {
        health.push(record);
        return Promise.resolve();
      },
    },
    promotions: {
      read: () => Promise.resolve(promotions),
      write: (text) => {
        promotions = text;
        return Promise.resolve();
      },
    },
    shadowLog: {
      size: () => Promise.resolve(shadow.length),
      read: () => Promise.resolve(shadow),
      write: (text) => {
        shadow = text;
        return Promise.resolve();
      },
      append: (line) => {
        shadow += line;
        return Promise.resolve();
      },
    },
    now: () => "2026-08-22T00:00:00Z",
  };
  return {
    deps,
    runtime: new RankingSignals(deps),
    shadowLines: () => shadow.split("\n").filter((l) => l.length > 0),
    promotionsText: () => promotions,
    health,
    contributeCalls,
  };
}

function declCandidates(): CandidateGroup[] {
  return [
    // The exported file sorts lexicographically LAST so a promoted feature's
    // reorder is observable against the baseline tie-break.
    {
      file: "src/a-internal.ts",
      junk: false,
      test: false,
      hits: [{ line: 1, column: 1, excerpt: "function needle() {" }],
    },
    {
      file: "src/z-exported.ts",
      junk: false,
      test: false,
      hits: [{ line: 1, column: 1, excerpt: "export function needle() {" }],
    },
  ];
}

/** Two files whose stage-5 scores tie without plugin features. */
function tiedCandidates(): CandidateGroup[] {
  return [
    {
      file: "src/alpha.ts",
      junk: false,
      test: false,
      hits: [{ line: 3, column: 1, excerpt: "function needle() {" }],
    },
    {
      file: "src/beta.ts",
      junk: false,
      test: false,
      hits: [{ line: 3, column: 1, excerpt: "function needle() {" }],
    },
  ];
}

describe("US1 — features only, first-party combiner preserved", () => {
  test("no lookup passed ⇒ identical to the baseline pipeline (SC-001)", () => {
    const withOmitted = rankGroups(tiedCandidates(), "needle");
    const withUndefined = rankGroups(tiedCandidates(), "needle", undefined);
    expect(withUndefined).toEqual(withOmitted);
    // Tie broken lexicographically, the first-party rule.
    expect(withOmitted.map((g) => g.file)).toEqual(["src/alpha.ts", "src/beta.ts"]);
  });

  test("a weighted feature reorders exactly via the combiner; removal restores baseline", () => {
    const boost: PluginFeature[] = [{ name: "ranking-visibility/exported", value: 1.0 }];
    const lookup = new Map([[pluginFeatureKey("src/beta.ts", 3, 1, "function needle() {"), boost]]);
    const ranked = rankGroups(tiedCandidates(), "needle", lookup);
    expect(ranked.map((g) => g.file)).toEqual(["src/beta.ts", "src/alpha.ts"]);

    // Same features, weight zero (unlisted name) ⇒ baseline order — weights
    // are opt-in per feature, the ONLY path to a score (FR-002).
    const unlisted = new Map([
      [
        pluginFeatureKey("src/beta.ts", 3, 1, "function needle() {"),
        [{ name: "ranking-visibility/unknown", value: 9.9 }],
      ],
    ]);
    expect(rankGroups(tiedCandidates(), "needle", unlisted).map((g) => g.file)).toEqual([
      "src/alpha.ts",
      "src/beta.ts",
    ]);
  });

  test("the applied boost is exactly Σ weight·value, no plugin-set scores", () => {
    const weight = RANKING_WEIGHTS.plugin["ranking-visibility/exported"];
    expect(weight).toBe(0.5);
    // A huge value cannot leapfrog the definition-first rule: a declaration
    // group still outranks a call-site group regardless of plugin features.
    const candidates: CandidateGroup[] = [
      {
        file: "src/decl.ts",
        junk: false,
        test: false,
        hits: [{ line: 1, column: 1, excerpt: "function needle() {" }],
      },
      {
        file: "src/call.ts",
        junk: false,
        test: false,
        hits: [{ line: 1, column: 1, excerpt: "  needle();" }],
      },
    ];
    const lookup = new Map([
      [
        pluginFeatureKey("src/call.ts", 1, 1, "  needle();"),
        [{ name: "ranking-visibility/exported", value: 999999 }],
      ],
    ]);
    const ranked = rankGroups(candidates, "needle", lookup);
    expect(ranked.map((g) => g.file)).toEqual(["src/decl.ts", "src/call.ts"]);
  });
});

describe("US1 — host contribution hygiene (contracts/ranking.md)", () => {
  test("names are namespaced per plugin; spoofing and malformed values dropped", () => {
    const raw: RawFeature[] = [
      { name: "exported", value: 1.0 },
      { name: "other-plugin/spoof", value: 1.0 }, // slash ⇒ spoof attempt, dropped
      { name: "", value: 1.0 }, // unnamed, dropped
      { name: "nan", value: Number.NaN }, // non-finite, dropped
      { name: "inf", value: Number.POSITIVE_INFINITY }, // non-finite, dropped
      { name: "huge", value: 1e9 }, // beyond magnitude cap, dropped
      { name: "negative", value: -0.25 }, // finite negative is legal
    ];
    expect(sanitizeContributions("ranking-visibility", raw)).toEqual([
      { name: "ranking-visibility/exported", value: 1.0 },
      { name: "ranking-visibility/negative", value: -0.25 },
    ]);
  });
});

describe("US2 — shadow by default, explicit promotion (quickstart Scenario 2)", () => {
  test("(a) fresh install: served order unchanged, shadow log accumulates, applied empty", async () => {
    const env = makeEnv();
    await env.runtime.boot();
    const baseline = rankGroups(declCandidates(), "needle").map((g) => g.file);
    const served = rankGroups(declCandidates(), "needle", env.runtime.promotedFeatures()).map(
      (g) => g.file,
    );
    expect(served).toEqual(baseline);
    env.runtime.afterSearch("needle", declCandidates(), served);
    await until(() => env.shadowLines().length === 1);
    const entry = JSON.parse(env.shadowLines()[0] as string) as {
      applied: string[];
      queryHash: string;
      servedOrder: string[];
      contributions: Record<string, Record<string, PluginFeature[]>>;
    };
    expect(entry.applied).toEqual([]);
    expect(entry.servedOrder).toEqual(served);
    expect(entry.queryHash).toHaveLength(16);
    const rows = entry.contributions["ranking-visibility"] ?? {};
    expect(Object.values(rows).flat()).toContainEqual({
      name: "ranking-visibility/exported",
      value: 1.0,
    });
    // Shadow NEVER applies: even with a warm cache, promotedFeatures is
    // undefined until promotion.
    expect(env.runtime.promotedFeatures()).toBeUndefined();
  });

  test("(b) promotion persists, survives restart, and applies from the warm cache", async () => {
    const env = makeEnv();
    await env.runtime.boot();
    env.runtime.afterSearch("needle", declCandidates(), []);
    await until(() => env.shadowLines().length === 1);
    const promoted = await env.runtime.promote("ranking-visibility");
    expect(promoted.ok).toBe(true);
    expect(env.promotionsText()).toContain('"promoted": true');
    expect(env.promotionsText()).toContain("hash-1");
    const lookup = env.runtime.promotedFeatures();
    expect(lookup).toBeDefined();
    const reordered = rankGroups(declCandidates(), "needle", lookup).map((g) => g.file);
    expect(reordered).toEqual(["src/z-exported.ts", "src/a-internal.ts"]);
    // The applied order is EXACTLY the logged features through the combiner
    // (SC-002): replaying the shadow entry's contributions reproduces it.
    const entry = JSON.parse(env.shadowLines()[0] as string) as {
      contributions: Record<string, Record<string, PluginFeature[]>>;
    };
    const replay = new Map(Object.entries(entry.contributions["ranking-visibility"] ?? {}));
    expect(rankGroups(declCandidates(), "needle", replay).map((g) => g.file)).toEqual(reordered);

    // Restart: same deps (promotions file kept), fresh runtime — promotion
    // survives; the cache starts cold so features re-warm before applying.
    const restarted = new RankingSignals(env.deps);
    await restarted.boot();
    expect(restarted.statuses()[0]?.promoted).toBe(true);
    expect(restarted.promotedFeatures()).toBeUndefined(); // cold cache ⇒ baseline (d)
    restarted.afterSearch("needle", declCandidates(), []);
    await until(() => restarted.promotedFeatures() !== undefined);
  });

  test("(c) demotion restores the baseline", async () => {
    const env = makeEnv();
    await env.runtime.boot();
    env.runtime.afterSearch("needle", declCandidates(), []);
    await until(() => env.shadowLines().length === 1);
    await env.runtime.promote("ranking-visibility");
    expect(env.runtime.promotedFeatures()).toBeDefined();
    const demoted = await env.runtime.demote("ranking-visibility");
    expect(demoted.ok).toBe(true);
    expect(env.runtime.promotedFeatures()).toBeUndefined();
    expect(
      rankGroups(declCandidates(), "needle", env.runtime.promotedFeatures()).map((g) => g.file),
    ).toEqual(["src/a-internal.ts", "src/z-exported.ts"]);
  });

  test("(e) a failing signal breaches, degrades to baseline, and goes quiet", async () => {
    const env = makeEnv({
      contribute: () => Promise.reject(new Error("guest exploded")),
    });
    await env.runtime.boot();
    await env.runtime.promote("ranking-visibility");
    env.runtime.afterSearch("needle", declCandidates(), []);
    await until(() => env.health.length === 1);
    expect(env.health[0]?.reason).toContain("guest exploded");
    expect(env.runtime.promotedFeatures()).toBeUndefined();
    expect(env.runtime.statuses()[0]?.healthy).toBe(false);
    // Unhealthy signals are skipped entirely: no more lane calls, no log.
    env.runtime.afterSearch("needle", declCandidates(), []);
    await new Promise((resolve) => setTimeout(resolve, 25));
    expect(env.health.length).toBe(1);
  });

  test("(f) promoting an unknown signal is refused without touching state", async () => {
    const env = makeEnv();
    await env.runtime.boot();
    const refused = await env.runtime.promote("ranking-nonexistent");
    expect(refused.ok).toBe(false);
    expect(refused.detail).toContain("unknown ranking signal");
    expect(env.promotionsText()).toBeNull();
  });

  test("(g) a changed component hash resets promotion to shadow", async () => {
    const env = makeEnv();
    await env.runtime.boot();
    await env.runtime.promote("ranking-visibility");
    // New bytes shipped: same promotions file, different componentHash.
    const changed = makeEnv({
      sha256: "hash-2",
      promotionsSeed: env.promotionsText(),
    });
    await changed.runtime.boot();
    expect(changed.runtime.statuses()[0]?.promoted).toBe(false);
    changed.runtime.afterSearch("needle", declCandidates(), []);
    await new Promise((resolve) => setTimeout(resolve, 25));
    expect(changed.runtime.promotedFeatures()).toBeUndefined();
  });

  test("shadow-log size cap evicts oldest entries, never fails the append", async () => {
    const env = makeEnv();
    await env.runtime.boot();
    // Pre-fill the log to just under the cap so the next entry must evict.
    const filler = `${JSON.stringify({ pad: "x".repeat(1024) })}\n`;
    let text = "";
    while (text.length < 5 * 1024 * 1024 - 512) text += filler;
    await env.deps.shadowLog.write(text);
    const before = env.shadowLines().length;
    env.runtime.afterSearch("needle", declCandidates(), []);
    await until(() => env.shadowLines().length < before);
    // Newest half kept plus the fresh entry; the fresh entry is last.
    const lines = env.shadowLines();
    expect(lines.length).toBeLessThan(before);
    expect(lines[lines.length - 1]).toContain("queryHash");
  });
});

describe("SC-005 — no sandbox invocation on the search response path", () => {
  function searchDeps(rankingSignals: SearchDeps["rankingSignals"]): SearchDeps {
    const files: Record<string, string> = {
      "src/a-internal.ts": "function needle() {",
      "src/z-exported.ts": "export function needle() {",
    };
    const engine: EnginePort = {
      version: PORT_VERSIONS.engine,
      parse: () => Promise.resolve({ ok: true }),
      search: (request) =>
        Promise.resolve(
          request.paths.flatMap((p) => {
            const content = files[p];
            if (content === undefined || !content.includes(request.pattern)) return [];
            return [
              {
                location: { path: p, line: 1, column: content.indexOf(request.pattern) + 1 },
                excerpt: content,
              },
            ];
          }),
        ),
      rewrite: () => Promise.reject(new Error("not under test")),
    };
    const index: IndexPort = {
      version: PORT_VERSIONS.index,
      status: () => Promise.resolve({ tier: 1, generation: 1, building: false }),
      build: () => Promise.reject(new Error("not under test")),
      querySymbols: () => Promise.resolve({ symbols: [], freshness: "warm" }),
      queryReferences: () => Promise.resolve({ edges: [], freshness: "warm" }),
      queryOutline: () => Promise.resolve({ files: [], freshness: "warm" }),
      queryImportGraph: () => Promise.resolve({ fanIn: [], fanOut: [], freshness: "warm" }),
      upsertFile: () => Promise.resolve(),
      refreshFile: () => Promise.resolve(),
      removeFile: () => Promise.resolve(),
    };
    return {
      engine,
      index,
      workspace: {
        list: () => Promise.resolve(Object.keys(files).map((p) => ({ path: p, junk: false }))),
        read: (p) => Promise.resolve(files[p] ?? ""),
      },
      ...(rankingSignals === undefined ? {} : { rankingSignals }),
    };
  }

  test("the response resolves while contribute is still pending, in both modes", async () => {
    let release: (() => void) | undefined;
    let invoked = 0;
    const gate = new Promise<void>((resolve) => {
      release = resolve;
    });
    const env = makeEnv({
      contribute: async (_absPath, hits) => {
        invoked += 1;
        await gate;
        return fakeContribute(hits);
      },
    });
    await env.runtime.boot();
    const deps = searchDeps(env.runtime);

    // Shadow mode: the search response settles with the lane still gated.
    const envelope = await runSearch(deps, { query: "needle" }, 1);
    expect(envelope.ok).toBe(true);
    await until(() => invoked === 1);
    // The response already resolved; the pending contribute proves nothing
    // on the response path awaited it.
    release?.();
    await until(() => env.shadowLines().length === 1);

    // Promoted mode: still no await — the warm cache serves synchronously.
    await env.runtime.promote("ranking-visibility");
    const promotedEnvelope = await runSearch(deps, { query: "needle" }, 1);
    expect(promotedEnvelope.ok).toBe(true);
    if (promotedEnvelope.ok) {
      // Warm cache: the exported declaration outranks the internal one.
      expect(promotedEnvelope.results.map((g) => g.file)).toEqual([
        "src/z-exported.ts",
        "src/a-internal.ts",
      ]);
    }
  });

  test("hitContextsOf derives lane inputs with stage-1/2 classes and a hit cap", () => {
    const contexts = hitContextsOf(declCandidates(), "needle");
    expect(contexts).toHaveLength(2);
    expect(contexts[0]?.kindClass).toBe("declaration");
    expect(contexts[0]?.fileClass).toBe("source");
    expect(hitContextsOf(declCandidates(), "needle", 1)).toHaveLength(1);
  });
});
