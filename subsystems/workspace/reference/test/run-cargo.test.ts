/**
 * The cargo family of `run` parsers (chunk:rust-cargo-runner-depth,
 * specs/013). Every input under test/support/golden/cargo/ was CAPTURED from
 * a real run (rustc/cargo 1.95.0, cargo-nextest 0.9.120, Windows — hence the
 * backslash paths the parsers normalize) over a three-crate scratch
 * workspace; the expected rows are checked-in goldens (`UPDATE_GOLDEN=1`
 * regenerates) AND explicit assertions on the facts the spec claims, so a
 * regenerated golden cannot quietly bless a regression.
 *
 * The live chapter builds a fixture workspace and runs the real toolchain
 * through the same parsers — the goldens' staleness guard (SC-006).
 */

import { afterAll, describe, expect, test } from "bun:test";
import { existsSync, mkdirSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import path from "node:path";
import type { ResponseEnvelope } from "../src/envelope";
import {
  BUNDLED_RUNNER_ADAPTERS,
  type RunDiagnosticsRow,
  type RunExecution,
  type RunOutputRow,
  type RunReportRow,
  type RunRow,
  runRun,
} from "../src/tools/run";
import {
  cargoSubcommand,
  extractCargoDiagnostics,
  parsePanicBlock,
  trimBacktrace,
} from "../src/tools/run-cargo";

const GOLDEN_DIR = path.join(import.meta.dir, "support", "golden", "cargo");
const UPDATE = process.env.UPDATE_GOLDEN === "1";

function golden(name: string, actual: unknown): void {
  const file = path.join(GOLDEN_DIR, `${name}.expected.json`);
  const plain = JSON.parse(JSON.stringify(actual));
  if (UPDATE) {
    writeFileSync(file, `${JSON.stringify(plain, null, 2)}\n`);
    return;
  }
  expect(plain).toEqual(JSON.parse(readFileSync(file, "utf8")));
}

/** A captured input: `<name>.txt` (stdout, or the 2>&1 capture) plus an
 * optional `<name>.stderr.txt`. */
function captured(name: string): { readonly stdout: string; readonly stderr: string } {
  const stdout = readFileSync(path.join(GOLDEN_DIR, `${name}.txt`), "utf8");
  const stderrPath = path.join(GOLDEN_DIR, `${name}.stderr.txt`);
  const stderr = existsSync(stderrPath) ? readFileSync(stderrPath, "utf8") : "";
  return { stdout, stderr };
}

async function runWith(
  line: string,
  execution: Partial<RunExecution>,
  input: { budget?: number } = {},
): Promise<ResponseEnvelope<RunRow>> {
  const exec: RunExecution = {
    exitCode: 0,
    stdout: "",
    stderr: "",
    timedOut: false,
    durationMs: 7,
    ...execution,
  };
  return runRun(
    { commands: { c: line }, execute: () => Promise.resolve(exec) },
    { command: "c", ...(input.budget === undefined ? {} : { budget: input.budget }) },
    1,
  );
}

function row(envelope: ResponseEnvelope<RunRow>): RunRow {
  if (!envelope.ok) throw new Error(`expected ok, got ${JSON.stringify(envelope.error)}`);
  const first = envelope.results[0];
  if (first === undefined) throw new Error("empty results");
  return first;
}

async function report(name: string, line: string, exitCode: number): Promise<RunReportRow> {
  const r = row(await runWith(line, { ...captured(name), exitCode }));
  if (r.kind !== "report") throw new Error(`expected report, got ${r.kind}`);
  return r;
}

async function diagnostics(
  name: string,
  line: string,
  exitCode: number,
): Promise<RunDiagnosticsRow> {
  const r = row(await runWith(line, { ...captured(name), exitCode }));
  if (r.kind !== "diagnostics") throw new Error(`expected diagnostics, got ${r.kind}`);
  return r;
}

const byName = (r: RunReportRow, testName: string) => {
  const f = r.failures.find((x) => x.testName === testName);
  if (f === undefined) throw new Error(`no failure named ${testName}`);
  return f;
};

// ---------------------------------------------------------------------------
// cargo test (US1 / SC-001)

describe("cargo test failures (US1)", () => {
  test("one binary: plain panic messages, forward-slash files, the location as the first trace line (golden)", async () => {
    const r = await report("cargo-test-single", "cargo test", 101);
    expect(r.adapter).toBe("cargo");
    expect(r.failures.map((f) => f.testName).sort()).toEqual([
      "tests::fails_assert",
      "tests::fails_eq",
      "tests::fails_panic",
    ]);
    // The message on the line AFTER `panicked at` — rustc ≥ 1.73.
    const plain = byName(r, "tests::fails_panic");
    expect(plain.assertion).toBe("boom 42");
    expect(plain.file).toBe("crates/alpha/src/lib.rs");
    expect(plain.trace.split("\n")[0]).toBe("crates/alpha/src/lib.rs:11:24");
    const eq = byName(r, "tests::fails_eq");
    expect(eq.assertion).toBe("assertion `left == right` failed: math is hard");
    expect(eq.trace).toContain("left: 3");
    expect(eq.trace).toContain("right: 4");
    expect(eq.trace).not.toContain("RUST_BACKTRACE");
    expect(byName(r, "tests::fails_assert").assertion).toBe("assertion failed: add(1, 1) == 3");
    // One binary: the summary is libtest's own line, verbatim.
    expect(r.summary).toBe(
      "test result: FAILED. 1 passed; 3 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s",
    );
    golden("cargo-test-single", r);
  });

  test("a --no-fail-fast workspace run: every binary's failures, attributed, one aggregated summary (golden)", async () => {
    const r = await report("cargo-test-workspace", "cargo test --workspace --no-fail-fast", 101);
    expect(r.failures).toHaveLength(4);
    expect(r.failures.map((f) => f.testName)).toEqual([
      "alpha tests::fails_eq",
      "alpha tests::fails_assert",
      "alpha tests::fails_panic",
      "integration integration_fails",
    ]);
    const oob = byName(r, "integration integration_fails");
    expect(oob.file).toBe("crates/beta/tests/integration.rs");
    expect(oob.assertion).toBe("index out of bounds: the len is 0 but the index is 0");
    expect(r.summary).toBe(
      "2 passed; 4 failed; 0 ignored across 5 binaries — rerun: -p alpha --lib, -p beta --test integration",
    );
    golden("cargo-test-workspace", r);
  });

  test("RUST_BACKTRACE=1: the trace keeps the workspace's frames and drops std/toolchain frames (golden)", async () => {
    const r = await report(
      "cargo-test-backtrace",
      "cargo test -p alpha --lib tests::fails_panic",
      101,
    );
    expect(r.failures).toHaveLength(1);
    const lines = (r.failures[0] as { trace: string }).trace.split("\n");
    expect(lines[0]).toBe("crates/alpha/src/lib.rs:11:24");
    expect(lines.some((l) => l.startsWith("alpha::tests::fails_panic at ./src/lib.rs:11"))).toBe(
      true,
    );
    expect(lines.some((l) => l.includes("/rustc/"))).toBe(false);
    expect(lines.some((l) => l.includes("panicking.rs"))).toBe(false);
    expect(lines.some((l) => l.includes(".rustup"))).toBe(false);
    expect(lines.length).toBeLessThanOrEqual(8);
    golden("cargo-test-backtrace", r);
  });

  test("the pre-1.73 panic line shape is still read", () => {
    const d = parsePanicBlock([
      "thread 'tests::old' panicked at 'boom', src/lib.rs:3:5",
      "note: run with `RUST_BACKTRACE=1` environment variable to display a backtrace",
    ]);
    expect(d.assertion).toBe("boom");
    expect(d.file).toBe("src/lib.rs");
    expect(d.trace).toBe("src/lib.rs:3:5");
  });

  test("a block without a panic line keeps the pre-013 shape: empty assertion, trimmed block", () => {
    const d = parsePanicBlock(["", "note: test did not panic as expected", ""]);
    expect(d.assertion).toBe("");
    expect(d.file).toBeUndefined();
    expect(d.trace).toBe("note: test did not panic as expected");
  });

  test("trimBacktrace pairs symbol and location lines and keeps only workspace paths", () => {
    expect(
      trimBacktrace([
        "   0: std::panicking::panic_handler",
        "             at /rustc/abc/library\\std\\src\\panicking.rs:689",
        "   2: alpha::tests::fails_panic",
        "             at .\\src\\lib.rs:11",
        "   4: core::ops::function::FnOnce::call_once",
        "             at C:\\Users\\dev\\.rustup\\toolchains\\stable\\lib\\rustlib\\src\\rust\\library\\core\\src\\ops\\function.rs:250",
        "   5: mycrate::helper",
        "             at C:\\Users\\dev\\.cargo\\registry\\src\\index\\dep-1.0.0\\src\\lib.rs:9",
      ]),
    ).toEqual(["alpha::tests::fails_panic at ./src/lib.rs:11"]);
  });
});

// ---------------------------------------------------------------------------
// cargo nextest (US1 / SC-001)

describe("cargo nextest failures (US1)", () => {
  test("nextest's own format: FAIL lines across binaries, panic from the stderr block, no repeat-list duplicates (golden)", async () => {
    const r = await report("nextest-workspace", "cargo nextest run --workspace", 100);
    expect(r.adapter).toBe("nextest");
    // Emission order — nextest schedules binaries in parallel, so this is
    // the captured run's order, not a stable sort.
    expect(r.failures.map((f) => f.testName)).toEqual([
      "alpha tests::fails_eq",
      "beta::integration integration_fails",
      "alpha tests::fails_panic",
      "alpha tests::fails_assert",
    ]);
    const oob = byName(r, "beta::integration integration_fails");
    expect(oob.file).toBe("crates/beta/tests/integration.rs");
    expect(oob.assertion).toBe("index out of bounds: the len is 0 but the index is 0");
    expect(oob.trace.split("\n")[0]).toBe("crates/beta/tests/integration.rs:2:61");
    expect(byName(r, "alpha tests::fails_panic").assertion).toBe("boom 42");
    expect(r.summary).toBe(
      "Summary [ 0.024s] 6 tests run: 2 passed, 4 failed, 0 skipped (cancelled: 5 still running)",
    );
    golden("nextest-workspace", r);
  });

  test("statuses without a panic message name the status; retries and flaky passes are not failures", async () => {
    const stderr = [
      "    Starting 3 tests across 1 binary",
      "        TRY 1 FAIL [   0.010s] (1/3) alpha tests::flaky",
      "      FLAKY 2/3 [   0.011s] (1/3) alpha tests::flaky",
      "     TIMEOUT [  60.000s] (2/3) alpha tests::hangs",
      "  stdout ───",
      "",
      "    running 1 test",
      "",
      "        PASS [   0.001s] (3/3) alpha tests::ok",
      "     Summary [  60.020s] 3 tests run: 2 passed (1 flaky), 1 failed, 0 skipped",
      "     TIMEOUT [  60.000s] (2/3) alpha tests::hangs",
    ].join("\n");
    const r = row(await runWith("cargo nextest run", { exitCode: 100, stderr })) as RunReportRow;
    expect(r.kind).toBe("report");
    expect(r.failures).toHaveLength(1);
    expect(r.failures[0]?.testName).toBe("alpha tests::hangs");
    expect(r.failures[0]?.assertion).toBe("TIMEOUT");
    expect(r.summary).toBe(
      "Summary [ 60.020s] 3 tests run: 2 passed (1 flaky), 1 failed, 0 skipped",
    );
  });

  test("nextest is claimed by its own adapter, no longer by cargo's", () => {
    const claims = BUNDLED_RUNNER_ADAPTERS.filter((a) =>
      a.canParse("cargo nextest run --workspace"),
    ).map((a) => a.name);
    expect(claims).toEqual(["nextest"]);
    expect(BUNDLED_RUNNER_ADAPTERS.map((a) => a.name).indexOf("nextest")).toBeLessThan(
      BUNDLED_RUNNER_ADAPTERS.map((a) => a.name).indexOf("cargo"),
    );
  });
});

// ---------------------------------------------------------------------------
// Diagnostics rows (US2 / SC-002, SC-003)

describe("cargo build / clippy diagnostics (US2)", () => {
  test("rustc JSON: errors first, codes, primary spans, help folded, cargo's own lines ignored (golden)", async () => {
    const r = await diagnostics(
      "build-json-warn-error",
      "cargo build --workspace --message-format=json",
      101,
    );
    expect(r.extractor).toBe("cargo");
    expect(r.diagnostics.map((d) => d.level)).toEqual(["error", "error", "warning"]);
    const first = r.diagnostics[0];
    expect(first?.code).toBe("E0308");
    expect(first?.message).toBe("mismatched types");
    expect(first?.file).toBe("crates/gamma/src/lib.rs");
    expect(first?.line).toBe(1);
    expect(first?.column).toBe(42);
    expect(first?.detail).toContain("help: try using a conversion method");
    expect(first?.detail.split("\n")[0]).not.toContain("-->");
    const warning = r.diagnostics[2];
    expect(warning?.code).toBe("unused_variables");
    expect(warning?.message).toBe("unused variable: `y`");
    expect(warning?.line).toBe(2);
    expect(r.summary).toBe("2 errors, 1 warning");
    golden("build-json-warn-error", r);
  });

  test("the rendered form of the same build yields EQUAL diagnostics (golden)", async () => {
    const json = await diagnostics(
      "build-json-warn-error",
      "cargo build --workspace --message-format=json",
      101,
    );
    const human = await diagnostics("build-human-warn-error", "cargo build --workspace", 101);
    expect(human.diagnostics).toEqual(json.diagnostics);
    expect(human.summary).toBe(json.summary);
    golden("build-human-warn-error", human);
  });

  test("clippy: JSON and rendered are equal, the lint name is the code (goldens)", async () => {
    const json = await diagnostics("clippy-json", "cargo clippy -p alpha --message-format=json", 0);
    const human = await diagnostics("clippy-human", "cargo clippy -p alpha", 0);
    expect(json.diagnostics).toHaveLength(1);
    expect(json.diagnostics[0]?.code).toBe("unused_variables");
    expect(human.diagnostics).toEqual(json.diagnostics);
    golden("clippy-json", json);
    golden("clippy-human", human);
  });

  test("a green build is an empty diagnostics row, not head+tail (golden)", async () => {
    const r = await diagnostics("build-green", "cargo build", 0);
    expect(r.diagnostics).toEqual([]);
    expect(r.summary).toBe("no diagnostics");
    golden("build-green", r);
  });

  test("a cargo test that failed to compile answers the compile errors", async () => {
    const r = row(
      await runWith("cargo test", { ...captured("build-human-warn-error"), exitCode: 101 }),
    );
    expect(r.kind).toBe("diagnostics");
    expect((r as RunDiagnosticsRow).diagnostics[0]?.code).toBe("E0308");
  });

  test("a test run that compiled is left to the runner adapter even with warnings above it", async () => {
    const r = row(await runWith("cargo test", { ...captured("cargo-test-single"), exitCode: 101 }));
    expect(r.kind).toBe("report");
  });

  test("a non-zero exit with nothing recognizable falls back to head+tail", async () => {
    const r = row(
      await runWith("cargo build", { exitCode: 101, stderr: "Segmentation fault (core dumped)\n" }),
    );
    expect(r.kind).toBe("output");
    expect((r as RunOutputRow).stderrHead[0]).toContain("Segmentation fault");
  });

  test("location-less errors (a linker, a missing subcommand) are still rows; cargo's summaries are not", async () => {
    const stderr = [
      "error: linking with `cc` failed: exit status: 1",
      "  |",
      "  = note: some arguments are omitted",
      "",
      "error: could not compile `x` (bin) due to 1 previous error",
    ].join("\n");
    const r = row(await runWith("cargo build", { exitCode: 101, stderr })) as RunDiagnosticsRow;
    expect(r.kind).toBe("diagnostics");
    expect(r.diagnostics).toHaveLength(1);
    expect(r.diagnostics[0]?.message).toBe("linking with `cc` failed: exit status: 1");
    expect(r.diagnostics[0]?.file).toBeUndefined();
    expect(r.summary).toBe("1 error");
    const missing = row(
      await runWith("cargo clippy", {
        exitCode: 101,
        stderr: "error: no such command: `clippy`\n",
      }),
    ) as RunDiagnosticsRow;
    expect(missing.diagnostics[0]?.message).toBe("no such command: `clippy`");
  });

  test("over budget: errors kept before warnings, the cut carries a manifest, the summary still counts the whole run", async () => {
    const line = "cargo build --workspace --message-format=json";
    const full = await diagnostics("build-json-warn-error", line, 101);
    // A budget that fits the row with its two errors and not the warning.
    const twoErrors = { ...full, diagnostics: full.diagnostics.slice(0, 2) };
    const budget = Math.ceil(JSON.stringify(twoErrors).length / 4) + 1;
    const envelope = await runWith(
      line,
      { ...captured("build-json-warn-error"), exitCode: 101 },
      { budget },
    );
    if (!envelope.ok) throw new Error("expected ok");
    const r = envelope.results[0] as RunDiagnosticsRow;
    expect(r.diagnostics.map((d) => d.level)).toEqual(["error", "error"]);
    expect(r.summary).toBe("2 errors, 1 warning");
    expect(envelope.truncation?.dropped).toBe(1);
    expect(envelope.truncation?.reason).toBe("budget");
    expect(envelope.truncation?.cursor).toBe("diagnostic:2");
    expect(envelope.truncation?.narrow.some((n) => n.includes("warnings were dropped first"))).toBe(
      true,
    );
    expect(envelope.budget.used).toBeLessThanOrEqual(budget);
    // Even a budget too small for one diagnostic keeps one — never an empty
    // row pretending there was nothing to say.
    const tiny = await runWith(
      line,
      { ...captured("build-json-warn-error"), exitCode: 101 },
      { budget: 1 },
    );
    if (!tiny.ok) throw new Error("expected ok");
    expect((tiny.results[0] as RunDiagnosticsRow).diagnostics).toHaveLength(1);
    expect(tiny.truncation?.dropped).toBe(2);
  });

  test("the same warning from a lib and its test build collapses to one", () => {
    const one = readFileSync(path.join(GOLDEN_DIR, "clippy-json.txt"), "utf8");
    const out = extractCargoDiagnostics({
      command: "cargo clippy --all-targets --message-format=json",
      exitCode: 0,
      stdout: `${one}\n${one}`,
      stderr: "",
    });
    expect(out).toHaveLength(1);
  });

  test("cargoSubcommand skips toolchain overrides and flags", () => {
    expect(cargoSubcommand("cargo +nightly build")).toBe("build");
    expect(cargoSubcommand("cargo --locked test -p x")).toBe("test");
    expect(cargoSubcommand("cargo nextest run")).toBe("nextest");
    expect(cargoSubcommand("bun test")).toBeUndefined();
  });
});

// ---------------------------------------------------------------------------
// Live toolchain (SC-006): the goldens' staleness guard.

const tempDirs: string[] = [];
afterAll(() => {
  for (const dir of tempDirs) rmSync(dir, { recursive: true, force: true });
});

function toolAvailable(argv: string[]): boolean {
  try {
    return Bun.spawnSync(argv, { stdout: "pipe", stderr: "pipe" }).exitCode === 0;
  } catch {
    return false;
  }
}

const HAVE_CARGO = toolAvailable(["cargo", "--version"]);
const HAVE_NEXTEST = HAVE_CARGO && toolAvailable(["cargo", "nextest", "--version"]);

function writeFixtureWorkspace(): string {
  const root = mkdtempSync(path.join(tmpdir(), "wb-013-live-"));
  tempDirs.push(root);
  const w = (rel: string, content: string) => {
    mkdirSync(path.dirname(path.join(root, rel)), { recursive: true });
    writeFileSync(path.join(root, rel), content);
  };
  w("Cargo.toml", '[workspace]\nresolver = "2"\nmembers = ["crates/*"]\n');
  w("crates/alpha/Cargo.toml", '[package]\nname = "alpha"\nversion = "0.1.0"\nedition = "2021"\n');
  w(
    "crates/alpha/src/lib.rs",
    [
      "pub fn add(a: i32, b: i32) -> i32 { a + b }",
      "pub fn unused_thing(x: i32) -> i32 { let y = x; x }",
      "#[cfg(test)]",
      "mod tests {",
      "    use super::*;",
      "    #[test]",
      "    fn adds() { assert_eq!(add(1, 2), 3); }",
      "    #[test]",
      '    fn fails_panic() { panic!("boom {}", 42); }',
      "}",
      "",
    ].join("\n"),
  );
  w(
    "crates/beta/Cargo.toml",
    '[package]\nname = "beta"\nversion = "0.1.0"\nedition = "2021"\n[dependencies]\nalpha = { path = "../alpha" }\n',
  );
  w("crates/beta/src/lib.rs", "pub fn twice(a: i32) -> i32 { alpha::add(a, a) }\n");
  w(
    "crates/beta/tests/integration.rs",
    "#[test]\nfn integration_fails() { let v: Vec<i32> = vec![]; let _ = v[0]; }\n",
  );
  return root;
}

function realRun(root: string, argv: string[]): RunExecution {
  const started = Date.now();
  const proc = Bun.spawnSync(argv, {
    cwd: root,
    stdout: "pipe",
    stderr: "pipe",
    env: { ...process.env, CARGO_TERM_COLOR: "never" },
  });
  return {
    exitCode: proc.exitCode,
    stdout: proc.stdout.toString(),
    stderr: proc.stderr.toString(),
    timedOut: false,
    durationMs: Date.now() - started,
  };
}

describe("live toolchain (SC-006)", () => {
  test.skipIf(!HAVE_CARGO)(
    "real cargo test / nextest / build over a fixture workspace match the goldens' shape",
    async () => {
      const root = writeFixtureWorkspace();
      const lines: string[] = [];

      const testLine = "cargo test --workspace --no-fail-fast";
      const testRun = realRun(root, testLine.split(" "));
      expect(testRun.exitCode).toBe(101);
      const testRow = row(await runWith(testLine, testRun)) as RunReportRow;
      expect(testRow.kind).toBe("report");
      expect(testRow.adapter).toBe("cargo");
      const names = testRow.failures.map((f) => f.testName);
      expect(names).toContain("alpha tests::fails_panic");
      expect(names).toContain("integration integration_fails");
      expect(byName(testRow, "alpha tests::fails_panic").assertion).toBe("boom 42");
      expect(byName(testRow, "alpha tests::fails_panic").file).toBe("crates/alpha/src/lib.rs");
      expect(testRow.summary).toContain("2 failed");
      expect(testRow.summary).toContain("rerun: -p alpha --lib, -p beta --test integration");
      lines.push(`cargo test: ${testRow.failures.length} failures, ${testRun.durationMs}ms`);

      if (HAVE_NEXTEST) {
        // As seeded (R8): without --no-fail-fast nextest cancels the run at
        // the first failure and the still-running tests never report.
        const nextestLine = "cargo nextest run --workspace --no-fail-fast";
        const nextestRun = realRun(root, nextestLine.split(" "));
        expect(nextestRun.exitCode).toBe(100);
        const nextestRow = row(await runWith(nextestLine, nextestRun)) as RunReportRow;
        expect(nextestRow.adapter).toBe("nextest");
        expect(nextestRow.failures.map((f) => f.testName).sort()).toEqual([
          "alpha tests::fails_panic",
          "beta::integration integration_fails",
        ]);
        expect(byName(nextestRow, "alpha tests::fails_panic").assertion).toBe("boom 42");
        expect(nextestRow.summary).toMatch(/^Summary \[.*\] 3 tests run: 1 passed, 2 failed/);
        expect(nextestRow.summary).not.toContain("cancelled");
        lines.push(`nextest: ${nextestRow.failures.length} failures, ${nextestRun.durationMs}ms`);
      } else {
        lines.push("nextest: not installed, skipped");
      }

      // A compile error joins the workspace: JSON and rendered agree.
      mkdirSync(path.join(root, "crates/gamma/src"), { recursive: true });
      writeFileSync(
        path.join(root, "crates/gamma/Cargo.toml"),
        '[package]\nname = "gamma"\nversion = "0.1.0"\nedition = "2021"\n',
      );
      writeFileSync(
        path.join(root, "crates/gamma/src/lib.rs"),
        "pub fn broken() -> i32 { let s: String = 1; s }\n",
      );
      const jsonLine = "cargo build --workspace --message-format=json";
      const jsonRun = realRun(root, jsonLine.split(" "));
      expect(jsonRun.exitCode).toBe(101);
      const jsonRow = row(await runWith(jsonLine, jsonRun)) as RunDiagnosticsRow;
      expect(jsonRow.kind).toBe("diagnostics");
      expect(jsonRow.diagnostics[0]?.level).toBe("error");
      expect(jsonRow.diagnostics[0]?.code).toBe("E0308");
      expect(jsonRow.diagnostics[0]?.file).toBe("crates/gamma/src/lib.rs");
      expect(jsonRow.diagnostics.some((d) => d.code === "unused_variables")).toBe(true);
      const humanLine = "cargo build --workspace";
      const humanRow = row(
        await runWith(humanLine, realRun(root, humanLine.split(" "))),
      ) as RunDiagnosticsRow;
      expect(humanRow.kind).toBe("diagnostics");
      expect(humanRow.diagnostics).toEqual(jsonRow.diagnostics);
      lines.push(`build: ${jsonRow.summary} (json == rendered), ${jsonRun.durationMs}ms`);

      console.log(`[013 SC-006] ${lines.join("; ")}`);
    },
    180_000,
  );
});
