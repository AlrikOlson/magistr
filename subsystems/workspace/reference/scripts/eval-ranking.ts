/**
 * Offline retrieval eval for the ranking pipeline (SDD §11.1) — MRR, Hit@1
 * and Hit@5 over the labeled corpus in eval/ranking-corpus.jsonl, and the
 * enforcement half of §7.4's rule: **no ranking-weight change merges without
 * a before/after MRR number**.
 *
 * eval/ranking-baseline.json records the metrics next to a hash of
 * RANKING_WEIGHTS. The default run recomputes both and fails (exit 1) when
 * either drifted from the baseline — so CI refuses a weight or behavior
 * change that did not land with re-recorded numbers. `--record` prints the
 * before/after pair and rewrites the baseline; put those numbers in the
 * commit message.
 */

import { createHash } from "node:crypto";
import { readFileSync, writeFileSync } from "node:fs";
import path from "node:path";
import { loadCore } from "../src/adapters/core";
import { hitContextsOf, sanitizeContributions } from "../src/protocol/ranking-plugin";
import { PLUGIN_FUEL_CAP, PLUGIN_MEMORY_CAP, parsePluginManifest } from "../src/tools/plugins";
import {
  type CandidateGroup,
  type PluginFeatureLookup,
  pluginFeatureKey,
  RANKING_WEIGHTS,
  rankGroups,
} from "../src/tools/ranking";

const ROOT = path.join(import.meta.dir, "..");
const CORPUS = path.join(ROOT, "eval", "ranking-corpus.jsonl");
const BASELINE = path.join(ROOT, "eval", "ranking-baseline.json");

interface CorpusEntry {
  readonly id: string;
  readonly query: string;
  /** The file a human labeled as the answer the agent needed. */
  readonly expected: string;
  readonly candidates: readonly CandidateGroup[];
}

interface Baseline {
  readonly weightsHash: string;
  readonly corpusEntries: number;
  readonly mrr: number;
  readonly hit1: number;
  readonly hit5: number;
}

/** Stable stringify (sorted keys, recursively) so the hash ignores key order. */
function stable(value: unknown): string {
  if (Array.isArray(value)) return `[${value.map(stable).join(",")}]`;
  if (value !== null && typeof value === "object") {
    const entries = Object.entries(value as Record<string, unknown>)
      .sort(([a], [b]) => (a < b ? -1 : 1))
      .map(([k, v]) => `${JSON.stringify(k)}:${stable(v)}`);
    return `{${entries.join(",")}}`;
  }
  return JSON.stringify(value);
}

function weightsHash(): string {
  return createHash("sha256").update(stable(RANKING_WEIGHTS)).digest("hex").slice(0, 16);
}

function loadCorpus(): CorpusEntry[] {
  return readFileSync(CORPUS, "utf8")
    .split("\n")
    .filter((line) => line.trim().length > 0)
    .map((line) => JSON.parse(line) as CorpusEntry);
}

function evaluate(
  corpus: readonly CorpusEntry[],
  featuresFor?: (entry: CorpusEntry) => PluginFeatureLookup | undefined,
): Omit<Baseline, "weightsHash"> {
  let reciprocal = 0;
  let hit1 = 0;
  let hit5 = 0;
  for (const entry of corpus) {
    const ranked = rankGroups(entry.candidates, entry.query, featuresFor?.(entry));
    const rank = ranked.findIndex((g) => g.file === entry.expected) + 1;
    if (rank === 0) continue; // expected file absent: reciprocal 0
    reciprocal += 1 / rank;
    if (rank === 1) hit1 += 1;
    if (rank <= 5) hit5 += 1;
  }
  const n = corpus.length;
  const round = (x: number): number => Math.round(x * 10000) / 10000;
  return {
    corpusEntries: n,
    mrr: round(reciprocal / n),
    hit1: round(hit1 / n),
    hit5: round(hit5 / n),
  };
}

function loadBaseline(): Baseline | null {
  try {
    return JSON.parse(readFileSync(BASELINE, "utf8")) as Baseline;
  } catch {
    return null;
  }
}

/**
 * Signal mode (chunk:m5-ranking-shadow, FR-007): `--signal <plugin-dir>`
 * computes the labeled-corpus metrics with and without the signal's features
 * applied at their table weight — the before/after pair a promotion decision
 * is made against. The default no-flag run is untouched (baseline gate).
 */
async function signalLookups(
  dir: string,
  corpus: readonly CorpusEntry[],
): Promise<Map<string, PluginFeatureLookup>> {
  const manifest = parsePluginManifest(readFileSync(path.join(ROOT, dir, "plugin.toml"), "utf8"));
  if (!manifest.ok) {
    throw new Error(`${dir}/plugin.toml: ${manifest.errors.join("; ")}`);
  }
  const guest = readFileSync(path.join(ROOT, dir, manifest.manifest.componentPath));
  const namespace = path.basename(dir);
  const { binding } = loadCore();
  const lane = binding.sandboxRankingContribute;
  if (lane === undefined) {
    throw new Error("this core binding lacks the Wasmtime sandbox — build the native artifact");
  }
  const policy = {
    fuel: Math.min(manifest.manifest.limits.fuel ?? PLUGIN_FUEL_CAP, PLUGIN_FUEL_CAP),
    memoryCapBytes: Math.min(
      manifest.manifest.limits.memoryCapBytes ?? PLUGIN_MEMORY_CAP,
      PLUGIN_MEMORY_CAP,
    ),
  };
  const lookups = new Map<string, PluginFeatureLookup>();
  for (const entry of corpus) {
    const hits = hitContextsOf(entry.candidates, entry.query, 10_000);
    const outcome = await lane(
      new Uint8Array(guest),
      hits.map((h) => ({ ...h })),
      [],
      policy,
    );
    if (outcome.outcome !== "ok") {
      throw new Error(`contribute failed on ${entry.id}: ${outcome.outcome} ${outcome.detail}`);
    }
    const map = new Map<string, ReturnType<typeof sanitizeContributions>>();
    outcome.contributions.forEach((c, i) => {
      const hit = hits[i];
      if (hit === undefined) return;
      const features = sanitizeContributions(namespace, [...c.features]);
      if (features.length > 0)
        map.set(pluginFeatureKey(hit.file, hit.line, hit.column, hit.excerpt), features);
    });
    lookups.set(entry.id, map);
  }
  return lookups;
}

const record = process.argv.includes("--record");
const corpus = loadCorpus();

const signalArg = process.argv.indexOf("--signal");
if (signalArg >= 0) {
  const dir = process.argv[signalArg + 1];
  if (dir === undefined) {
    console.error("usage: bun run eval:rank --signal <plugin-dir>");
    process.exit(2);
  }
  const before = evaluate(corpus);
  const lookups = await signalLookups(dir, corpus);
  const after = evaluate(corpus, (entry) => lookups.get(entry.id));
  console.log(`signal eval: ${path.basename(dir)} over ${corpus.length} labeled queries`);
  console.log(`before: MRR ${before.mrr}, Hit@1 ${before.hit1}, Hit@5 ${before.hit5}`);
  console.log(`after:  MRR ${after.mrr}, Hit@1 ${after.hit1}, Hit@5 ${after.hit5}`);
  console.log(
    before.mrr === after.mrr && before.hit1 === after.hit1 && before.hit5 === after.hit5
      ? "delta: zero — the signal's features did not change any labeled ordering (a valid result)"
      : `delta: MRR ${Math.round((after.mrr - before.mrr) * 10000) / 10000}, ` +
          `Hit@1 ${Math.round((after.hit1 - before.hit1) * 10000) / 10000}, ` +
          `Hit@5 ${Math.round((after.hit5 - before.hit5) * 10000) / 10000}`,
  );
  console.log("record this pair in the commit that lands the signal's weight (SDD §7.4)");
  process.exit(0);
}

const metrics = evaluate(corpus);
const hash = weightsHash();
const baseline = loadBaseline();

console.log(
  `ranking eval: ${metrics.corpusEntries} labeled queries — ` +
    `MRR ${metrics.mrr}, Hit@1 ${metrics.hit1}, Hit@5 ${metrics.hit5} (weights ${hash})`,
);

if (record) {
  if (baseline !== null) {
    console.log(
      `before: MRR ${baseline.mrr}, Hit@1 ${baseline.hit1}, Hit@5 ${baseline.hit5} ` +
        `(weights ${baseline.weightsHash})`,
    );
    console.log(`after:  MRR ${metrics.mrr}, Hit@1 ${metrics.hit1}, Hit@5 ${metrics.hit5}`);
    console.log("include this before/after pair in the commit message (SDD §7.4)");
  } else {
    console.log("no previous baseline — recording the first one");
  }
  writeFileSync(BASELINE, `${JSON.stringify({ weightsHash: hash, ...metrics }, null, 2)}\n`);
  console.log(`baseline written to ${path.relative(ROOT, BASELINE)}`);
  process.exit(0);
}

if (baseline === null) {
  console.error("FAIL: no recorded baseline — run `bun run eval:rank --record` and commit it");
  process.exit(1);
}
if (baseline.weightsHash !== hash) {
  console.error(
    `FAIL: RANKING_WEIGHTS changed (${baseline.weightsHash} -> ${hash}) without re-recording. ` +
      "Run `bun run eval:rank --record` and include the before/after MRR in the commit (SDD §7.4).",
  );
  process.exit(1);
}
if (
  baseline.mrr !== metrics.mrr ||
  baseline.hit1 !== metrics.hit1 ||
  baseline.hit5 !== metrics.hit5 ||
  baseline.corpusEntries !== metrics.corpusEntries
) {
  console.error(
    `FAIL: eval drifted from the baseline (MRR ${baseline.mrr} -> ${metrics.mrr}, ` +
      `Hit@1 ${baseline.hit1} -> ${metrics.hit1}, Hit@5 ${baseline.hit5} -> ${metrics.hit5}, ` +
      `entries ${baseline.corpusEntries} -> ${metrics.corpusEntries}). ` +
      "Pipeline behavior or corpus changed — re-record with `bun run eval:rank --record` " +
      "and include the before/after numbers in the commit.",
  );
  process.exit(1);
}
console.log("baseline check passed");
