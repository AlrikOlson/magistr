/**
 * `run` (SDD §9.6) — the declared-command escape hatch. Commands come from
 * the merged config's `[commands]` table (§7.2): seeded at init from
 * lockfile/manifest detection (src/config/detect.ts), amendable only by a
 * human (workbench.toml is a PROTECTED_FILE — the server cannot self-amend
 * its own allowlist, constitution IV), governable by org policy (a centrally
 * pinned table). An undeclared command answers COMMAND_NOT_DECLARED
 * (recovery abort) pointing the human at their config layer.
 *
 * No general shell, ever (§1.1, constitution IV): the resolved command line
 * is executed as a single argv with no shell interpretation. Shell operators
 * in the declared line or the args vector, shells as the program, and text
 * munging programs anywhere in a program position are rejected with
 * RUN_REDIRECTED (§4.3 v1.5) — the message redirects to search/read for
 * inspection and edit for modification, because an untyped escape hatch is
 * bash by another name.
 *
 * Output is parsed, never dumped: bundled runner adapters (bun/jest/vitest/
 * pytest/cargo/go — SDD §8.3's first-party six) extract §9.6's structured
 * failure quartet (test name, file, assertion, trimmed trace) on the
 * wit/runner.wit vocabulary verbatim; the first claimant in trust order
 * parses. An adapter that cannot extract structure returns undefined and the
 * host falls back to head+tail with a truncation manifest — an honest cut,
 * never a silent guess. Runner-adapter PLUGINS extend the same interface in
 * M5 (§8.3); only the bundled six live here.
 *
 * The executor is injected (like ScriptRuntime.runGuest): the actual spawn
 * lives in the composing layer, tests script it, and this module stays a
 * pure decision surface. A timeout is data (timedOut: true, exitCode null),
 * not an error envelope — the agent can act on partial output.
 */

import { okEnvelope, type ResponseEnvelope, type Truncation, taxonomyError } from "../envelope";
import {
  cargoAdapter,
  cargoDiagnosticExtractor,
  nextestAdapter,
  orderDiagnostics,
  summarizeDiagnostics,
} from "./run-cargo";
import { hasToken, programBasename, tokensOf, trimTrace } from "./run-parse";

// ---------------------------------------------------------------------------
// Input and dependency surface

export interface RunInput {
  /** The declared command key from the `[commands]` table, e.g. "test". */
  readonly command: string;
  /** Extra argv appended to the declared line, e.g. ["--filter", "auth"]. */
  readonly args?: readonly string[];
  /** Wall-clock limit in seconds. Default 300 (§9.6), clamped to [1, 3600]. */
  readonly timeout?: number;
  readonly budget?: number;
}

/** What one execution produced — the executor's whole answer. */
export interface RunExecution {
  /** Process exit code; null when the timeout killed it. */
  readonly exitCode: number | null;
  readonly stdout: string;
  readonly stderr: string;
  readonly timedOut: boolean;
  readonly durationMs: number;
}

/**
 * The composed execution seam: one argv, no shell, hard wall-clock cap.
 * The real implementation (main.ts) is Bun.spawn; tests inject scripts.
 */
export type RunExecutor = (
  argv: readonly string[],
  opts: { readonly timeoutMs: number },
) => Promise<RunExecution>;

export interface RunDeps {
  /** The merged `[commands]` allowlist (ResolvedConfig.commands, §7.2). */
  readonly commands: Readonly<Record<string, string>>;
  readonly execute: RunExecutor;
}

// ---------------------------------------------------------------------------
// wit/runner.wit vocabulary, mirrored verbatim (§6.4 re-export discipline)

/** wit: run-output — what `run` captured from one declared-command execution. */
export interface RunOutput {
  readonly command: string;
  readonly exitCode: number;
  readonly stdout: string;
  readonly stderr: string;
}

/** wit: structured-failure — §9.6's exact quartet. */
export interface StructuredFailure {
  readonly testName: string;
  /** Workspace-relative path when the runner's output names one. */
  readonly file?: string;
  /** The assertion or error message, verbatim from the runner. */
  readonly assertion: string;
  /** The trace, already trimmed to the load-bearing frames. */
  readonly trace: string;
}

/** wit: run-report. */
export interface RunReport {
  readonly failures: readonly StructuredFailure[];
  /** Optional one-line runner summary, e.g. "3 failed, 41 passed". */
  readonly summary?: string;
}

/** wit: the runner interface — can-parse / parse-output, first claimant wins.
 * Methods may answer promises: a component-lane adapter (chunk:m5-runner-
 * adapter) resolves through the engine child, while the bundled TS adapters
 * stay synchronous. */
export interface RunnerAdapter {
  readonly name: string;
  canParse(commandLine: string): boolean | Promise<boolean>;
  /** undefined = could not extract structure; the host falls back honestly. */
  parseOutput(output: RunOutput): RunReport | undefined | Promise<RunReport | undefined>;
}

// ---------------------------------------------------------------------------
// Result rows

/** A run whose output an adapter structured. */
export interface RunReportRow {
  readonly kind: "report";
  readonly command: string;
  readonly resolvedLine: string;
  readonly adapter: string;
  readonly exitCode: number | null;
  readonly timedOut: boolean;
  readonly durationMs: number;
  readonly failures: readonly StructuredFailure[];
  readonly summary?: string;
}

/** The honest fallback: head+tail of the raw streams, cut on the envelope
 * truncation manifest — never a silent guess at structure. */
export interface RunOutputRow {
  readonly kind: "output";
  readonly command: string;
  readonly resolvedLine: string;
  readonly exitCode: number | null;
  readonly timedOut: boolean;
  readonly durationMs: number;
  readonly stdoutHead: readonly string[];
  readonly stdoutTail: readonly string[];
  readonly stderrHead: readonly string[];
  readonly stderrTail: readonly string[];
}

/** One compiler or lint diagnostic (specs/013 R1): a tool-layer row, never a
 * runner report — the public workbench:runner@0.1.0 record is frozen. */
export interface Diagnostic {
  readonly level: "error" | "warning";
  /** E0308 | unused_variables | clippy::needless_return, when the tool named one. */
  readonly code?: string;
  /** The diagnostic's own message, verbatim. */
  readonly message: string;
  /** Workspace-relative, forward slashes (the primary span). */
  readonly file?: string;
  readonly line?: number;
  readonly column?: number;
  /** The rendered snippet plus help/note lines, capped. */
  readonly detail: string;
}

/** A run whose output a diagnostic extractor structured: a build, check or
 * lint, or a test run that failed to compile. Errors first, then warnings;
 * trimmed to the budget with a truncation manifest on any cut. */
export interface RunDiagnosticsRow {
  readonly kind: "diagnostics";
  readonly command: string;
  readonly resolvedLine: string;
  readonly extractor: string;
  readonly exitCode: number | null;
  readonly timedOut: false;
  readonly durationMs: number;
  readonly diagnostics: readonly Diagnostic[];
  /** Counts over the WHOLE run, before any budget cut: "2 errors, 1 warning". */
  readonly summary: string;
}

export type RunRow = RunReportRow | RunOutputRow | RunDiagnosticsRow;

/** The host-side diagnostics seam (specs/013 R1), consulted before the
 * runner adapters: the first claimant decides; `undefined` from
 * `extract` means "no diagnostics to report here" and the runner adapters
 * get their turn. Host-side by construction — nothing here is a plugin
 * surface. */
export interface DiagnosticExtractor {
  readonly name: string;
  canExtract(commandLine: string): boolean;
  extract(output: RunOutput): readonly Diagnostic[] | undefined;
}

// ---------------------------------------------------------------------------
// The no-shell guard (§1.1, constitution IV)

/** Tokens that only mean something to a shell — run never provides one. */
const SHELL_OPERATORS = new Set([
  "|",
  "||",
  "&&",
  ";",
  "&",
  ">",
  ">>",
  "<",
  "<<",
  "2>",
  "2>>",
  "2>&1",
  "1>&2",
  "$(",
  "`",
]);

/** Text/file munging programs: their job belongs to search/read/edit. */
const MUNGING_PROGRAMS = new Set([
  "sed",
  "awk",
  "gawk",
  "mawk",
  "grep",
  "egrep",
  "fgrep",
  "rg",
  "ag",
  "ack",
  "cat",
  "head",
  "tail",
  "cut",
  "tr",
  "sort",
  "uniq",
  "perl",
  "patch",
  "ed",
  "ex",
  "sd",
  "find",
  "fd",
]);

/** Shells: a declared shell IS general shell execution, refused outright. */
const SHELL_PROGRAMS = new Set([
  "sh",
  "bash",
  "zsh",
  "dash",
  "fish",
  "ksh",
  "csh",
  "tcsh",
  "cmd",
  "command",
  "powershell",
  "pwsh",
]);

/** Wrappers that execute their argument: the NEXT token is a program too. */
const PASSTHROUGH_PROGRAMS = new Set(["env", "xargs", "npx", "bunx", "exec", "nice", "time"]);

export interface RunGuardVerdict {
  readonly rejected: true;
  readonly offender: string;
  readonly why: "shell-operator" | "shell-program" | "munging-program";
}

/**
 * Scan the resolved command line plus the extra args as one token stream.
 * A token is in PROGRAM POSITION at index 0, after a shell operator, or
 * after a passthrough wrapper. Shell operators anywhere are rejected (argv
 * execution would pass them as literal arguments — the line needs a shell
 * run never provides); shells and munging programs in program position are
 * rejected with the search/edit redirect.
 */
export function guardCommandLine(
  resolvedLine: string,
  extraArgs: readonly string[],
): RunGuardVerdict | undefined {
  const tokens = [...resolvedLine.split(/\s+/).filter((t) => t.length > 0), ...extraArgs];
  let programPosition = true;
  for (const token of tokens) {
    if (SHELL_OPERATORS.has(token)) {
      return { rejected: true, offender: token, why: "shell-operator" };
    }
    if (programPosition) {
      // Env assignments (FOO=1) and flags (-n1) before the program keep the
      // NEXT token in program position — `env FOO=1 bash` is still bash.
      if (/^[A-Za-z_][A-Za-z0-9_]*=/.test(token) || token.startsWith("-")) continue;
      const program = programBasename(token);
      if (SHELL_PROGRAMS.has(program)) {
        return { rejected: true, offender: token, why: "shell-program" };
      }
      if (MUNGING_PROGRAMS.has(program)) {
        return { rejected: true, offender: token, why: "munging-program" };
      }
      programPosition = PASSTHROUGH_PROGRAMS.has(program);
    }
  }
  return undefined;
}

// ---------------------------------------------------------------------------
// Bundled runner adapters (SDD §8.3's first-party six)

const bunAdapter: RunnerAdapter = {
  name: "bun",
  canParse: (line) => hasToken(line, "bun") && hasToken(line, "test"),
  parseOutput(output) {
    const lines = `${output.stdout}\n${output.stderr}`.split("\n");
    const failures: StructuredFailure[] = [];
    let summary: string | undefined;
    for (let i = 0; i < lines.length; i++) {
      const line = lines[i] as string;
      const fail = line.match(/^\s*(?:✗|\(fail\))\s+(.+?)(?:\s+\[[\d.]+m?s\])?\s*$/u);
      if (fail !== null) {
        const testName = (fail[1] as string).trim();
        let assertion = "";
        const trace: string[] = [];
        let file: string | undefined;
        for (let j = i + 1; j < Math.min(i + 30, lines.length); j++) {
          const body = lines[j] as string;
          if (/^\s*(?:✗|✓|\(fail\)|\(pass\))\s/u.test(body)) break;
          const err = body.match(/^\s*(?:error|AssertionError)[:\s](.*)$/i);
          if (err !== null && assertion === "") assertion = (err[1] as string).trim();
          const at = body.match(/\bat\s+(?:.*\()?([^\s()]+?):(\d+)(?::\d+)?\)?\s*$/);
          if (at !== null) {
            file ??= at[1] as string;
            trace.push(body.trim());
          }
        }
        failures.push({
          testName,
          ...(file === undefined ? {} : { file }),
          assertion,
          trace: trimTrace(trace),
        });
      }
      // Bun prints " N pass" and " N fail" on separate lines — join them.
      const sum = line.match(/^\s*(\d+\s+(?:pass|fail)(?:ed)?)\s*$/i);
      if (sum !== null) {
        summary = summary === undefined ? (sum[1] as string) : `${summary}, ${sum[1] as string}`;
      }
    }
    if (failures.length === 0 && output.exitCode !== 0) return undefined;
    return { failures, ...(summary === undefined ? {} : { summary }) };
  },
};

const jestAdapter: RunnerAdapter = {
  name: "jest",
  canParse: (line) => hasToken(line, "jest"),
  parseOutput(output) {
    const lines = `${output.stdout}\n${output.stderr}`.split("\n");
    const failures: StructuredFailure[] = [];
    let summary: string | undefined;
    let lastFailedFile: string | undefined;
    for (let i = 0; i < lines.length; i++) {
      const line = lines[i] as string;
      const failFile = line.match(/^\s*FAIL\s+(\S+)/);
      if (failFile !== null) lastFailedFile = failFile[1] as string;
      const block = line.match(/^\s*●\s+(.+)$/u);
      if (block !== null && !/Test suite failed/i.test(block[1] as string)) {
        const testName = (block[1] as string).trim();
        let assertion = "";
        const trace: string[] = [];
        let file: string | undefined;
        for (let j = i + 1; j < Math.min(i + 40, lines.length); j++) {
          const body = lines[j] as string;
          if (/^\s*●\s/u.test(body)) break;
          if (assertion === "" && /\S/.test(body)) assertion = body.trim();
          const at = body.match(/^\s*at\s+.*?\(?([^\s():]+):(\d+)(?::\d+)?\)?\s*$/);
          if (at !== null) {
            file ??= at[1] as string;
            trace.push(body.trim());
          }
        }
        failures.push({
          testName,
          ...((file ?? lastFailedFile) === undefined
            ? {}
            : { file: (file ?? lastFailedFile) as string }),
          assertion,
          trace: trimTrace(trace),
        });
      }
      if (/^\s*Tests:\s+.*\bfailed\b/.test(line) || /^\s*Tests:\s+\d+\s+passed/.test(line)) {
        summary = line.trim();
      }
    }
    if (failures.length === 0 && output.exitCode !== 0) return undefined;
    return { failures, ...(summary === undefined ? {} : { summary }) };
  },
};

const vitestAdapter: RunnerAdapter = {
  name: "vitest",
  canParse: (line) => hasToken(line, "vitest"),
  parseOutput(output) {
    const lines = `${output.stdout}\n${output.stderr}`.split("\n");
    const failures: StructuredFailure[] = [];
    let summary: string | undefined;
    for (let i = 0; i < lines.length; i++) {
      const line = lines[i] as string;
      const fail = line.match(
        /^\s*(?:❯|×|FAIL)\s+(\S+\.[cm]?[jt]sx?)\s*>\s*(.+?)(?:\s+\d+ms)?\s*$/u,
      );
      if (fail !== null) {
        const file = fail[1] as string;
        const testName = (fail[2] as string).trim();
        let assertion = "";
        const trace: string[] = [];
        for (let j = i + 1; j < Math.min(i + 40, lines.length); j++) {
          const body = lines[j] as string;
          if (/^\s*(?:❯|×|FAIL)\s+\S+\s*>/u.test(body)) break;
          const err = body.match(/^\s*(?:AssertionError|Error|TypeError)[:\s](.*)$/);
          if (err !== null && assertion === "") assertion = (err[1] as string).trim();
          const at = body.match(/^\s*❯\s+.*?([^\s()]+):(\d+)(?::\d+)?\s*$/u);
          if (at !== null) trace.push(body.trim());
        }
        failures.push({ testName, file, assertion, trace: trimTrace(trace) });
      }
      if (/^\s*Tests\s+.*\bfailed\b/u.test(line) || /^\s*Tests\s+\d+\s+passed/u.test(line)) {
        summary = line.trim().replace(/\s+/g, " ");
      }
    }
    if (failures.length === 0 && output.exitCode !== 0) return undefined;
    return { failures, ...(summary === undefined ? {} : { summary }) };
  },
};

const pytestAdapter: RunnerAdapter = {
  name: "pytest",
  canParse: (line) =>
    hasToken(line, "pytest") ||
    hasToken(line, "py.test") ||
    (tokensOf(line).some((t) => t.startsWith("python")) && /\s-m\s+pytest\b/.test(line)),
  parseOutput(output) {
    const lines = `${output.stdout}\n${output.stderr}`.split("\n");
    const failures: StructuredFailure[] = [];
    let summary: string | undefined;
    const traces = new Map<string, string>();
    // Long-form blocks: ___ test_name ___ ... E   assert ...
    for (let i = 0; i < lines.length; i++) {
      const header = (lines[i] as string).match(/^_{3,}\s+(.+?)\s+_{3,}$/);
      if (header === null) continue;
      const name = (header[1] as string).trim();
      const block: string[] = [];
      for (let j = i + 1; j < lines.length; j++) {
        const body = lines[j] as string;
        if (/^(?:_{3,}|={3,})/.test(body)) break;
        if (/^E\s/.test(body) || /^\S+\.py:\d+/.test(body)) block.push(body.trim());
      }
      traces.set(name, trimTrace(block));
    }
    for (const line of lines) {
      const failed = line.match(/^FAILED\s+(\S+?)::(\S+?)(?:\s+-\s+(.*))?$/);
      if (failed !== null) {
        const file = failed[1] as string;
        const testName = failed[2] as string;
        const shortName = testName.split("[")[0] as string;
        failures.push({
          testName: `${file}::${testName}`,
          file,
          assertion: ((failed[3] as string | undefined) ?? "").trim(),
          trace: traces.get(testName) ?? traces.get(shortName) ?? "",
        });
      }
      const sum = line.match(/^=+\s+(.*\bfailed\b.*|.*\bpassed\b.*)\s+=+$/);
      if (sum !== null) summary = (sum[1] as string).trim();
    }
    if (failures.length === 0 && output.exitCode !== 0) return undefined;
    return { failures, ...(summary === undefined ? {} : { summary }) };
  },
};

const goAdapter: RunnerAdapter = {
  name: "go",
  canParse: (line) => hasToken(line, "go") && hasToken(line, "test"),
  parseOutput(output) {
    const lines = `${output.stdout}\n${output.stderr}`.split("\n");
    const failures: StructuredFailure[] = [];
    let summary: string | undefined;
    for (let i = 0; i < lines.length; i++) {
      const fail = (lines[i] as string).match(/^(?:\s*)--- FAIL:\s+(\S+)\s+\(([\d.]+s)\)/);
      if (fail !== null) {
        const testName = fail[1] as string;
        let assertion = "";
        let file: string | undefined;
        const trace: string[] = [];
        for (let j = i + 1; j < Math.min(i + 30, lines.length); j++) {
          const body = lines[j] as string;
          if (/^(?:\s*)--- (?:FAIL|PASS):/.test(body) || /^(?:FAIL|PASS|ok)\b/.test(body)) break;
          const at = body.match(/^\s+([^\s:]+\.go):(\d+):\s*(.*)$/);
          if (at !== null) {
            file ??= at[1] as string;
            if (assertion === "") assertion = (at[3] as string).trim();
            trace.push(body.trim());
          }
        }
        failures.push({
          testName,
          ...(file === undefined ? {} : { file }),
          assertion,
          trace: trimTrace(trace),
        });
      }
      const sum = (lines[i] as string).match(/^(?:FAIL|ok)\s+\S+\s+[\d.]+s/);
      if (sum !== null) summary = (lines[i] as string).trim();
    }
    if (failures.length === 0 && output.exitCode !== 0) return undefined;
    return { failures, ...(summary === undefined ? {} : { summary }) };
  },
};

/** Trust order: bundled first-party adapters, consulted first to last. */
export const BUNDLED_RUNNER_ADAPTERS: readonly RunnerAdapter[] = [
  bunAdapter,
  vitestAdapter,
  jestAdapter,
  pytestAdapter,
  nextestAdapter,
  cargoAdapter,
  goAdapter,
];

/** Trust order for diagnostic extractors: bundled first-party only. */
export const BUNDLED_DIAGNOSTIC_EXTRACTORS: readonly DiagnosticExtractor[] = [
  cargoDiagnosticExtractor,
];

// ---------------------------------------------------------------------------
// Head+tail fallback (honest cut, manifest attached by construction)

interface HeadTail {
  readonly head: readonly string[];
  readonly tail: readonly string[];
  readonly dropped: number;
}

function headTail(text: string, headLines: number, tailLines: number): HeadTail {
  const lines = text.replace(/\r\n/g, "\n").split("\n");
  while (lines.length > 0 && (lines[lines.length - 1] as string).length === 0) lines.pop();
  if (lines.length <= headLines + tailLines) {
    return { head: lines, tail: [], dropped: 0 };
  }
  return {
    head: lines.slice(0, headLines),
    tail: lines.slice(lines.length - tailLines),
    dropped: lines.length - headLines - tailLines,
  };
}

/** Keep as many ordered diagnostics as the budget allows — at least one when
 * there are any — measuring the row the way every run row is measured. */
function fitDiagnostics(
  base: RunDiagnosticsRow,
  ordered: readonly Diagnostic[],
  budget: number,
): { readonly row: RunDiagnosticsRow; readonly dropped: number } {
  const limit = budget * CHARS_PER_TOKEN;
  let size = JSON.stringify(base).length;
  let kept = 0;
  for (const d of ordered) {
    const next = size + JSON.stringify(d).length + 1;
    if (next > limit && kept > 0) break;
    size = next;
    kept++;
  }
  return {
    row: { ...base, diagnostics: ordered.slice(0, kept) },
    dropped: ordered.length - kept,
  };
}

// ---------------------------------------------------------------------------
// The tool

export const RUN_TIMEOUT_DEFAULT_S = 300;
export const RUN_TIMEOUT_MAX_S = 3600;
const RUN_BUDGET_DEFAULT = 4000;
const CHARS_PER_TOKEN = 4;
/** Fallback line allocation: most of the budget to stdout, a slice to stderr. */
const STDERR_SHARE = 0.25;

function redirectMessage(verdict: RunGuardVerdict, command: string): string {
  const redirect =
    "use `search` (or `read`) to inspect files and `edit` to modify them — " +
    "run executes declared project commands only (SDD §9.6, §1.1)";
  switch (verdict.why) {
    case "shell-operator":
      return (
        `run executes a single argv with no shell: ${JSON.stringify(verdict.offender)} in ` +
        `command "${command}" would be passed as a literal argument, never interpreted; ${redirect}`
      );
    case "shell-program":
      return (
        `command "${command}" resolves to a shell (${JSON.stringify(verdict.offender)}), ` +
        `which is general shell execution; ${redirect}`
      );
    case "munging-program":
      return `command "${command}" is text/file munging via ${JSON.stringify(verdict.offender)}; ${redirect}`;
  }
}

/**
 * Execute one declared command. Throws RangeError on schema-level misuse
 * (like every tool); everything an agent can act on comes back typed in the
 * envelope. `adapters` defaults to the bundled six; the M5 plugin lane will
 * prepend installed runner-adapter plugins in trust order.
 */
export async function runRun(
  deps: RunDeps,
  input: RunInput,
  v: number,
  adapters: readonly RunnerAdapter[] = BUNDLED_RUNNER_ADAPTERS,
  extractors: readonly DiagnosticExtractor[] = BUNDLED_DIAGNOSTIC_EXTRACTORS,
): Promise<ResponseEnvelope<RunRow>> {
  if (typeof input.command !== "string" || input.command.length === 0) {
    throw new RangeError("run: command must be a non-empty string");
  }
  const extraArgs = input.args ?? [];
  if (!Array.isArray(extraArgs) || extraArgs.some((a) => typeof a !== "string")) {
    throw new RangeError("run: args must be an array of strings");
  }
  const timeoutS = input.timeout ?? RUN_TIMEOUT_DEFAULT_S;
  if (!Number.isInteger(timeoutS) || timeoutS < 1 || timeoutS > RUN_TIMEOUT_MAX_S) {
    throw new RangeError(`run: timeout must be an integer in [1, ${RUN_TIMEOUT_MAX_S}] seconds`);
  }
  const budget = input.budget ?? RUN_BUDGET_DEFAULT;
  if (!Number.isInteger(budget) || budget < 1) {
    throw new RangeError("run: budget must be a positive integer");
  }

  const resolvedLine = deps.commands[input.command];
  if (resolvedLine === undefined) {
    const declared = Object.keys(deps.commands).sort();
    return taxonomyError({
      v,
      code: "COMMAND_NOT_DECLARED",
      message:
        `command ${JSON.stringify(input.command)} is not in the merged [commands] table. ` +
        `Declared: ${declared.length === 0 ? "(none)" : declared.join(", ")}. ` +
        "A human can declare it in workbench.toml [commands] (or the user/org config layer, " +
        "SDD §7.2) — the server cannot amend its own allowlist.",
    });
  }

  const verdict = guardCommandLine(resolvedLine, extraArgs);
  if (verdict !== undefined) {
    return taxonomyError({
      v,
      code: "RUN_REDIRECTED",
      message: redirectMessage(verdict, input.command),
    });
  }

  const argv = [...resolvedLine.split(/\s+/).filter((t) => t.length > 0), ...extraArgs];
  const execution = await deps.execute(argv, { timeoutMs: timeoutS * 1000 });

  // Structured extraction: diagnostics first (a build, a lint, or a test run
  // that never compiled), then the runner adapters — first claimant in trust
  // order decides; a claimant that cannot extract structure falls through to
  // the honest cut.
  if (!execution.timedOut && execution.exitCode !== null) {
    const output: RunOutput = {
      command: resolvedLine,
      exitCode: execution.exitCode,
      stdout: execution.stdout,
      stderr: execution.stderr,
    };
    for (const extractor of extractors) {
      if (!extractor.canExtract(resolvedLine)) continue;
      const diagnostics = extractor.extract(output);
      if (diagnostics === undefined) break;
      const fitted = fitDiagnostics(
        {
          kind: "diagnostics",
          command: input.command,
          resolvedLine,
          extractor: extractor.name,
          exitCode: execution.exitCode,
          timedOut: false,
          durationMs: execution.durationMs,
          diagnostics: [],
          summary: summarizeDiagnostics(diagnostics),
        },
        orderDiagnostics(diagnostics),
        budget,
      );
      const truncation: Truncation | undefined =
        fitted.dropped === 0
          ? undefined
          : {
              dropped: fitted.dropped,
              reason: "budget",
              narrow: [
                "raise budget to keep more diagnostics",
                "fix the errors first — warnings were dropped first, then the latest errors",
              ],
              cursor: `diagnostic:${fitted.row.diagnostics.length}`,
            };
      return okEnvelope<RunRow>({
        v,
        freshness: "warm",
        budget: {
          requested: budget,
          used: Math.ceil(JSON.stringify(fitted.row).length / CHARS_PER_TOKEN),
        },
        results: truncation === undefined ? [fitted.row] : { kept: [fitted.row], truncation },
      });
    }
    for (const adapter of adapters) {
      if (!(await adapter.canParse(resolvedLine))) continue;
      const report = await adapter.parseOutput(output);
      if (report === undefined) break;
      const row: RunReportRow = {
        kind: "report",
        command: input.command,
        resolvedLine,
        adapter: adapter.name,
        exitCode: execution.exitCode,
        timedOut: false,
        durationMs: execution.durationMs,
        failures: report.failures,
        ...(report.summary === undefined ? {} : { summary: report.summary }),
      };
      return okEnvelope<RunRow>({
        v,
        freshness: "warm",
        budget: {
          requested: budget,
          used: Math.ceil(JSON.stringify(row).length / CHARS_PER_TOKEN),
        },
        results: [row],
      });
    }
  }

  // Head+tail fallback with a truncation manifest — never a silent guess.
  const totalLines = Math.max(8, Math.floor((budget * CHARS_PER_TOKEN) / 80));
  const stderrLines = Math.max(4, Math.floor(totalLines * STDERR_SHARE));
  const stdoutLines = Math.max(4, totalLines - stderrLines);
  const out = headTail(execution.stdout, Math.ceil(stdoutLines / 2), Math.floor(stdoutLines / 2));
  const err = headTail(execution.stderr, Math.ceil(stderrLines / 2), Math.floor(stderrLines / 2));
  const row: RunOutputRow = {
    kind: "output",
    command: input.command,
    resolvedLine,
    exitCode: execution.exitCode,
    timedOut: execution.timedOut,
    durationMs: execution.durationMs,
    stdoutHead: out.head,
    stdoutTail: out.tail,
    stderrHead: err.head,
    stderrTail: err.tail,
  };
  const dropped = out.dropped + err.dropped;
  const truncation: Truncation | undefined =
    dropped === 0
      ? undefined
      : {
          dropped,
          reason: "budget",
          narrow: [
            "raise budget to widen the head+tail window",
            "no bundled runner adapter claimed this command — a runner-adapter plugin (§8.3) can add structured extraction",
          ],
          cursor: `line:${out.head.length}`,
        };
  const notes: string[] = [];
  if (execution.timedOut) {
    notes.push(`command timed out after ${timeoutS}s and was killed; output below is partial`);
  }
  return okEnvelope<RunRow>({
    v,
    freshness: "warm",
    budget: { requested: budget, used: Math.ceil(JSON.stringify(row).length / CHARS_PER_TOKEN) },
    results: truncation === undefined ? [row] : { kept: [row], truncation },
    ...(notes.length === 0 ? {} : { notes }),
  });
}
