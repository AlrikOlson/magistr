/**
 * The `--root` mode end to end over a fixture with the stub core (specs/016
 * SC-001): both benches run as child processes exactly as a maintainer would
 * run them, answer the full report, write the JSON, leave the fixture
 * byte-identical, remove the `.workbench/` they created, and refuse a
 * pre-existing one.
 */

import { afterAll, beforeAll, describe, expect, test } from "bun:test";
import { createHash } from "node:crypto";
import {
  existsSync,
  mkdirSync,
  mkdtempSync,
  readdirSync,
  readFileSync,
  rmSync,
  statSync,
  writeFileSync,
} from "node:fs";
import { tmpdir } from "node:os";
import path from "node:path";
import type { BenchReport, CorpusFacts } from "../scripts/bench-common";

const SCRIPTS = path.resolve(import.meta.dirname, "..", "scripts");
let root = "";

function fixture(): string {
  const dir = mkdtempSync(path.join(tmpdir(), "wb-bench-root-"));
  mkdirSync(path.join(dir, "src"));
  mkdirSync(path.join(dir, "target", "debug"), { recursive: true });
  writeFileSync(path.join(dir, "Cargo.toml"), '[package]\nname = "fixture"\n');
  // TypeScript sources: the stub core extracts symbols for TypeScript only
  // (its `languages()` answers ["typescript"]); a Cargo.toml still drives the
  // default probe, and a Rust file under target/ proves the junk flag.
  writeFileSync(
    path.join(dir, "src", "lib.ts"),
    "export function alpha(): number {\n  return beta() + 1;\n}\n\nfunction beta(): number {\n  return 1;\n}\n\nexport function gamma(): void {}\n",
  );
  writeFileSync(
    path.join(dir, "src", "main.ts"),
    'import { alpha } from "./lib";\n\nfunction main(): void {\n  console.log(alpha());\n}\n\nfunction alpha(): void {}\n',
  );
  writeFileSync(path.join(dir, ".gitignore"), "ignored.ts\n");
  writeFileSync(path.join(dir, "ignored.ts"), "function hiddenFromTheWalk() {}\n");
  writeFileSync(path.join(dir, "target", "debug", "junk.rs"), "fn generated() {}\n");
  return dir;
}

/** Every file under the fixture (minus .workbench) with its content hash. */
function snapshot(dir: string, rel = ""): Record<string, string> {
  const out: Record<string, string> = {};
  for (const name of readdirSync(path.join(dir, rel)).sort()) {
    const relPath = rel === "" ? name : `${rel}/${name}`;
    if (relPath === ".workbench") continue;
    const abs = path.join(dir, relPath);
    if (statSync(abs).isDirectory()) Object.assign(out, snapshot(dir, relPath));
    else out[relPath] = createHash("sha256").update(readFileSync(abs)).digest("hex");
  }
  return out;
}

function runBench(
  script: "bench-index.ts" | "bench-refs.ts",
  args: readonly string[],
): { exitCode: number; stdout: string; stderr: string } {
  const proc = Bun.spawnSync(
    [process.execPath, path.join(SCRIPTS, script), "--stub", "--report-only", ...args],
    { stdout: "pipe", stderr: "pipe", env: { ...process.env, WORKBENCH_ENGINE_STUB_CORE: "" } },
  );
  return {
    exitCode: proc.exitCode,
    stdout: proc.stdout.toString(),
    stderr: proc.stderr.toString(),
  };
}

beforeAll(() => {
  root = fixture();
});
afterAll(() => {
  rmSync(root, { recursive: true, force: true, maxRetries: 10, retryDelay: 100 });
});

describe("bench-index.ts --root", () => {
  test("indexes the fixture, reports, writes the JSON, and leaves the tree as it found it", () => {
    const before = snapshot(root);
    const out = path.join(root, "..", `${path.basename(root)}-index.json`);
    const run = runBench("bench-index.ts", ["--root", root, "--json", out]);
    expect(run.stderr).toBe("");
    expect(run.exitCode).toBe(0);
    expect(run.stdout).toContain(`root: ${path.resolve(root)} (probe Cargo.toml for "package")`);
    expect(run.stdout).toContain("tier-0 lexical answer");
    expect(run.stdout).toContain("tier=1, building=false");
    expect(run.stdout).toContain("report-only: gates not applied");

    const report = JSON.parse(readFileSync(out, "utf8")) as BenchReport;
    rmSync(out, { force: true });
    expect(report.schema).toBe("workbench-bench/1");
    expect(report.script).toBe("bench-index");
    expect(report.profile).toBe("root");
    expect(report.machine.coreFlavor).toBe("stub");
    const corpus = report.corpus as CorpusFacts;
    expect(corpus.kind).toBe("root");
    expect(corpus.root).toBe(path.resolve(root));
    // lib.ts (9 lines) + main.ts (7 lines); target/debug/junk.rs is junk; ignored.ts is not walked.
    expect(corpus.loc).toBe(16);
    expect(corpus.sourceFiles).toBe(2);
    expect(corpus.junkFiles).toBe(1);
    expect(corpus.languages).toEqual([{ language: "typescript", files: 2, loc: 16 }]);
    expect(corpus.repo).toBeUndefined();
    expect(report.measurements["tier1Reached"]).toBe(true);
    expect(report.gates.map((g) => [g.name, typeof g.pass])).toEqual([
      ["tier0", "boolean"],
      ["tier1", "boolean"],
    ]);
    expect(report.gates[1]?.note).toContain("normalized per 500000 LOC");

    expect(existsSync(path.join(root, ".workbench"))).toBe(false);
    expect(snapshot(root)).toEqual(before);
  });

  test("--probe overrides the default and --keep-index keeps the index", () => {
    const run = runBench("bench-index.ts", [
      "--root",
      root,
      "--probe",
      "src/lib.ts:gamma",
      "--keep-index",
    ]);
    expect(run.exitCode).toBe(0);
    expect(run.stdout).toContain('probe src/lib.ts for "gamma"');
    expect(existsSync(path.join(root, ".workbench", "index.db"))).toBe(true);
  });

  test("a pre-existing .workbench is refused before anything runs", () => {
    expect(existsSync(path.join(root, ".workbench"))).toBe(true);
    const run = runBench("bench-index.ts", ["--root", root]);
    expect(run.exitCode).toBe(2);
    expect(run.stderr).toContain("warm index is not a cold build");
    expect(run.stdout).not.toContain("tier-0");
    expect(existsSync(path.join(root, ".workbench", "index.db"))).toBe(true);
    rmSync(path.join(root, ".workbench"), { recursive: true, force: true });
  });

  test("a missing root or a bad probe is a usage error", () => {
    expect(runBench("bench-index.ts", ["--root", path.join(root, "nope")]).exitCode).toBe(2);
    expect(runBench("bench-index.ts", ["--root", root, "--probe", "nocolon"]).exitCode).toBe(2);
    expect(existsSync(path.join(root, ".workbench"))).toBe(false);
  });
});

describe("bench-refs.ts --root", () => {
  test("samples hot / rare / miss from the fixture's symbols and reports per query", () => {
    const before = snapshot(root);
    const out = path.join(root, "..", `${path.basename(root)}-refs.json`);
    const run = runBench("bench-refs.ts", ["--root", root, "--json", out]);
    const report = JSON.parse(readFileSync(out, "utf8")) as BenchReport;
    rmSync(out, { force: true });

    expect(report.script).toBe("bench-refs");
    expect(report.profile).toBe("root");
    expect((report.corpus as CorpusFacts).loc).toBe(16);
    const queries = report.measurements["queries"] as readonly {
      role: string;
      name: string;
      definitions: number;
      p50Ms: number;
      p95Ms: number;
      edges: number;
    }[];
    // alpha is defined in both files (hot, 2 defs); beta / gamma / main once each.
    expect(queries[0]).toMatchObject({ role: "hot", name: "alpha", definitions: 2 });
    expect(queries.filter((q) => q.role === "hot")).toHaveLength(3);
    expect(queries.filter((q) => q.role === "rare").length).toBeGreaterThan(0);
    expect(queries[queries.length - 1]).toMatchObject({
      role: "miss",
      name: "bench_refs_no_such_symbol",
      definitions: 0,
      edges: 0,
    });
    for (const q of queries) {
      expect(q.p95Ms).toBeGreaterThanOrEqual(q.p50Ms);
      expect(run.stdout).toContain(`${q.name}: p50`);
    }
    expect(report.measurements["calls"]).toBe(queries.length * 40);
    expect(report.gates.map((g) => g.name)).toEqual(["references_p95"]);

    // The vacuity rule decides the exit code; either way the report was written.
    const hotEdges = queries.filter((q) => q.role === "hot").some((q) => q.edges > 0);
    expect(run.exitCode).toBe(hotEdges ? 0 : 1);
    if (!hotEdges) expect(run.stderr).toContain("vacuous");

    expect(existsSync(path.join(root, ".workbench"))).toBe(false);
    expect(snapshot(root)).toEqual(before);
  });

  test("refuses a pre-existing .workbench too", () => {
    mkdirSync(path.join(root, ".workbench"));
    const run = runBench("bench-refs.ts", ["--root", root]);
    expect(run.exitCode).toBe(2);
    expect(run.stderr).toContain("warm index");
    rmSync(path.join(root, ".workbench"), { recursive: true, force: true });
  });
});
