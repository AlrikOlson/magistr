/**
 * The references perf gate (Constitution VI, chunk:m1-search-structural):
 * reference query p95 under 50ms, measured through the engine child's real
 * `index/references` frames against a warm Tier-1 index.
 *
 * The corpus mirrors bench-index's deterministic generator, with one
 * reference-heavy twist: every bulk module imports and calls a single shared
 * symbol, so the hot query exercises the FTS5 candidate cap, the import
 * linkage, and the per-file line scan at their bounds. The query mix rotates
 * hot / rare / miss so the p95 is not a single shape.
 *
 * CI runs the 500k-LOC profile alongside bench-index; the constitution's
 * 1M-LOC release profile is the same script with `--loc 1000000`, and
 * `--root <path>` (specs/016) measures an existing tree instead, sampling
 * hot / rare / miss queries from the symbols the index actually holds.
 *
 * Usage: bun run scripts/bench-refs.ts [--report-only] [--stub] [--loc N]
 *          [--root <path>] [--json <file>] [--keep-index] [--deadline <s>]
 *   --stub uses the JS stub core (pipeline smoke, not a real measurement).
 */

import { Database } from "bun:sqlite";
import { mkdirSync, mkdtempSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import path from "node:path";
import { loadCore } from "../src/adapters/core/loader";
import { EngineSupervisor } from "../src/adapters/engine-ipc";
import {
  BENCH_SCHEMA,
  type BenchReport,
  BenchUsageError,
  type CoreFlavorLabel,
  type CorpusFacts,
  countLoc,
  describeGate,
  type Gate,
  gitFacts,
  guardIndexDir,
  indexDirFor,
  MISS_NAME,
  machineFacts,
  parseBenchArgs,
  percentiles,
  profileName,
  REFS_P95_GATE_MS,
  type ReferenceQuery,
  ROOT_DEADLINE_S,
  refsP95Gate,
  removeIndexDir,
  resolveRoot,
  sampleReferenceQueries,
  writeReport,
} from "./bench-common";

const ITERATIONS_PER_QUERY = 40;

function tsModule(fileIndex: number, lines: number): string {
  const parts: string[] = [`import { shared_ref_target } from "../src/shared";`];
  let line = 1;
  let fn = 0;
  while (line < lines) {
    parts.push(
      `export function bulk_${fileIndex}_${fn}(value: number): number {`,
      `  const doubled = shared_ref_target(value * 2);`,
      `  const label = "file ${fileIndex} fn ${fn}";`,
      `  if (doubled > 10) return doubled + label.length;`,
      `  return doubled;`,
      `}`,
    );
    line += 6;
    fn += 1;
  }
  return `${parts.join("\n")}\n`;
}

function makeCorpus(loc: number): { root: string; writtenLoc: number } {
  const root = mkdtempSync(path.join(tmpdir(), "wb-bench-refs-"));
  mkdirSync(path.join(root, "src"), { recursive: true });
  mkdirSync(path.join(root, "bulk"), { recursive: true });

  writeFileSync(
    path.join(root, "src", "shared.ts"),
    "export function shared_ref_target(value: number): number {\n  return value;\n}\n",
  );
  let writtenLoc = 3;

  const linesPerFile = 250;
  const files = Math.ceil((loc - writtenLoc) / linesPerFile);
  for (let i = 0; i < files; i += 1) {
    writeFileSync(path.join(root, "bulk", `mod_${i}.ts`), tsModule(i, linesPerFile));
    writtenLoc += linesPerFile + 2;
  }
  return { root, writtenLoc };
}

/** hot: defined once, called from every bulk module (caps in play);
 *  rare: one definition, references only inside its own file;
 *  miss: no rows anywhere. */
const SYNTHETIC_QUERIES: readonly ReferenceQuery[] = [
  { role: "hot", name: "shared_ref_target", definitions: 1 },
  { role: "rare", name: "bulk_5_2", definitions: 1 },
  { role: "miss", name: MISS_NAME, definitions: 0 },
];

let args: ReturnType<typeof parseBenchArgs>;
let root: string;
let corpusKind: "synthetic" | "root";
try {
  args = parseBenchArgs(process.argv.slice(2));
  if (args.root !== undefined) {
    root = resolveRoot(args.root);
    guardIndexDir(root, args.keepIndex);
    corpusKind = "root";
    console.log(`root: ${root}`);
  } else {
    console.log(`generating ~${args.loc} LOC corpus…`);
    const corpus = makeCorpus(args.loc);
    root = corpus.root;
    console.log(`corpus ready: ${corpus.writtenLoc} LOC at ${root}`);
    corpusKind = "synthetic";
  }
} catch (error) {
  if (error instanceof BenchUsageError) {
    console.error(`usage: ${error.message}`);
    process.exit(2);
  }
  throw error;
}

const deadlineS = args.deadlineS ?? (corpusKind === "root" ? ROOT_DEADLINE_S : 120);
const useStub = args.stub;
const reportOnly = args.reportOnly;

const coreFlavor: CoreFlavorLabel = useStub
  ? "stub"
  : (() => {
      try {
        return loadCore().flavor;
      } catch {
        return "unknown";
      }
    })();

const supervisor = new EngineSupervisor({
  env: useStub ? { WORKBENCH_ENGINE_STUB_CORE: "1" } : {},
});

let failed = false;
const fail = (message: string): void => {
  console.error(`FAIL: ${message}`);
  failed = true;
};

const measurements: Record<string, unknown> = {};
const gates: Gate[] = [];
let corpus: CorpusFacts | undefined;

try {
  const buildStartNs = Bun.nanoseconds();
  await supervisor.call("index/build", { root });
  const deadline = buildStartNs + deadlineS * 1e9;
  let status = { tier: 0, building: true, filesIndexed: 0, filesTotal: 0 };
  while (Bun.nanoseconds() < deadline) {
    status = (await supervisor.call("index/status", {})) as typeof status;
    if (status.tier === 1 && !status.building) break;
    await new Promise((r) => setTimeout(r, 200));
  }
  const tier1Reached = status.tier === 1 && !status.building;
  Object.assign(measurements, {
    tier1S: Number(((Bun.nanoseconds() - buildStartNs) / 1e9).toFixed(3)),
    tier1Reached,
    filesIndexed: status.filesIndexed,
    filesTotal: status.filesTotal,
  });

  if (!tier1Reached) {
    fail(
      `build did not reach tier 1 within ${deadlineS}s ` +
        `(${status.filesIndexed}/${status.filesTotal} files) — measurement not taken`,
    );
  } else {
    // Corpus facts and, under --root, the sampled queries — both read from
    // the on-disk index after the build settled, outside any timed window.
    let queries: readonly ReferenceQuery[] = SYNTHETIC_QUERIES;
    const db = new Database(path.join(indexDirFor(root), "index.db"), { readonly: true });
    try {
      const loc = countLoc(db, root);
      const repo = corpusKind === "root" ? gitFacts(root) : undefined;
      corpus = {
        kind: corpusKind,
        ...(corpusKind === "root" ? { root } : {}),
        ...loc,
        ...(repo !== undefined ? { repo } : {}),
      };
      if (corpusKind === "root") queries = sampleReferenceQueries(db);
    } finally {
      db.close();
    }
    console.log(
      `corpus: ${corpus.loc} source LOC over ${corpus.sourceFiles} source files; queries: ` +
        queries.map((q) => `${q.role}=${q.name}`).join(", "),
    );

    // Warmup: first calls page the index into cache like a live server's would be.
    for (const q of queries) await supervisor.call("index/references", { name: q.name });

    const samples: number[] = [];
    const perQuery = new Map<string, { samples: number[]; edges: number }>();
    for (const q of queries) perQuery.set(q.name, { samples: [], edges: 0 });
    for (let i = 0; i < ITERATIONS_PER_QUERY * queries.length; i += 1) {
      const query = queries[i % queries.length] as ReferenceQuery;
      const start = Bun.nanoseconds();
      const result = (await supervisor.call("index/references", { name: query.name })) as {
        edges: readonly { ref_kind: string; confidence: string; provenance: string }[];
      };
      const ms = (Bun.nanoseconds() - start) / 1e6;
      samples.push(ms);
      const bucket = perQuery.get(query.name);
      if (bucket !== undefined) {
        bucket.samples.push(ms);
        bucket.edges = result.edges.length;
      }
    }

    const overall = percentiles(samples);
    const hotEdges = queries
      .filter((q) => q.role === "hot")
      .map((q) => perQuery.get(q.name)?.edges ?? 0);
    console.log(
      `references: ${samples.length} calls — p50 ${overall.p50.toFixed(2)}ms, ` +
        `p95 ${overall.p95.toFixed(2)}ms, max ${overall.max.toFixed(2)}ms ` +
        `(hot query: ${hotEdges[0] ?? 0} edges)`,
    );
    const queryRows = queries.map((q) => {
      const bucket = perQuery.get(q.name) ?? { samples: [], edges: 0 };
      const p = percentiles(bucket.samples);
      return {
        role: q.role,
        name: q.name,
        definitions: q.definitions,
        p50Ms: Number(p.p50.toFixed(3)),
        p95Ms: Number(p.p95.toFixed(3)),
        edges: bucket.edges,
      };
    });
    for (const row of queryRows) {
      console.log(
        `  ${row.role.padEnd(4)} ${row.name}: p50 ${row.p50Ms}ms p95 ${row.p95Ms}ms ` +
          `(${row.definitions} defs, ${row.edges} edges)`,
      );
    }
    Object.assign(measurements, {
      calls: samples.length,
      p50Ms: Number(overall.p50.toFixed(3)),
      p95Ms: Number(overall.p95.toFixed(3)),
      maxMs: Number(overall.max.toFixed(3)),
      queries: queryRows,
    });
    if (!hotEdges.some((edges) => edges > 0)) {
      fail("no hot query returned edges — the measurement is vacuous");
    }

    const gate = refsP95Gate(overall.p95, corpus.loc);
    gates.push(gate);
    if (reportOnly) {
      console.log("report-only: gate not applied");
      console.log(`  ${describeGate(gate)}`);
    } else if (!gate.pass) {
      fail(
        `references p95 ${overall.p95.toFixed(2)}ms exceeds the ${REFS_P95_GATE_MS}ms gate (Constitution VI)`,
      );
    } else {
      console.log(`OK: references p95 ${overall.p95.toFixed(2)}ms < ${REFS_P95_GATE_MS}ms`);
    }
  }
  if (failed) process.exitCode = 1;
} finally {
  await supervisor.stop();
  if (corpusKind === "root") {
    if (!args.keepIndex) removeIndexDir(root);
  } else {
    rmSync(root, { recursive: true, force: true, maxRetries: 10, retryDelay: 100 });
  }
  if (args.json !== undefined) {
    const report: BenchReport = {
      schema: BENCH_SCHEMA,
      script: "bench-refs",
      profile: profileName(args),
      recordedAt: new Date().toISOString(),
      ...(corpus !== undefined ? { corpus } : {}),
      machine: machineFacts(coreFlavor),
      measurements,
      gates,
    };
    writeReport(args.json, report);
    console.log(`report: ${args.json}`);
  }
}
