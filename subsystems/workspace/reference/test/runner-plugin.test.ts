/**
 * The go runner adapter on the public SDK (chunk:m5-runner-adapter).
 *
 * The chunk's acceptance is EXACTNESS: the component must produce structured
 * output identical to the pre-migration TS adapter. The golden chapter
 * literally diffs the two implementations over fixtures spanning every
 * branch of the parser; the wrapper chapter pins the degradation floor
 * (Constitution I: a dead lane parses exactly as before); the contracts
 * chapter pins zero-capability purity and WIT drift.
 */

import { describe, expect, test } from "bun:test";
import { existsSync, readFileSync } from "node:fs";
import { readFile } from "node:fs/promises";
import path from "node:path";
import { loadCore, nativeArtifactKey } from "../src/adapters/core";
import { componentRunnerAdapter, type RunnerGuestOutcome } from "../src/protocol/runner-plugin";
import { BUNDLED_RUNNER_ADAPTERS, type RunOutput, type RunReport } from "../src/tools";
import { parsePluginManifest } from "../src/tools/plugins";

const ROOT = path.join(import.meta.dir, "..");
const PLUGIN_DIR = path.join(ROOT, "plugins", "runner-go");
const GUEST = path.join(PLUGIN_DIR, "guest.wasm");

const goTwin = BUNDLED_RUNNER_ADAPTERS.find((a) => a.name === "go");
if (goTwin === undefined) throw new Error("bundled go adapter missing");

/** Fixture outputs spanning the go parser's branches. */
const FIXTURES: readonly { readonly label: string; readonly output: RunOutput }[] = [
  {
    label: "single failure with file, assertion, trace and FAIL summary",
    output: {
      command: "go test ./...",
      exitCode: 1,
      stdout: [
        "--- FAIL: TestAdd (0.00s)",
        "    math_test.go:12: expected 4, got 5",
        "    math_test.go:13: extra context",
        "FAIL",
        "FAIL\texample.com/m\t0.004s",
        "",
      ].join("\n"),
      stderr: "",
    },
  },
  {
    label: "multiple failures, subtests, and an ok summary line",
    output: {
      command: "go test -run X ./pkg",
      exitCode: 1,
      stdout: [
        "=== RUN   TestA",
        "--- FAIL: TestA (0.01s)",
        "    a_test.go:8: boom",
        "--- FAIL: TestA/sub_case (0.00s)",
        "    a_test.go:14: nested boom",
        "        with a continuation line",
        "--- PASS: TestB (0.00s)",
        "ok  \texample.com/other\t0.101s",
      ].join("\n"),
      stderr: "",
    },
  },
  {
    label: "failure details on stderr, not stdout",
    output: {
      command: "go test",
      exitCode: 1,
      stdout: "",
      stderr: ["--- FAIL: TestEnv (2.30s)", "    env_test.go:99: missing VAR", "FAIL"].join("\n"),
    },
  },
  {
    label: "all passing: empty failures with a summary",
    output: {
      command: "go test ./...",
      exitCode: 0,
      stdout: ["ok  \texample.com/m\t0.020s", ""].join("\n"),
      stderr: "",
    },
  },
  {
    label: "nonzero exit with no recognizable structure answers undefined",
    output: {
      command: "go test ./...",
      exitCode: 2,
      stdout: "build failed: syntax error in main.go\n",
      stderr: "",
    },
  },
  {
    label: "FAIL header without a duration is not a failure block",
    output: {
      command: "go test",
      exitCode: 1,
      stdout: ["--- FAIL: TestNoDuration", "    x_test.go:1: msg", "FAIL"].join("\n"),
      stderr: "",
    },
  },
  {
    label: "trace trimmed to eight at-lines",
    output: {
      command: "go test",
      exitCode: 1,
      stdout: [
        "--- FAIL: TestBig (0.10s)",
        ...Array.from({ length: 12 }, (_, i) => `    big_test.go:${i + 1}: line ${i + 1}`),
        "FAIL\texample.com/big\t0.5s",
      ].join("\n"),
      stderr: "",
    },
  },
];

const CAN_PARSE_CASES: readonly string[] = [
  "go test ./...",
  "/usr/local/go/bin/go test",
  "C:\\go\\bin\\go.exe test -v",
  "GO=1 go vet",
  "cargo test",
  "gotestsum ./...",
  "go build",
];

const nativeArtifact = path.join(
  ROOT,
  "crates",
  "workbench-core",
  "artifacts",
  `workbench-core.${nativeArtifactKey()}.node`,
);

describe.if(existsSync(nativeArtifact))("golden equality: component vs TS twin", () => {
  const { binding } = loadCore({ force: "native" });

  async function componentParse(output: RunOutput): Promise<RunnerGuestOutcome> {
    if (binding.sandboxRunnerParse === undefined) {
      throw new Error("native binding lacks the runner lane");
    }
    const guest = new Uint8Array(await readFile(GUEST));
    return binding.sandboxRunnerParse(
      guest,
      output.command,
      output.exitCode,
      output.stdout,
      output.stderr,
      [],
      { fuel: 200_000_000, memoryCapBytes: 32 * 1024 * 1024 },
    ) as Promise<RunnerGuestOutcome>;
  }

  /** The guest's lifted report folded to the tools shape, like the wrapper. */
  function folded(outcome: RunnerGuestOutcome): RunReport | undefined {
    const report = outcome.report ?? null;
    if (outcome.outcome !== "ok" || !outcome.claimed || report === null) {
      return undefined;
    }
    return {
      failures: report.failures.map((f) => ({
        testName: f.testName,
        ...(f.file === null || f.file === undefined ? {} : { file: f.file }),
        assertion: f.assertion,
        trace: f.trace,
      })),
      ...(report.summary === null || report.summary === undefined
        ? {}
        : { summary: report.summary }),
    };
  }

  for (const fixture of FIXTURES) {
    test(`identical structure: ${fixture.label}`, async () => {
      const twin = goTwin.parseOutput(fixture.output);
      const outcome = await componentParse(fixture.output);
      expect(outcome.outcome).toBe("ok");
      expect(outcome.claimed).toBe(true);
      expect(folded(outcome)).toEqual(twin as RunReport | undefined);
    }, 30_000);
  }

  test("identical claims across command-line shapes", async () => {
    if (binding.sandboxRunnerParse === undefined) {
      throw new Error("native binding lacks the runner lane");
    }
    const guest = new Uint8Array(await readFile(GUEST));
    for (const line of CAN_PARSE_CASES) {
      const outcome = await binding.sandboxRunnerParse(guest, line, 0, "", "", [], {
        fuel: 200_000_000,
        memoryCapBytes: 32 * 1024 * 1024,
      });
      expect(`${line}: ${outcome.claimed}`).toBe(`${line}: ${String(goTwin.canParse(line))}`);
    }
  }, 30_000);
});

describe("componentRunnerAdapter wrapper", () => {
  const output: RunOutput = {
    command: "go test",
    exitCode: 1,
    stdout: "--- FAIL: TestX (0.00s)\n    x_test.go:1: nope\nFAIL\tm\t0.1s\n",
    stderr: "",
  };
  const twinReport = goTwin.parseOutput(output);

  test("a healthy lane answers the folded guest report", async () => {
    const adapter = componentRunnerAdapter({
      name: "go",
      guestPath: "/x/guest.wasm",
      invoke: () =>
        Promise.resolve({
          outcome: "ok",
          detail: "",
          fuelUsed: 10,
          claimed: true,
          report: {
            failures: [{ testName: "TestX", file: "x_test.go", assertion: "nope", trace: "t" }],
            summary: "FAIL\tm\t0.1s",
          },
        } satisfies RunnerGuestOutcome),
      fallback: goTwin,
    });
    expect(await adapter.canParse(output.command)).toBe(true);
    expect(await adapter.parseOutput(output)).toEqual({
      failures: [{ testName: "TestX", file: "x_test.go", assertion: "nope", trace: "t" }],
      summary: "FAIL\tm\t0.1s",
    });
  });

  test("a null guest report is the honest 'no structure' answer, not a fallback", async () => {
    const adapter = componentRunnerAdapter({
      name: "go",
      guestPath: "/x/guest.wasm",
      invoke: () =>
        Promise.resolve({
          outcome: "ok",
          detail: "",
          fuelUsed: 10,
          claimed: true,
          report: null,
        } satisfies RunnerGuestOutcome),
      fallback: goTwin,
    });
    expect(await adapter.parseOutput(output)).toBeUndefined();
  });

  test("a dead lane degrades to the TS twin — pre-migration behavior exactly", async () => {
    const adapter = componentRunnerAdapter({
      name: "go",
      guestPath: "/x/guest.wasm",
      invoke: () => Promise.reject(new Error("engine child down")),
      fallback: goTwin,
    });
    expect(await adapter.parseOutput(output)).toEqual(twinReport as RunReport);
  });

  test("a trapped guest degrades to the TS twin too", async () => {
    const adapter = componentRunnerAdapter({
      name: "go",
      guestPath: "/x/guest.wasm",
      invoke: () =>
        Promise.resolve({
          outcome: "guest-trap",
          detail: "unreachable",
          fuelUsed: 5,
          claimed: false,
          report: null,
        } satisfies RunnerGuestOutcome),
      fallback: goTwin,
    });
    expect(await adapter.parseOutput(output)).toEqual(twinReport as RunReport);
  });
});

describe("contracts: purity and drift pins", () => {
  test("the manifest declares ZERO capability imports (a runner is a pure function)", () => {
    const parsed = parsePluginManifest(readFileSync(path.join(PLUGIN_DIR, "plugin.toml"), "utf8"));
    if (!parsed.ok) throw new Error(parsed.errors.join("; "));
    expect(parsed.manifest.publisher).toBe("workbench");
    expect(parsed.manifest.pkg).toBe("runner-go");
    expect([...parsed.manifest.imports]).toEqual([]);
  });

  test("the guest's wit/deps copy is byte-identical to the public wit/", () => {
    expect(
      readFileSync(path.join(PLUGIN_DIR, "guest", "wit", "deps", "runner", "runner.wit"), "utf8"),
    ).toBe(readFileSync(path.join(ROOT, "wit", "runner.wit"), "utf8"));
  });
});
