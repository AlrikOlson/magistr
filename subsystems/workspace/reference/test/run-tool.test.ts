/**
 * `run` (SDD §9.6) — chunk:m3-run-tool.
 *
 * Pins the acceptance criteria: an undeclared command answers
 * COMMAND_NOT_DECLARED (abort, message pointing at the config layer);
 * attempted text munging or shell execution answers RUN_REDIRECTED with a
 * redirect to search/edit; the six bundled runner adapters
 * (bun/jest/vitest/pytest/cargo/go) extract §9.6's structured failure
 * quartet from realistic output; unknown commands fall back to head+tail
 * with a truncation manifest; lockfile/manifest detection seeds the
 * allowlist at `workbench-mcp init` and never clobbers a human-amended
 * table; and the served surface offers `run` only when an executor lane was
 * composed, sealing (redacting) every result like any other tool.
 */

import { afterAll, describe, expect, test } from "bun:test";
import { mkdirSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { Client } from "@modelcontextprotocol/sdk/client/index.js";
import { InMemoryTransport } from "@modelcontextprotocol/sdk/inMemory.js";
import { defaultResolvedConfig } from "../src/config";
import { detectCommands, renderCommandsTable } from "../src/config/detect";
import type { ErrorEnvelope, ResponseEnvelope } from "../src/envelope";
import { buildServer, type RunRuntime, type WorkspaceRuntime } from "../src/protocol";
import type { SearchDeps } from "../src/tools";
import {
  BUNDLED_RUNNER_ADAPTERS,
  guardCommandLine,
  type RunDiagnosticsRow,
  type RunExecution,
  type RunOutputRow,
  type RunReportRow,
  type RunRow,
  runRun,
} from "../src/tools/run";

const tempDirs: string[] = [];
afterAll(() => {
  for (const dir of tempDirs) rmSync(dir, { recursive: true, force: true });
});

function tempDir(prefix: string): string {
  const dir = mkdtempSync(join(tmpdir(), prefix));
  tempDirs.push(dir);
  return dir;
}

const okExecution = (over: Partial<RunExecution> = {}): RunExecution => ({
  exitCode: 0,
  stdout: "",
  stderr: "",
  timedOut: false,
  durationMs: 5,
  ...over,
});

function scriptedRun(
  commands: Record<string, string>,
  execution: RunExecution | ((argv: readonly string[]) => RunExecution),
): {
  deps: { commands: Record<string, string>; execute: (typeof calls)["execute"] };
  argvSeen: string[][];
  timeoutsSeen: number[];
} {
  const argvSeen: string[][] = [];
  const timeoutsSeen: number[] = [];
  const calls = {
    execute: (argv: readonly string[], opts: { timeoutMs: number }) => {
      argvSeen.push([...argv]);
      timeoutsSeen.push(opts.timeoutMs);
      return Promise.resolve(typeof execution === "function" ? execution(argv) : execution);
    },
  };
  return { deps: { commands, execute: calls.execute }, argvSeen, timeoutsSeen };
}

function expectError(envelope: ResponseEnvelope<RunRow>): ErrorEnvelope["error"] {
  if (envelope.ok) throw new Error(`expected error, got ok: ${JSON.stringify(envelope)}`);
  return envelope.error;
}

function expectRow(envelope: ResponseEnvelope<RunRow>): RunRow {
  if (!envelope.ok) throw new Error(`expected ok, got ${JSON.stringify(envelope.error)}`);
  const row = envelope.results[0];
  if (row === undefined) throw new Error("empty results");
  return row;
}

// ---------------------------------------------------------------------------

describe("the allowlist (acceptance #1)", () => {
  test("an undeclared command answers COMMAND_NOT_DECLARED pointing at the config layer", async () => {
    const { deps } = scriptedRun({ test: "bun test" }, okExecution());
    const error = expectError(await runRun(deps, { command: "deploy" }, 1));
    expect(error.code).toBe("COMMAND_NOT_DECLARED");
    expect(error.recovery).toBe("abort");
    expect(error.message).toContain("workbench.toml");
    expect(error.message).toContain("[commands]");
    expect(error.message).toContain("test"); // the declared table is named
  });

  test("a declared command executes as one argv with the extra args appended", async () => {
    const { deps, argvSeen, timeoutsSeen } = scriptedRun({ test: "bun test" }, okExecution());
    const row = expectRow(await runRun(deps, { command: "test", args: ["--filter", "auth"] }, 1));
    expect(argvSeen[0]).toEqual(["bun", "test", "--filter", "auth"]);
    expect(timeoutsSeen[0]).toBe(300_000); // §9.6 default, seconds -> ms
    expect(row.command).toBe("test");
  });

  test("schema-level misuse throws (never an envelope)", async () => {
    const { deps } = scriptedRun({ test: "bun test" }, okExecution());
    expect(runRun(deps, { command: "test", timeout: 0 }, 1)).rejects.toThrow(RangeError);
    expect(runRun(deps, { command: "test", timeout: 9999 }, 1)).rejects.toThrow(RangeError);
    expect(runRun(deps, { command: "" }, 1)).rejects.toThrow(RangeError);
  });
});

describe("the no-shell guard (acceptance #2)", () => {
  test("a declared munging command is rejected with the search/edit redirect", async () => {
    const { deps, argvSeen } = scriptedRun({ find: "grep -rn TODO src" }, okExecution());
    const error = expectError(await runRun(deps, { command: "find" }, 1));
    expect(error.code).toBe("RUN_REDIRECTED");
    expect(error.recovery).toBe("abort");
    expect(error.message).toContain("search");
    expect(error.message).toContain("edit");
    expect(argvSeen).toHaveLength(0); // rejected before any execution
  });

  test("a shell as the program is general shell execution, refused", async () => {
    const { deps } = scriptedRun({ sh: "bash -c ls" }, okExecution());
    const error = expectError(await runRun(deps, { command: "sh" }, 1));
    expect(error.code).toBe("RUN_REDIRECTED");
    expect(error.message).toContain("shell");
  });

  test("shell operators in the declared line are rejected — run provides no shell", async () => {
    const { deps } = scriptedRun({ weird: "cargo test | tail -20" }, okExecution());
    const error = expectError(await runRun(deps, { command: "weird" }, 1));
    expect(error.code).toBe("RUN_REDIRECTED");
  });

  test("munging via the args vector is rejected too", async () => {
    const { deps, argvSeen } = scriptedRun({ test: "bun test" }, okExecution());
    const error = expectError(
      await runRun(deps, { command: "test", args: ["|", "grep", "fail"] }, 1),
    );
    expect(error.code).toBe("RUN_REDIRECTED");
    expect(argvSeen).toHaveLength(0);
  });

  test("the guard sees through paths, .exe suffixes and passthrough wrappers", () => {
    expect(guardCommandLine("C:\\tools\\grep.exe -rn TODO", [])?.why).toBe("munging-program");
    expect(guardCommandLine("/usr/bin/sed -i s/a/b/", [])?.why).toBe("munging-program");
    expect(guardCommandLine("npx sed s/a/b/", [])?.why).toBe("munging-program");
    expect(guardCommandLine("env FOO=1 bash script", [])?.why).toBe("shell-program");
    // Legitimate runners pass, including under a passthrough wrapper.
    expect(guardCommandLine("npx vitest run", [])).toBeUndefined();
    expect(guardCommandLine("cargo test --workspace", ["--", "--nocapture"])).toBeUndefined();
    // A munging NAME in a plain argument position is just an argument.
    expect(guardCommandLine("bun test", ["--filter", "cat"])).toBeUndefined();
  });
});

// ---------------------------------------------------------------------------
// Bundled adapters (acceptance #3): realistic fixtures for all six runners.

const BUN_OUTPUT = [
  "test/auth.test.ts:",
  "✓ login > accepts a valid token [1.02ms]",
  "✗ login > rejects an expired token [2.11ms]",
  "  error: expect(received).toBe(expected)",
  "  Expected: 401",
  "  Received: 200",
  "      at test/auth.test.ts:42:5",
  "",
  " 1 pass",
  " 1 fail",
].join("\n");

const JEST_OUTPUT = [
  "FAIL src/auth.test.js",
  "  ● login › rejects an expired token",
  "",
  "    expect(received).toBe(expected) // Object.is equality",
  "",
  "    Expected: 401",
  "    Received: 200",
  "",
  "      at Object.<anonymous> (src/auth.test.js:42:19)",
  "",
  "Tests:       1 failed, 3 passed, 4 total",
].join("\n");

const VITEST_OUTPUT = [
  " ❯ src/auth.test.ts > login > rejects an expired token",
  "AssertionError: expected 200 to be 401",
  " ❯ src/auth.test.ts:42:19",
  "",
  " Tests  1 failed | 3 passed (4)",
].join("\n");

const PYTEST_OUTPUT = [
  "=================================== FAILURES ===================================",
  "____________________________ test_rejects_expired _____________________________",
  "",
  "    def test_rejects_expired():",
  ">       assert resp.status == 401",
  "E       assert 200 == 401",
  "",
  "tests/test_auth.py:42: AssertionError",
  "=========================== short test summary info ============================",
  "FAILED tests/test_auth.py::test_rejects_expired - assert 200 == 401",
  "========================= 1 failed, 3 passed in 0.21s ==========================",
].join("\n");

const CARGO_OUTPUT = [
  "running 4 tests",
  "test auth::accepts_valid ... ok",
  "test auth::rejects_expired ... FAILED",
  "",
  "failures:",
  "",
  "---- auth::rejects_expired stdout ----",
  "thread 'auth::rejects_expired' panicked at src/auth.rs:42:9:",
  "assertion `left == right` failed",
  "  left: 200",
  " right: 401",
  "",
  "failures:",
  "    auth::rejects_expired",
  "",
  "test result: FAILED. 3 passed; 1 failed; 0 ignored; 0 measured; 0 filtered out",
].join("\n");

const GO_OUTPUT = [
  "--- FAIL: TestRejectsExpired (0.00s)",
  "    auth_test.go:42: got status 200, want 401",
  "FAIL",
  "FAIL\texample.com/auth\t0.012s",
].join("\n");

describe("bundled runner adapters (acceptance #3)", () => {
  const cases: readonly {
    adapter: string;
    line: string;
    stdout: string;
    testName: string;
    file: string;
    assertionFragment: string;
  }[] = [
    {
      adapter: "bun",
      line: "bun test",
      stdout: BUN_OUTPUT,
      testName: "login > rejects an expired token",
      file: "test/auth.test.ts",
      assertionFragment: "expect(received).toBe(expected)",
    },
    {
      adapter: "jest",
      line: "npx jest",
      stdout: JEST_OUTPUT,
      testName: "login › rejects an expired token",
      file: "src/auth.test.js",
      assertionFragment: "expect(received).toBe(expected)",
    },
    {
      adapter: "vitest",
      line: "npx vitest run",
      stdout: VITEST_OUTPUT,
      testName: "login > rejects an expired token",
      file: "src/auth.test.ts",
      assertionFragment: "expected 200 to be 401",
    },
    {
      adapter: "pytest",
      line: "pytest",
      stdout: PYTEST_OUTPUT,
      testName: "tests/test_auth.py::test_rejects_expired",
      file: "tests/test_auth.py",
      assertionFragment: "assert 200 == 401",
    },
    {
      adapter: "cargo",
      line: "cargo test",
      stdout: CARGO_OUTPUT,
      testName: "auth::rejects_expired",
      file: "src/auth.rs",
      assertionFragment: "assertion `left == right` failed",
    },
    {
      adapter: "go",
      line: "go test ./...",
      stdout: GO_OUTPUT,
      testName: "TestRejectsExpired",
      file: "auth_test.go",
      assertionFragment: "got status 200, want 401",
    },
  ];

  for (const c of cases) {
    test(`${c.adapter}: one failure extracted with the §9.6 quartet`, async () => {
      const { deps } = scriptedRun({ t: c.line }, okExecution({ exitCode: 1, stdout: c.stdout }));
      const row = expectRow(await runRun(deps, { command: "t" }, 1)) as RunReportRow;
      expect(row.kind).toBe("report");
      expect(row.adapter).toBe(c.adapter);
      expect(row.failures).toHaveLength(1);
      const failure = row.failures[0];
      expect(failure?.testName).toBe(c.testName);
      expect(failure?.file).toBe(c.file);
      expect(failure?.assertion).toContain(c.assertionFragment);
      expect(row.summary).toBeDefined();
    });
  }

  test("every bundled adapter claims only its own command line", () => {
    const lines: Record<string, string> = {
      bun: "bun test",
      vitest: "npx vitest run",
      jest: "npx jest",
      pytest: "pytest -q",
      nextest: "cargo nextest run --workspace",
      cargo: "cargo test --workspace",
      go: "go test ./...",
    };
    for (const adapter of BUNDLED_RUNNER_ADAPTERS) {
      for (const [name, line] of Object.entries(lines)) {
        expect(adapter.canParse(line)).toBe(adapter.name === name);
      }
    }
  });

  test("a green run from a known runner is a report with zero failures", async () => {
    const { deps } = scriptedRun(
      { t: "cargo test" },
      okExecution({ stdout: "running 4 tests\ntest result: ok. 4 passed; 0 failed" }),
    );
    const row = expectRow(await runRun(deps, { command: "t" }, 1)) as RunReportRow;
    expect(row.kind).toBe("report");
    expect(row.failures).toHaveLength(0);
    expect(row.summary).toContain("4 passed");
  });

  test("a claimed runner whose output defies extraction falls back honestly", async () => {
    const { deps } = scriptedRun(
      { t: "cargo test" },
      okExecution({ exitCode: 101, stdout: "Segmentation fault (core dumped)\n" }),
    );
    const row = expectRow(await runRun(deps, { command: "t" }, 1));
    expect(row.kind).toBe("output"); // never a silent guess at structure
  });

  test("a test run that never compiled answers the compile errors as diagnostics (013)", async () => {
    // Pre-013 this exact input fell to head+tail; the error IS the structure.
    const { deps } = scriptedRun(
      { t: "cargo test" },
      okExecution({ exitCode: 101, stdout: "error[E0308]: mismatched types\n(compile failure)" }),
    );
    const row = expectRow(await runRun(deps, { command: "t" }, 1)) as RunDiagnosticsRow;
    expect(row.kind).toBe("diagnostics");
    expect(row.diagnostics[0]?.code).toBe("E0308");
    expect(row.diagnostics[0]?.message).toBe("mismatched types");
    expect(row.summary).toBe("1 error");
  });
});

describe("head+tail fallback with a truncation manifest", () => {
  test("an unknown command's long output is cut with a manifest, never dumped", async () => {
    const lines = Array.from({ length: 500 }, (_, i) => `line ${i}`).join("\n");
    const { deps } = scriptedRun({ gen: "protoc --version" }, okExecution({ stdout: lines }));
    const envelope = await runRun(deps, { command: "gen", budget: 100 }, 1);
    if (!envelope.ok) throw new Error("expected ok");
    const row = envelope.results[0] as RunOutputRow;
    expect(row.kind).toBe("output");
    expect(row.stdoutHead[0]).toBe("line 0");
    expect(row.stdoutTail.at(-1)).toBe("line 499");
    expect(envelope.truncation?.dropped).toBeGreaterThan(0);
    expect(envelope.truncation?.reason).toBe("budget");
  });

  test("short output arrives whole — a manifest only exists when something was cut", async () => {
    const { deps } = scriptedRun({ gen: "protoc --version" }, okExecution({ stdout: "ok\n" }));
    const envelope = await runRun(deps, { command: "gen" }, 1);
    if (!envelope.ok) throw new Error("expected ok");
    expect(envelope.truncation).toBeUndefined();
  });

  test("a timeout is data: partial output, timedOut true, a note — not an error", async () => {
    const { deps } = scriptedRun(
      { slow: "protoc --wait" },
      okExecution({ exitCode: null, stdout: "partial", timedOut: true, durationMs: 2000 }),
    );
    const envelope = await runRun(deps, { command: "slow", timeout: 2 }, 1);
    if (!envelope.ok) throw new Error("expected ok");
    const row = envelope.results[0] as RunOutputRow;
    expect(row.timedOut).toBe(true);
    expect(row.exitCode).toBeNull();
    expect(envelope.notes?.some((n) => n.includes("timed out"))).toBe(true);
  });
});

// ---------------------------------------------------------------------------
// Detection + init seeding (the m1-map-enrichment deferral).

describe("lockfile/manifest detection seeds the allowlist (init)", () => {
  test("package.json scripts are discriminated by lockfile", () => {
    const root = tempDir("wb-detect-bun-");
    writeFileSync(
      join(root, "package.json"),
      JSON.stringify({ scripts: { test: "bun test", build: "tsc", irrelevant: "x" } }),
    );
    writeFileSync(join(root, "bun.lock"), "");
    const detected = detectCommands(root);
    expect(detected.find((d) => d.name === "test")?.line).toBe("bun run test");
    expect(detected.find((d) => d.name === "build")?.line).toBe("bun run build");
    expect(detected.find((d) => d.name === "irrelevant")).toBeUndefined();
  });

  test("cargo, go and pytest projects detect their runners", () => {
    const root = tempDir("wb-detect-multi-");
    writeFileSync(join(root, "Cargo.toml"), '[package]\nname = "x"');
    writeFileSync(join(root, "go.mod"), "module example.com/x");
    writeFileSync(join(root, "pyproject.toml"), "[tool.pytest.ini_options]\n");
    const names = detectCommands(root).map((d) => d.name);
    expect(names).toContain("cargo-test");
    expect(names).toContain("go-test");
    expect(names).toContain("pytest");
  });

  test("a Cargo workspace seeds a --no-fail-fast whole-workspace run and one command per member (013)", () => {
    const root = tempDir("wb-detect-cargo-ws-");
    const w = (rel: string, content: string): void => {
      const abs = join(root, rel);
      mkdirSync(join(abs, ".."), { recursive: true });
      writeFileSync(abs, content);
    };
    w(
      "Cargo.toml",
      [
        "[workspace]",
        'members = ["crates/*", "tools/cli", "vendor/**"]',
        'exclude = ["crates/skipme"]',
        "",
        "[package]",
        'name = "rootpkg"',
        'version = "0.1.0"',
        "",
      ].join("\n"),
    );
    w("crates/alpha/Cargo.toml", '[package]\nname = "alpha"\nversion = "0.1.0"\n');
    w("crates/beta/Cargo.toml", '[package]\nname = "beta-crate"\nversion = "0.1.0"\n');
    w("crates/skipme/Cargo.toml", '[package]\nname = "skipme"\n');
    w("crates/notacrate/README.md", "not a crate\n");
    w("tools/cli/Cargo.toml", '[package]\nname = "cli"\n');
    const detected = detectCommands(root);
    const line = (name: string): string | undefined => detected.find((d) => d.name === name)?.line;
    expect(line("cargo-test")).toBe("cargo test --workspace --no-fail-fast");
    expect(line("cargo-build")).toBe("cargo build --workspace");
    expect(line("cargo-test-alpha")).toBe("cargo test -p alpha");
    expect(line("cargo-test-beta-crate")).toBe("cargo test -p beta-crate");
    expect(line("cargo-test-cli")).toBe("cargo test -p cli");
    expect(line("cargo-test-rootpkg")).toBe("cargo test -p rootpkg");
    expect(line("cargo-test-skipme")).toBeUndefined(); // excluded
    expect(line("cargo-test-notacrate")).toBeUndefined(); // no manifest
    // An unexpanded glob is reported, never silently dropped.
    expect(detected.find((d) => d.name === "cargo-test")?.provenance).toContain("vendor/**");
    // Nothing declares nextest or clippy yet.
    expect(line("cargo-nextest")).toBeUndefined();
    expect(line("cargo-clippy")).toBeUndefined();
    w(".config/nextest.toml", "[profile.default]\nretries = 0\n");
    w(
      "rust-toolchain.toml",
      '[toolchain]\nchannel = "stable"\ncomponents = ["rustfmt", "clippy"]\n',
    );
    const declared = detectCommands(root);
    expect(declared.find((d) => d.name === "cargo-nextest")?.line).toBe(
      "cargo nextest run --workspace --no-fail-fast",
    );
    expect(declared.find((d) => d.name === "cargo-clippy")?.line).toBe(
      "cargo clippy --workspace --all-targets",
    );
  });

  test("a single crate seeds exactly as before; a huge workspace's member seeds are capped honestly (013)", () => {
    const single = tempDir("wb-detect-cargo-crate-");
    writeFileSync(join(single, "Cargo.toml"), '[package]\nname = "solo"\nversion = "0.1.0"\n');
    writeFileSync(join(single, ".config"), ""); // a FILE named .config is not a nextest config
    const crate = detectCommands(single);
    expect(crate.map((d) => [d.name, d.line])).toEqual([
      ["cargo-test", "cargo test"],
      ["cargo-build", "cargo build"],
    ]);

    const huge = tempDir("wb-detect-cargo-huge-");
    writeFileSync(join(huge, "Cargo.toml"), '[workspace]\nmembers = ["crates/*"]\n');
    for (let i = 0; i < 40; i++) {
      const name = `m${String(i).padStart(2, "0")}`;
      mkdirSync(join(huge, "crates", name), { recursive: true });
      writeFileSync(join(huge, "crates", name, "Cargo.toml"), `[package]\nname = "${name}"\n`);
    }
    const seeds = detectCommands(huge).filter((d) => d.name.startsWith("cargo-test-"));
    expect(seeds).toHaveLength(32);
    expect(seeds[0]?.name).toBe("cargo-test-m00");
    expect(seeds[31]?.provenance).toContain("8 more members not seeded");
  });

  test("detection never executes and never writes", () => {
    const root = tempDir("wb-detect-empty-");
    expect(detectCommands(root)).toEqual([]);
  });

  test("rendered [commands] tables parse as TOML and carry provenance", () => {
    const rendered = renderCommandsTable([
      { name: "test", line: "bun run test", provenance: "package.json scripts.test + bun.lock" },
    ]);
    const parsed = Bun.TOML.parse(rendered) as { commands: Record<string, string> };
    expect(parsed.commands["test"]).toBe("bun run test");
    expect(rendered).toContain("only a human may change it");
  });

  test("`workbench-mcp init` seeds workbench.toml once and never clobbers a human table", () => {
    const root = tempDir("wb-init-");
    writeFileSync(join(root, "package.json"), JSON.stringify({ scripts: { test: "bun test" } }));
    writeFileSync(join(root, "bun.lock"), "");
    const main = join(import.meta.dirname, "..", "src", "main.ts");
    const first = Bun.spawnSync(["bun", main, "init", root]);
    expect(first.exitCode).toBe(0);
    const seeded = readFileSync(join(root, "workbench.toml"), "utf8");
    expect((Bun.TOML.parse(seeded) as { commands: Record<string, string> }).commands["test"]).toBe(
      "bun run test",
    );
    // A human amends the table; init must refuse to touch it again.
    writeFileSync(join(root, "workbench.toml"), '[commands]\n"test" = "bun test --coverage"\n');
    const second = Bun.spawnSync(["bun", main, "init", root]);
    expect(second.exitCode).toBe(0);
    expect(new TextDecoder().decode(second.stderr)).toContain("left untouched");
    const kept = readFileSync(join(root, "workbench.toml"), "utf8");
    expect(kept).toContain("bun test --coverage");
  });
});

// ---------------------------------------------------------------------------
// The served surface: honest absence, config-sourced allowlist, sealing.

function fakeSearch(): SearchDeps {
  return {
    engine: {
      searchLexical: () => Promise.reject(new Error("not under test")),
      searchStructural: () => Promise.reject(new Error("not under test")),
      parseCheck: () => Promise.reject(new Error("not under test")),
      applyRewrite: () => Promise.reject(new Error("not under test")),
    } as unknown as SearchDeps["engine"],
    index: {
      status: () => Promise.resolve({ state: "absent" as const }),
      build: () => Promise.reject(new Error("not under test")),
      outline: () => Promise.resolve(null),
      resolveSymbol: () => Promise.resolve([]),
      references: () => Promise.resolve([]),
      searchFiles: () => Promise.resolve([]),
    } as unknown as SearchDeps["index"],
    workspace: {
      list: () => Promise.resolve([]),
      read: () => Promise.reject(new Error("not under test")),
    },
  };
}

function runWorkspace(commands: Record<string, string>): WorkspaceRuntime {
  const root = tempDir("wb-run-ws-");
  mkdirSync(join(root, ".workbench"), { recursive: true });
  return {
    root,
    stateDir: join(root, ".workbench"),
    detection: { networkFs: false, method: "default-local" },
    journalMode: "WAL",
    appliedJournalMode: "wal",
    config: { ...defaultResolvedConfig(), commands },
    notes: [],
    close: () => {},
  };
}

function envelopeFrom(result: unknown): ResponseEnvelope<RunRow> {
  const content = (result as { content: Array<{ type: string; text: string }> }).content;
  const first = content[0];
  if (first === undefined) throw new Error("empty content");
  return JSON.parse(first.text) as ResponseEnvelope<RunRow>;
}

async function connectedClient(workspace: WorkspaceRuntime, run?: RunRuntime): Promise<Client> {
  const [clientTransport, serverTransport] = InMemoryTransport.createLinkedPair();
  const server = buildServer(workspace, fakeSearch(), undefined, undefined, undefined, run);
  const client = new Client({ name: "run-test", version: "0.0.0" });
  await server.connect(serverTransport);
  await client.connect(clientTransport);
  return client;
}

describe("the served surface", () => {
  test("run is offered only when an executor lane was composed (§6.2 honest absence)", async () => {
    const without = await connectedClient(runWorkspace({}));
    const bare = await without.listTools();
    expect(bare.tools.some((t) => t.name === "run")).toBe(false);
    expect(without.callTool({ name: "run", arguments: { command: "test" } })).rejects.toThrow(
      /not offered/,
    );

    const withLane = await connectedClient(runWorkspace({}), {
      execute: () => Promise.resolve(okExecution()),
    });
    const listed = await withLane.listTools();
    expect(listed.tools.some((t) => t.name === "run")).toBe(true);
  });

  test("the allowlist is the merged config's — an undeclared command answers typed over the wire", async () => {
    const client = await connectedClient(runWorkspace({ test: "bun test" }), {
      execute: () => Promise.resolve(okExecution()),
    });
    const envelope = envelopeFrom(
      await client.callTool({ name: "run", arguments: { command: "deploy" } }),
    );
    expect(envelope.ok).toBe(false);
    if (!envelope.ok) expect(envelope.error.code).toBe("COMMAND_NOT_DECLARED");
  });

  test("a declared command's structured report rides the sealed envelope — secrets redacted", async () => {
    const secretOutput = [
      "✗ config > loads the token [1ms]",
      "  error: leaked ghp_0123456789abcdefghijklmnopqrstuvwxyz012345",
      "      at test/config.test.ts:7:3",
      " 0 pass",
      " 1 fail",
    ].join("\n");
    const client = await connectedClient(runWorkspace({ test: "bun test" }), {
      execute: () => Promise.resolve(okExecution({ exitCode: 1, stdout: secretOutput })),
    });
    const envelope = envelopeFrom(
      await client.callTool({ name: "run", arguments: { command: "test" } }),
    );
    if (!envelope.ok) throw new Error("expected ok");
    const row = envelope.results[0] as RunReportRow;
    expect(row.kind).toBe("report");
    expect(row.adapter).toBe("bun");
    const wire = JSON.stringify(envelope);
    expect(wire).toContain("[REDACTED:github-token]");
    expect(wire).not.toContain("ghp_0123456789");
  });

  test("a build's diagnostics row rides the sealed envelope (013)", async () => {
    const rendered = [
      "error[E0425]: cannot find value `nope` in this scope",
      " --> src\\main.rs:3:5",
      "  |",
      "3 |     nope",
      "  |     ^^^^ not found in this scope",
      "",
      "error: could not compile `x` (bin) due to 1 previous error",
    ].join("\n");
    const client = await connectedClient(runWorkspace({ build: "cargo build" }), {
      execute: () => Promise.resolve(okExecution({ exitCode: 101, stderr: rendered })),
    });
    const envelope = envelopeFrom(
      await client.callTool({ name: "run", arguments: { command: "build" } }),
    );
    if (!envelope.ok) throw new Error("expected ok");
    const row = envelope.results[0] as RunDiagnosticsRow;
    expect(row.kind).toBe("diagnostics");
    expect(row.extractor).toBe("cargo");
    expect(row.diagnostics).toEqual([
      {
        level: "error",
        code: "E0425",
        message: "cannot find value `nope` in this scope",
        file: "src/main.rs",
        line: 3,
        column: 5,
        detail: "  |\n3 |     nope\n  |     ^^^^ not found in this scope",
      },
    ]);
    expect(row.summary).toBe("1 error");
  });

  test("munging via the served surface answers RUN_REDIRECTED", async () => {
    const client = await connectedClient(runWorkspace({ grep: "grep -rn TODO" }), {
      execute: () => Promise.resolve(okExecution()),
    });
    const envelope = envelopeFrom(
      await client.callTool({ name: "run", arguments: { command: "grep" } }),
    );
    expect(envelope.ok).toBe(false);
    if (!envelope.ok) {
      expect(envelope.error.code).toBe("RUN_REDIRECTED");
      expect(envelope.error.message).toContain("search");
    }
  });
});
