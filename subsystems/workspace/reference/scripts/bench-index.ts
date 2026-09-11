/**
 * The indexer perf gate (SDD §7.3.2, law #1), CI-enforced on lanes with a
 * built artifact:
 *
 *   1. Useful at Tier 0 within 5 seconds of first start: from engine-child
 *      spawn to a successful lexical answer, with a 500k-LOC Tier-1 build
 *      already in flight.
 *   2. 500k LOC reaches Tier 1 in under 60 seconds.
 *
 * The corpus is generated deterministically: bulk source across the fast
 * languages plus one marker file per bundled grammar, so the run also proves
 * all 13 language ids and the plain-text FTS5 lane are indexed (§7.3.4).
 *
 * `--root <path>` (specs/016) measures an existing tree instead: no corpus
 * is generated, the tree is never modified, the `.workbench/` the run
 * creates is removed at exit (unless `--keep-index`), and the tier-1 gate
 * is judged normalized per 500k LOC. LOC is counted from the index's own
 * files table after the build settles.
 *
 * Usage: bun run scripts/bench-index.ts [--report-only] [--stub] [--loc N]
 *          [--root <path>] [--probe <relpath>:<pattern>] [--json <file>]
 *          [--keep-index] [--deadline <s>]
 *   --stub uses the JS stub core (pipeline smoke, not a real measurement).
 */

import { Database } from "bun:sqlite";
import { mkdirSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";
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
  defaultProbe,
  describeGate,
  type Gate,
  gitFacts,
  guardIndexDir,
  indexDirFor,
  machineFacts,
  type Probe,
  parseBenchArgs,
  parseProbe,
  profileName,
  ROOT_DEADLINE_S,
  removeIndexDir,
  resolveRoot,
  TIER0_GATE_S,
  TIER1_GATE_S,
  tier0Gate,
  tier1Gate,
  writeReport,
} from "./bench-common";

/** One marker file per bundled grammar, each declaring bench_marker_<lang>. */
const LANGUAGE_SAMPLES: Readonly<Record<string, { file: string; content: string }>> = {
  bash: { file: "samples/marker.sh", content: "bench_marker_bash() {\n  echo hi\n}\n" },
  c: { file: "samples/marker.c", content: "int bench_marker_c(void) { return 1; }\n" },
  cpp: { file: "samples/marker.cpp", content: "int bench_marker_cpp() { return 1; }\n" },
  csharp: {
    file: "samples/marker.cs",
    content: "public class BenchMarkerCsharp { public int Run() { return 1; } }\n",
  },
  go: {
    file: "samples/marker.go",
    content: "package main\n\nfunc bench_marker_go() int { return 1 }\n",
  },
  java: {
    file: "samples/Marker.java",
    content: "public class BenchMarkerJava { int run() { return 1; } }\n",
  },
  javascript: { file: "samples/marker.js", content: "function bench_marker_javascript() {}\n" },
  json: { file: "samples/marker.json", content: '{ "bench_marker_json": true }\n' },
  python: { file: "samples/marker.py", content: "def bench_marker_python():\n    pass\n" },
  ruby: { file: "samples/marker.rb", content: "def bench_marker_ruby\nend\n" },
  rust: { file: "samples/marker.rs", content: "pub fn bench_marker_rust() -> u32 { 1 }\n" },
  typescript: {
    file: "samples/marker.ts",
    content: "export function bench_marker_typescript(): void {}\n",
  },
  tsx: {
    file: "samples/marker.tsx",
    content: "export function bench_marker_tsx() { return null; }\n",
  },
};

function tsModule(fileIndex: number, lines: number): string {
  const parts: string[] = [];
  let line = 0;
  let fn = 0;
  while (line < lines) {
    parts.push(
      `export function bulk_${fileIndex}_${fn}(value: number): number {`,
      `  const doubled = value * 2;`,
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
  const root = mkdtempSync(path.join(tmpdir(), "wb-bench-index-"));
  mkdirSync(path.join(root, "samples"), { recursive: true });
  mkdirSync(path.join(root, "bulk"), { recursive: true });

  let writtenLoc = 0;
  for (const sample of Object.values(LANGUAGE_SAMPLES)) {
    writeFileSync(path.join(root, sample.file), sample.content);
    writtenLoc += sample.content.split("\n").length - 1;
  }
  writeFileSync(root + "/README.md", "bench corpus fts_needle_marker documentation\n");
  writeFileSync(root + "/config.lock", "fts_needle_marker: locked\n");
  writtenLoc += 2;

  const linesPerFile = 250;
  const files = Math.ceil((loc - writtenLoc) / linesPerFile);
  for (let i = 0; i < files; i += 1) {
    writeFileSync(path.join(root, "bulk", `mod_${i}.ts`), tsModule(i, linesPerFile));
    writtenLoc += linesPerFile + 2;
  }
  return { root, writtenLoc };
}

let args: ReturnType<typeof parseBenchArgs>;
let root: string;
let probe: Probe;
let corpusKind: "synthetic" | "root";
try {
  args = parseBenchArgs(process.argv.slice(2));
  if (args.root !== undefined) {
    root = resolveRoot(args.root);
    guardIndexDir(root, args.keepIndex);
    probe = args.probe !== undefined ? parseProbe(args.probe) : defaultProbe(root);
    corpusKind = "root";
    console.log(`root: ${root} (probe ${probe.path} for "${probe.pattern}")`);
  } else {
    console.log(`generating ~${args.loc} LOC corpus…`);
    const genStart = Bun.nanoseconds();
    const corpus = makeCorpus(args.loc);
    root = corpus.root;
    console.log(
      `corpus ready: ${corpus.writtenLoc} LOC at ${root} in ${((Bun.nanoseconds() - genStart) / 1e9).toFixed(1)}s`,
    );
    probe = { path: "bulk/mod_0.ts", pattern: "bulk_0_1" };
    corpusKind = "synthetic";
  }
} catch (error) {
  if (error instanceof BenchUsageError) {
    console.error(`usage: ${error.message}`);
    process.exit(2);
  }
  throw error;
}

const deadlineS = args.deadlineS ?? (corpusKind === "root" ? ROOT_DEADLINE_S : TIER1_GATE_S + 30);
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
  // The clock starts at first start: spawn + core load + self-test all count.
  const startNs = Bun.nanoseconds();
  await supervisor.call("index/build", { root });

  // Tier-0 usefulness while the build runs: a real lexical answer, the same
  // per-file frame the search tool's cold path uses.
  const haystack = readFileSync(path.join(root, probe.path), "utf8");
  const search = (await supervisor.call("search", {
    pattern: probe.pattern,
    haystack,
    path: probe.path,
  })) as { hits: unknown[] };
  const tier0S = (Bun.nanoseconds() - startNs) / 1e9;
  if (search.hits.length === 0) fail("tier-0 probe returned no hits");
  console.log(`tier-0 lexical answer (spawn+selftest+build-kick+search): ${tier0S.toFixed(2)}s`);
  measurements["tier0S"] = Number(tier0S.toFixed(3));

  // Tier 1: poll until the build settles.
  let status: { tier: number; building: boolean; filesIndexed: number; filesTotal: number } = {
    tier: 0,
    building: true,
    filesIndexed: 0,
    filesTotal: 0,
  };
  const deadline = startNs + deadlineS * 1e9;
  while (Bun.nanoseconds() < deadline) {
    status = (await supervisor.call("index/status", {})) as typeof status;
    if (status.tier === 1 && !status.building) break;
    await new Promise((r) => setTimeout(r, 200));
  }
  const tier1S = (Bun.nanoseconds() - startNs) / 1e9;
  const tier1Reached = status.tier === 1 && !status.building;
  console.log(
    `tier-1: ${status.filesIndexed}/${status.filesTotal} files in ${tier1S.toFixed(2)}s ` +
      `(tier=${status.tier}, building=${status.building})`,
  );
  if (!tier1Reached) fail("build did not reach tier 1");
  Object.assign(measurements, {
    tier1S: Number(tier1S.toFixed(3)),
    tier1Reached,
    filesIndexed: status.filesIndexed,
    filesTotal: status.filesTotal,
  });

  if (corpusKind === "synthetic") {
    // Symbols are queryable through the child.
    const symbols = (await supervisor.call("index/symbols", { name: "bulk_0_1" })) as {
      symbols: readonly { name: string }[];
    };
    if (symbols.symbols.length === 0) fail("no symbols for a known bulk function");
  }

  // §7.3.4 coverage, read from the on-disk index: every bundled language id
  // is present in files, and plain text made it into FTS5. Under --root the
  // languages are reported, not gated, and LOC is counted from the same
  // table for both modes.
  const db = new Database(path.join(indexDirFor(root), "index.db"), { readonly: true });
  try {
    const languages = (
      db.query("SELECT DISTINCT language FROM files WHERE language IS NOT NULL").all() as {
        language: string;
      }[]
    ).map((row) => row.language);
    measurements["languagesIndexed"] = languages.length;
    if (corpusKind === "synthetic") {
      const missing = Object.keys(LANGUAGE_SAMPLES).filter((l) => !languages.includes(l));
      if (useStub) {
        console.log(`languages indexed (stub run, coverage not gated): ${languages.length}`);
      } else if (missing.length > 0) {
        fail(`languages missing from the index: ${missing.join(", ")}`);
      } else {
        console.log(`languages indexed: all ${languages.length} bundled ids`);
      }
      const fts = db
        .query("SELECT count(*) AS n FROM content_fts WHERE content_fts MATCH 'fts_needle_marker'")
        .get() as { n: number };
      if (fts.n < 2) fail(`plain-text FTS5 rows missing (matched ${fts.n}, expected >= 2)`);
      else console.log(`fts5: ${fts.n} plain-text matches for the needle (docs + lockfile)`);
    } else {
      console.log(`languages indexed: ${languages.length}`);
    }
    const loc = countLoc(db, root);
    const repo = corpusKind === "root" ? gitFacts(root) : undefined;
    corpus = {
      kind: corpusKind,
      ...(corpusKind === "root" ? { root } : {}),
      ...loc,
      ...(repo !== undefined ? { repo } : {}),
    };
    console.log(
      `corpus: ${loc.loc} source LOC over ${loc.sourceFiles} source files ` +
        `(${loc.files} files, ${loc.junkFiles} junk); top languages ` +
        loc.languages
          .slice(0, 5)
          .map((l) => `${l.language} ${l.loc}`)
          .join(", "),
    );
  } finally {
    db.close();
  }

  gates.push(tier0Gate(tier0S));
  gates.push(
    tier1Gate(tier1S, corpus.loc, tier1Reached, {
      filesIndexed: status.filesIndexed,
      filesTotal: status.filesTotal,
    }),
  );

  if (reportOnly) {
    console.log("report-only: gates not applied");
    for (const gate of gates) console.log(`  ${describeGate(gate)}`);
  } else {
    for (const gate of gates) {
      if (!gate.pass) {
        fail(
          `${gate.name} ${gate.value}${gate.unit} exceeds the ${gate.limit}${gate.unit} gate ` +
            `(SDD §7.3.2)${gate.note === undefined ? "" : ` — ${gate.note}`}`,
        );
      }
    }
    if (!failed) {
      const tier1 = gates.find((g) => g.name === "tier1");
      console.log(
        `OK: tier-0 ${tier0S.toFixed(2)}s < ${TIER0_GATE_S}s, ` +
          `tier-1 ${tier1S.toFixed(2)}s < ${TIER1_GATE_S}s` +
          (tier1 !== undefined && corpus.loc !== 0 && corpusKind === "root"
            ? ` (${tier1.value}s per 500k LOC)`
            : ""),
      );
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
      script: "bench-index",
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
