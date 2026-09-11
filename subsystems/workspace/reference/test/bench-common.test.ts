/**
 * The bench helpers (specs/016 SC-002): argument parsing, the probe default,
 * LOC over a files table, hot / rare / miss sampling over a symbols table,
 * the gates judged as the constitution states them, the report schema. No
 * core, no engine child — in-memory SQLite and temp dirs only.
 */

import { Database } from "bun:sqlite";
import { afterEach, describe, expect, test } from "bun:test";
import { existsSync, mkdirSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import path from "node:path";
import {
  BENCH_SCHEMA,
  BenchUsageError,
  countLoc,
  countNewlines,
  defaultProbe,
  describeGate,
  guardIndexDir,
  IPC_P95_GATE_MS,
  ipcP95Gate,
  MISS_NAME,
  machineFacts,
  parseBenchArgs,
  parseProbe,
  percentiles,
  profileName,
  REFS_P95_GATE_MS,
  refsP95Gate,
  resolveRoot,
  sampleReferenceQueries,
  TIER0_GATE_S,
  TIER1_GATE_S,
  tier0Gate,
  tier1Gate,
  writeReport,
} from "../scripts/bench-common";

const roots: string[] = [];
function tempRoot(): string {
  const root = mkdtempSync(path.join(tmpdir(), "wb-bench-common-"));
  roots.push(root);
  return root;
}
afterEach(() => {
  for (const root of roots.splice(0)) {
    rmSync(root, { recursive: true, force: true, maxRetries: 5, retryDelay: 50 });
  }
});

describe("parseBenchArgs", () => {
  test("defaults match the synthetic CI profile", () => {
    const args = parseBenchArgs([]);
    expect(args).toEqual({ keepIndex: false, loc: 500_000, stub: false, reportOnly: false });
  });

  test("reads every flag", () => {
    const args = parseBenchArgs([
      "--root",
      "D:/repo",
      "--probe",
      "Cargo.toml:package",
      "--json",
      "out.json",
      "--keep-index",
      "--deadline",
      "30",
      "--loc",
      "1000000",
      "--stub",
      "--report-only",
    ]);
    expect(args).toEqual({
      root: "D:/repo",
      probe: "Cargo.toml:package",
      json: "out.json",
      keepIndex: true,
      deadlineS: 30,
      loc: 1_000_000,
      stub: true,
      reportOnly: true,
    });
  });

  test("a flag without its value, or a non-positive number, is a usage error", () => {
    expect(() => parseBenchArgs(["--root"])).toThrow(BenchUsageError);
    expect(() => parseBenchArgs(["--root", "--stub"])).toThrow(BenchUsageError);
    expect(() => parseBenchArgs(["--deadline", "0"])).toThrow(BenchUsageError);
    expect(() => parseBenchArgs(["--loc", "many"])).toThrow(BenchUsageError);
  });

  test("profile names follow the LOC target; --root is 'root'", () => {
    expect(profileName({ loc: 500_000 })).toBe("synthetic-500k");
    expect(profileName({ loc: 1_000_000 })).toBe("synthetic-1m");
    expect(profileName({ loc: 1234 })).toBe("synthetic-1234");
    expect(profileName({ root: "x", loc: 500_000 })).toBe("root");
  });
});

describe("--root guard rails", () => {
  test("the root must be an existing directory", () => {
    const root = tempRoot();
    expect(resolveRoot(root)).toBe(path.resolve(root));
    expect(() => resolveRoot(path.join(root, "missing"))).toThrow(BenchUsageError);
    writeFileSync(path.join(root, "file.txt"), "x\n");
    expect(() => resolveRoot(path.join(root, "file.txt"))).toThrow(BenchUsageError);
  });

  test("a pre-existing .workbench is refused unless --keep-index", () => {
    const root = tempRoot();
    expect(() => guardIndexDir(root, false)).not.toThrow();
    mkdirSync(path.join(root, ".workbench"));
    expect(() => guardIndexDir(root, false)).toThrow(/warm index/);
    expect(() => guardIndexDir(root, true)).not.toThrow();
    expect(existsSync(path.join(root, ".workbench"))).toBe(true);
  });
});

describe("the tier-0 probe", () => {
  test("--probe splits at the last colon and forward-slashes the path", () => {
    expect(parseProbe("src/lib.rs:pub fn")).toEqual({ path: "src/lib.rs", pattern: "pub fn" });
    expect(parseProbe("src\\a:b.rs:token")).toEqual({ path: "src/a:b.rs", pattern: "token" });
    expect(() => parseProbe("nocolon")).toThrow(BenchUsageError);
    expect(() => parseProbe("path:")).toThrow(BenchUsageError);
    expect(() => parseProbe(":pattern")).toThrow(BenchUsageError);
  });

  test("the default prefers Cargo.toml, then README.md, then package.json", () => {
    const root = tempRoot();
    expect(() => defaultProbe(root)).toThrow(/--probe/);
    writeFileSync(path.join(root, "package.json"), '{\n  "name": "fixture"\n}\n');
    expect(defaultProbe(root)).toEqual({ path: "package.json", pattern: "name" });
    writeFileSync(path.join(root, "README.md"), "\n\n# Fixture repo\n");
    expect(defaultProbe(root)).toEqual({ path: "README.md", pattern: "Fixture" });
    writeFileSync(path.join(root, "Cargo.toml"), "[workspace]\nmembers = []\n");
    expect(defaultProbe(root)).toEqual({ path: "Cargo.toml", pattern: "workspace" });
  });

  test("a candidate without an identifier of three characters is skipped", () => {
    const root = tempRoot();
    writeFileSync(path.join(root, "Cargo.toml"), "# a b\n");
    writeFileSync(path.join(root, "README.md"), "Hello\n");
    expect(defaultProbe(root)).toEqual({ path: "README.md", pattern: "Hello" });
  });
});

function filesTable(): Database {
  const db = new Database(":memory:");
  db.run(
    "CREATE TABLE files (path TEXT PRIMARY KEY, language TEXT, size INTEGER NOT NULL, " +
      "mtime_ms INTEGER NOT NULL, junk INTEGER NOT NULL, content_indexed INTEGER NOT NULL)",
  );
  return db;
}

describe("countLoc", () => {
  test("counts newlines of source rows only, per language, from the files table", () => {
    const root = tempRoot();
    mkdirSync(path.join(root, "src"));
    mkdirSync(path.join(root, "target"));
    writeFileSync(path.join(root, "src", "a.ts"), "1\n2\n3\n");
    writeFileSync(path.join(root, "src", "b.rs"), "fn a() {}\nfn b() {}\n");
    writeFileSync(path.join(root, "src", "c.rs"), "no trailing newline");
    writeFileSync(path.join(root, "Cargo.lock"), "[[package]]\n");
    writeFileSync(path.join(root, "target", "gen.rs"), "junk\njunk\n");
    writeFileSync(path.join(root, "notes.txt"), "plain\ntext\n");
    const db = filesTable();
    const insert = db.query(
      "INSERT INTO files (path, language, size, mtime_ms, junk, content_indexed) VALUES (?, ?, 0, 0, ?, 1)",
    );
    insert.run("src/a.ts", "typescript", 0);
    insert.run("src/b.rs", "rust", 0);
    insert.run("src/c.rs", "rust", 0);
    insert.run("Cargo.lock", null, 1);
    insert.run("target/gen.rs", "rust", 1);
    insert.run("notes.txt", null, 0);
    insert.run("vanished.rs", "rust", 0);

    const loc = countLoc(db, root);
    expect(loc).toEqual({
      loc: 5,
      files: 7,
      sourceFiles: 3,
      junkFiles: 2,
      languages: [
        { language: "typescript", files: 1, loc: 3 },
        { language: "rust", files: 2, loc: 2 },
      ],
    });
    db.close();
  });

  test("countNewlines streams past the buffer size", () => {
    const root = tempRoot();
    const big = path.join(root, "big.txt");
    writeFileSync(big, "x".repeat(1_500_000).replace(/x{1000}/g, "$&\n"));
    expect(countNewlines(big)).toBe(1500);
  });
});

describe("sampleReferenceQueries", () => {
  function symbolsTable(rows: readonly [string, number][]): Database {
    const db = new Database(":memory:");
    db.run(
      "CREATE TABLE symbols (path TEXT NOT NULL, name TEXT NOT NULL, kind TEXT NOT NULL, " +
        "start_line INTEGER NOT NULL, start_col INTEGER NOT NULL, end_line INTEGER NOT NULL, " +
        "detail TEXT, doc TEXT)",
    );
    const insert = db.query(
      "INSERT INTO symbols (path, name, kind, start_line, start_col, end_line) VALUES (?, ?, 'function', 1, 0, 1)",
    );
    for (const [name, count] of rows) {
      for (let i = 0; i < count; i += 1) insert.run(`src/${name}_${i}.rs`, name);
    }
    return db;
  }

  test("hot by count (names of >= 3 chars), rare at the quartiles of the singles, one miss", () => {
    const db = symbolsTable([
      ["ab", 10],
      ["new", 5],
      ["fmt", 3],
      ["from", 2],
      ["into", 2],
      ...["s1", "s2", "s3", "s4", "s5", "s6", "s7", "s8"].map((n): [string, number] => [n, 1]),
    ]);
    const queries = sampleReferenceQueries(db);
    expect(queries.map((q) => q.role)).toEqual([
      "hot",
      "hot",
      "hot",
      "rare",
      "rare",
      "rare",
      "miss",
    ]);
    expect(queries.slice(0, 3).map((q) => [q.name, q.definitions])).toEqual([
      ["new", 5],
      ["fmt", 3],
      ["from", 2],
    ]);
    // 8 singles sorted: s1..s8; quartiles at indices 2, 4, 6.
    expect(queries.slice(3, 6).map((q) => q.name)).toEqual(["s3", "s5", "s7"]);
    expect(queries[6]).toEqual({ role: "miss", name: MISS_NAME, definitions: 0 });
    db.close();
  });

  test("is deterministic and copes with a tiny table", () => {
    const db = symbolsTable([["only", 1]]);
    const first = sampleReferenceQueries(db);
    expect(first).toEqual(sampleReferenceQueries(db));
    expect(first.map((q) => q.role)).toEqual(["hot", "rare", "miss"]);
    expect(first[0]?.name).toBe("only");
    db.close();
  });

  test("an empty table samples only the miss", () => {
    const db = symbolsTable([]);
    expect(sampleReferenceQueries(db)).toEqual([{ role: "miss", name: MISS_NAME, definitions: 0 }]);
    db.close();
  });
});

describe("gates as the constitution states them", () => {
  test("tier-0 is judged as written", () => {
    expect(tier0Gate(0.12)).toEqual({
      name: "tier0",
      value: 0.12,
      limit: TIER0_GATE_S,
      unit: "s",
      pass: true,
    });
    expect(tier0Gate(5.001).pass).toBe(false);
  });

  test("tier-1 is normalized per 500k LOC and says so", () => {
    const at500k = tier1Gate(10.09, 504_021, true);
    expect(at500k.value).toBeCloseTo(10.01, 2);
    expect(at500k.pass).toBe(true);
    expect(at500k.note).toContain("normalized per 500000 LOC");
    expect(at500k.note).toContain("10.09s over 504021 LOC");
    const at4m = tier1Gate(120, 4_000_000, true);
    expect(at4m).toMatchObject({ value: 15, limit: TIER1_GATE_S, unit: "s", pass: true });
    const slow = tier1Gate(600, 4_000_000, true);
    expect(slow).toMatchObject({ value: 75, pass: false });
    const empty = tier1Gate(3, 0, true);
    expect(empty).toMatchObject({ value: 3, pass: true });
  });

  test("tier-1 not reached fails whatever the clock says", () => {
    const gate = tier1Gate(900, 4_000_000, false, { filesIndexed: 100, filesTotal: 62_221 });
    expect(gate.pass).toBe(false);
    expect(gate.value).toBe(900);
    expect(gate.note).toBe("tier 1 not reached within 900s (100/62221 files)");
  });

  test("references p95 and IPC p95 are judged as written", () => {
    expect(refsP95Gate(4.58, 504_021)).toMatchObject({
      name: "references_p95",
      value: 4.58,
      limit: REFS_P95_GATE_MS,
      unit: "ms",
      pass: true,
    });
    expect(refsP95Gate(50.001, 4_000_000).pass).toBe(false);
    expect(ipcP95Gate(0.4)).toEqual({
      name: "ipc_p95",
      value: 0.4,
      limit: IPC_P95_GATE_MS,
      unit: "ms",
      pass: true,
    });
    expect(ipcP95Gate(1.2).pass).toBe(false);
  });

  test("pass is always value <= limit; the console line names the verdict", () => {
    for (const gate of [tier0Gate(7), tier1Gate(70, 500_000, true), refsP95Gate(1, 1)]) {
      expect(gate.pass).toBe(gate.value <= gate.limit);
      expect(describeGate(gate)).toMatch(/^(OK|FAIL): /);
    }
  });
});

describe("percentiles and the report", () => {
  test("percentiles sort a copy and answer zeros for an empty list", () => {
    expect(percentiles([5, 1, 3, 2, 4])).toEqual({ p50: 3, p95: 5, p99: 5, max: 5 });
    expect(percentiles([])).toEqual({ p50: 0, p95: 0, p99: 0, max: 0 });
  });

  test("writeReport creates the directory and writes the schema", () => {
    const root = tempRoot();
    const file = path.join(root, "results", "nested", "ipc.json");
    const machine = machineFacts("stub");
    expect(machine.coreFlavor).toBe("stub");
    expect(machine.cores).toBeGreaterThan(0);
    expect(machine.bun).toBe(Bun.version);
    writeReport(file, {
      schema: BENCH_SCHEMA,
      script: "bench-ipc",
      profile: "ipc",
      recordedAt: "2026-08-24T00:00:00.000Z",
      machine,
      measurements: { pings: 1, p95Ms: 0.4 },
      gates: [ipcP95Gate(0.4)],
    });
    const parsed = JSON.parse(readFileSync(file, "utf8")) as { schema: string; gates: unknown[] };
    expect(parsed.schema).toBe("workbench-bench/1");
    expect(parsed.gates).toHaveLength(1);
    expect(readFileSync(file, "utf8").endsWith("\n")).toBe(true);
  });
});
