/**
 * The cargo family of `run` parsers (chunk:rust-cargo-runner-depth,
 * specs/013): the bundled `cargo test` and `cargo nextest` runner adapters
 * on the wit/runner.wit vocabulary, and the host-side diagnostic extractor
 * that lifts rustc/clippy diagnostics — from `--message-format=json` or the
 * rendered form, equal rows either way — into `run`'s `diagnostics` row.
 *
 * Pure functions from captured output to rows; nothing here spawns. Every
 * shape below was captured from real runs (rustc/cargo 1.95, cargo-nextest
 * 0.9) and is pinned by the goldens under test/support/golden/cargo.
 *
 * What is deliberately NOT here: the public workbench:runner@0.1.0 record
 * is frozen at this major (wit/README.md rule 3), so diagnostics never cross
 * the plugin boundary — they are a tool-layer row, not a runner report.
 */

import type {
  Diagnostic,
  DiagnosticExtractor,
  RunnerAdapter,
  RunOutput,
  RunReport,
  StructuredFailure,
} from "./run";
import { hasToken, normalizePath, tokensOf, trimTrace } from "./run-parse";

// ---------------------------------------------------------------------------
// Command-line facts

/** The cargo subcommand of a declared line: `cargo +nightly build …` → build. */
export function cargoSubcommand(commandLine: string): string | undefined {
  const tokens = tokensOf(commandLine);
  const at = tokens.indexOf("cargo");
  if (at < 0) return undefined;
  for (const token of tokens.slice(at + 1)) {
    if (token.startsWith("+") || token.startsWith("-")) continue;
    return token;
  }
  return undefined;
}

const usesJsonMessages = (commandLine: string): boolean =>
  /--message-format(?:=|\s+)json/.test(commandLine);

const splitLines = (text: string): string[] => text.replace(/\r\n/g, "\n").split("\n");

// ---------------------------------------------------------------------------
// The panic block: shared by cargo test and nextest

interface PanicDetails {
  readonly file?: string;
  readonly assertion: string;
  readonly trace: string;
}

/** rustc ≥ 1.73: `panicked at <path>:<line>:<col>:` with the message on the
 * same line or the next; older: `panicked at '<message>', <path>:<line>:<col>`. */
const PANIC_NEW = /panicked at (.+?):(\d+):(\d+):?\s*(.*)$/;
const PANIC_OLD = /panicked at '(.*)', (.+?):(\d+):(\d+)\s*$/;

const isNoise = (line: string): boolean =>
  /^note: (?:run with `RUST_BACKTRACE|Some details are omitted)/.test(line) ||
  line === "stack backtrace:";

/** Frames the workspace did not write: the standard library and toolchains. */
const isForeignFrame = (path: string): boolean =>
  path.startsWith("/rustc/") ||
  /[\\/]\.rustup[\\/]|[\\/]toolchains[\\/]|[\\/]\.cargo[\\/]registry[\\/]|[\\/]\.cargo[\\/]git[\\/]/.test(
    path,
  );

/** Keep only the frames whose source lives in the workspace; each kept frame
 * is one `symbol at path:line` line. */
export function trimBacktrace(lines: readonly string[]): string[] {
  const kept: string[] = [];
  let pendingSymbol: string | undefined;
  for (const raw of lines) {
    const line = raw.trim();
    const frame = line.match(/^(\d+): (.+)$/);
    if (frame !== null) {
      pendingSymbol = frame[2] as string;
      continue;
    }
    const at = line.match(/^at (.+?)(?::(\d+))?(?::(\d+))?$/);
    if (at !== null && pendingSymbol !== undefined) {
      const path = at[1] as string;
      if (!isForeignFrame(path)) {
        const loc = at[2] === undefined ? path : `${path}:${at[2]}`;
        kept.push(`${pendingSymbol} at ${normalizePath(loc)}`);
      }
      pendingSymbol = undefined;
    }
  }
  return kept;
}

/** Extract the quartet's file/assertion/trace from one test's output block
 * (already dedented). Without a panic line the assertion is empty and the
 * trace is the trimmed block — the pre-013 behaviour. */
export function parsePanicBlock(block: readonly string[]): PanicDetails {
  const lines = block.map((l) => l.trimEnd());
  for (let i = 0; i < lines.length; i++) {
    const line = (lines[i] as string).trim();
    let file: string;
    let lineNo: string;
    let col: string;
    let message: string;
    // The legacy shape first: its quoted message would otherwise be read as
    // part of the modern shape's path.
    const legacy = line.match(PANIC_OLD);
    const modern = legacy === null ? line.match(PANIC_NEW) : null;
    if (modern !== null) {
      [file, lineNo, col, message] = [
        modern[1] as string,
        modern[2] as string,
        modern[3] as string,
        (modern[4] as string).trim(),
      ];
    } else if (legacy !== null) {
      [message, file, lineNo, col] = [
        legacy[1] as string,
        legacy[2] as string,
        legacy[3] as string,
        legacy[4] as string,
      ];
    } else {
      continue;
    }
    const continuation: string[] = [];
    let j = i + 1;
    // The message follows on the next non-empty line when the panic line
    // ended with the colon (rustc ≥ 1.73).
    if (message === "") {
      while (j < lines.length && (lines[j] as string).trim() === "") j++;
      const next = (lines[j] as string | undefined)?.trim() ?? "";
      if (next !== "" && !isNoise(next)) {
        message = next;
        j++;
      }
    }
    // Continuation: `left: …` / `right: …` and similar lines until a blank,
    // a note, or the backtrace.
    while (j < lines.length) {
      const next = (lines[j] as string).trim();
      if (next === "" || isNoise(next) || next === "stack backtrace:") break;
      continuation.push(next);
      j++;
    }
    const backtraceAt = lines.findIndex((l) => l.trim() === "stack backtrace:");
    const frames = backtraceAt < 0 ? [] : trimBacktrace(lines.slice(backtraceAt + 1));
    const normalized = normalizePath(file);
    return {
      file: normalized,
      assertion: message,
      trace: trimTrace([`${normalized}:${lineNo}:${col}`, ...continuation, ...frames]),
    };
  }
  return { assertion: "", trace: trimTrace(lines.filter((l) => !isNoise(l.trim()))) };
}

// ---------------------------------------------------------------------------
// cargo test (libtest output)

const RUNNING_BINARY = /^\s*Running .*[\\/]deps[\\/]([A-Za-z0-9_.-]+?)-[0-9a-f]+(?:\.exe)?\)\s*$/;
const DOC_TESTS = /^\s*Doc-tests (\S+)\s*$/;
/** libtest's summary line; `ignored` onward is optional so a truncated
 * line still counts. */
const TEST_RESULT = /^test result: (\w+)\. (\d+) passed; (\d+) failed(?:; (\d+) ignored)?/;

interface CargoTestFailure {
  readonly binary: number;
  readonly name: string;
}

/**
 * cargo prints its `Running <binary>` / `Doc-tests <crate>` status lines on
 * STDERR and libtest's output on STDOUT, so when `run` captures the streams
 * separately the two never interleave. Attribution therefore pairs them by
 * ORDER: every `running N tests` header on stdout opens the next binary's
 * section, and the i-th section belongs to the i-th status line. The same
 * pairing holds for a `2>&1` capture, where they do interleave.
 */
function parseCargoTest(output: RunOutput): RunReport | undefined {
  const lines = splitLines(`${output.stdout}\n${output.stderr}`);
  const binaries: string[] = [];
  const failed: CargoTestFailure[] = [];
  const details = new Map<string, PanicDetails>();
  const results: string[] = [];
  const hints: string[] = [];
  let totals = { passed: 0, failed: 0, ignored: 0 };
  let section = -1;

  const current = (): number => Math.max(0, section);
  for (let i = 0; i < lines.length; i++) {
    const line = lines[i] as string;
    const running = line.match(RUNNING_BINARY);
    if (running !== null) {
      binaries.push(running[1] as string);
      continue;
    }
    const doc = line.match(DOC_TESTS);
    if (doc !== null) {
      binaries.push(`${doc[1] as string} (doctest)`);
      continue;
    }
    if (/^running \d+ tests?$/.test(line.trim())) {
      section++;
      continue;
    }
    const failedLine = line.match(/^test\s+(.+?)\s+\.\.\.\s+FAILED$/);
    if (failedLine !== null) {
      failed.push({ binary: current(), name: (failedLine[1] as string).trim() });
      continue;
    }
    const header = line.match(/^----\s+(.+?)\s+stdout\s+----$/);
    if (header !== null) {
      const block: string[] = [];
      for (let j = i + 1; j < lines.length; j++) {
        const body = lines[j] as string;
        if (/^----\s+.+?\s+stdout\s+----$/.test(body) || /^failures:$/.test(body)) break;
        block.push(body);
      }
      details.set(`${current()} ${(header[1] as string).trim()}`, parsePanicBlock(block));
      continue;
    }
    const result = line.match(TEST_RESULT);
    if (result !== null) {
      results.push(line.trim());
      totals = {
        passed: totals.passed + Number(result[2]),
        failed: totals.failed + Number(result[3]),
        ignored: totals.ignored + Number(result[4] ?? 0),
      };
      continue;
    }
    const hint = line.match(/to rerun pass `([^`]+)`/);
    if (hint !== null) hints.push(hint[1] as string);
  }

  const several = section > 0;
  const failures: StructuredFailure[] = failed.map((f) => {
    const d = details.get(`${f.binary} ${f.name}`);
    // A section without a status line (a binary that died before cargo's
    // line was captured) is named by its position rather than guessed.
    const stem = binaries[f.binary] ?? `binary ${f.binary + 1}`;
    const testName = several ? `${stem} ${f.name}` : f.name;
    return {
      testName,
      ...(d?.file === undefined ? {} : { file: d.file }),
      assertion: d?.assertion ?? "",
      trace: d?.trace ?? "",
    };
  });

  let summary: string | undefined;
  if (results.length === 1) {
    summary = results[0];
  } else if (results.length > 1) {
    summary =
      `${totals.passed} passed; ${totals.failed} failed; ${totals.ignored} ignored ` +
      `across ${results.length} binaries`;
    if (hints.length > 0) summary += ` — rerun: ${hints.join(", ")}`;
  }
  if (failures.length === 0 && output.exitCode !== 0) return undefined;
  return { failures, ...(summary === undefined ? {} : { summary }) };
}

export const cargoAdapter: RunnerAdapter = {
  name: "cargo",
  canParse: (line) => hasToken(line, "cargo") && cargoSubcommand(line) === "test",
  parseOutput: parseCargoTest,
};

// ---------------------------------------------------------------------------
// cargo nextest

/** A test's final status line: `FAIL [ 0.008s] (1/6) alpha tests::x`.
 * `TRY n` lines are retries; the final status follows and is the one kept. */
const NEXTEST_STATUS =
  /^\s*(?:(TRY \d+) )?(PASS|FAIL|SLOW|LEAK|FLAKY|TIMEOUT|SIGABRT|SIGSEGV|SIGKILL|SIGILL|SIGBUS|ABORT)\s+\[\s*[\d.]+s\]\s+(?:\(\d+\/\d+\)\s+)?(\S+)\s+(\S+)\s*$/;
const NEXTEST_FAILING = new Set([
  "FAIL",
  "TIMEOUT",
  "SIGABRT",
  "SIGSEGV",
  "SIGKILL",
  "SIGILL",
  "SIGBUS",
  "ABORT",
]);
const NEXTEST_BOUNDARY =
  /^\s*(?:TRY \d+ )?(?:PASS|FAIL|SLOW|LEAK|FLAKY|TIMEOUT|SIG[A-Z]+|ABORT)\s+\[|^\s*Summary \[|^\s*Cancelling |^\s*Starting \d+ test|^─+$/;

function parseNextest(output: RunOutput): RunReport | undefined {
  const lines = splitLines(`${output.stdout}\n${output.stderr}`);
  const failures: StructuredFailure[] = [];
  const seen = new Set<string>();
  let summary: string | undefined;
  let cancelled: string | undefined;
  let inRepeatList = false;
  for (let i = 0; i < lines.length; i++) {
    const line = lines[i] as string;
    const summaryLine = line.match(/^\s*Summary \[.*$/);
    if (summaryLine !== null) {
      summary = line.trim().replace(/\s+/g, " ");
      inRepeatList = true;
      continue;
    }
    const cancel = line.match(/^\s*Cancelling due to (.+?): (\d+) tests? still running/);
    if (cancel !== null) {
      cancelled = `${cancel[2] as string} still running`;
      continue;
    }
    if (inRepeatList) continue;
    const status = line.match(NEXTEST_STATUS);
    if (status === null || status[1] !== undefined) continue; // TRY lines: wait for the final status
    const verdict = status[2] as string;
    if (!NEXTEST_FAILING.has(verdict)) continue;
    const testName = `${status[3] as string} ${status[4] as string}`;
    if (seen.has(testName)) continue;
    seen.add(testName);
    const block: string[] = [];
    for (let j = i + 1; j < lines.length; j++) {
      const body = lines[j] as string;
      if (NEXTEST_BOUNDARY.test(body)) break;
      if (/^\s*(?:stdout|stderr) ─+\s*$/.test(body)) continue;
      block.push(body.replace(/^ {4}/, ""));
    }
    const d = parsePanicBlock(block);
    failures.push({
      testName,
      ...(d.file === undefined ? {} : { file: d.file }),
      assertion: d.assertion === "" ? verdict : d.assertion,
      trace: d.trace,
    });
  }
  if (summary !== undefined && cancelled !== undefined) summary += ` (cancelled: ${cancelled})`;
  if (failures.length === 0 && output.exitCode !== 0) return undefined;
  return { failures, ...(summary === undefined ? {} : { summary }) };
}

export const nextestAdapter: RunnerAdapter = {
  name: "nextest",
  canParse: (line) => hasToken(line, "cargo") && hasToken(line, "nextest"),
  parseOutput: parseNextest,
};

// ---------------------------------------------------------------------------
// Diagnostics: rustc / clippy, JSON or rendered

const DETAIL_MAX_LINES = 12;

/** The rendered text minus its header and `-->` lines: the snippet plus
 * help/note lines, capped. Shared by both modes so they stay equal. */
function detailOf(lines: readonly string[]): string {
  const body = lines.map((l) => l.trimEnd());
  let start = 0;
  if (body.length > 0 && /^\s*--> /.test(body[0] as string)) start = 1;
  const kept = body.slice(start);
  while (kept.length > 0 && (kept[kept.length - 1] as string).trim() === "") kept.pop();
  return kept.slice(0, DETAIL_MAX_LINES).join("\n");
}

const LINT_ATTRIBUTE = /`?#\[(?:warn|deny|forbid|expect)\(([A-Za-z0-9_:]+)\)\]/;

/** Lines cargo prints ABOUT a build rather than diagnostics of it. */
const isCargoSummary = (message: string): boolean =>
  /^could not compile\b/.test(message) ||
  /^aborting due to\b/.test(message) ||
  /^build failed\b/.test(message) ||
  /generated \d+ warnings?\b/.test(message) ||
  /^test failed\b/.test(message) ||
  /^\d+ targets? failed\b/.test(message) ||
  /^test run failed\b/.test(message);

interface RawSpan {
  readonly file_name?: string;
  readonly line_start?: number;
  readonly column_start?: number;
  readonly is_primary?: boolean;
}

interface RawMessage {
  readonly level?: string;
  readonly code?: { readonly code?: string } | null;
  readonly message?: string;
  readonly spans?: readonly RawSpan[];
  readonly rendered?: string | null;
  readonly children?: readonly { readonly level?: string; readonly message?: string }[];
}

function parseJsonDiagnostics(stdout: string): Diagnostic[] {
  const out: Diagnostic[] = [];
  for (const raw of splitLines(stdout)) {
    const line = raw.trim();
    if (!line.startsWith("{")) continue;
    let parsed: { reason?: string; message?: RawMessage };
    try {
      parsed = JSON.parse(line) as { reason?: string; message?: RawMessage };
    } catch {
      continue;
    }
    if (parsed.reason !== "compiler-message" || parsed.message === undefined) continue;
    const msg = parsed.message;
    if (msg.level !== "error" && msg.level !== "warning") continue;
    const spans = msg.spans ?? [];
    const span = spans.find((s) => s.is_primary === true) ?? spans[0];
    const rendered = msg.rendered ?? null;
    const detail =
      rendered === null
        ? (msg.children ?? [])
            .filter((c) => c.message !== undefined)
            .map((c) => `${c.level ?? "note"}: ${c.message as string}`)
            .join("\n")
        : detailOf(splitLines(rendered).slice(1));
    const code = msg.code?.code;
    out.push({
      level: msg.level,
      ...(code === undefined || code === null ? {} : { code }),
      message: msg.message ?? "",
      ...(span?.file_name === undefined ? {} : { file: normalizePath(span.file_name) }),
      ...(span?.line_start === undefined ? {} : { line: span.line_start }),
      ...(span?.column_start === undefined ? {} : { column: span.column_start }),
      detail,
    });
  }
  return out;
}

const RENDERED_HEADER = /^(error|warning)(?:\[([A-Za-z0-9_:]+)\])?: (.+)$/;

function parseRenderedDiagnostics(text: string): Diagnostic[] {
  const lines = splitLines(text);
  const out: Diagnostic[] = [];
  for (let i = 0; i < lines.length; i++) {
    const header = (lines[i] as string).match(RENDERED_HEADER);
    if (header === null) continue;
    const message = (header[3] as string).trim();
    if (isCargoSummary(message)) continue;
    const block: string[] = [];
    for (let j = i + 1; j < lines.length; j++) {
      const body = lines[j] as string;
      if (body.trim() === "" || RENDERED_HEADER.test(body)) break;
      block.push(body);
    }
    const location = (block[0] ?? "").match(/^\s*--> (.+?):(\d+):(\d+)\s*$/);
    let code = header[2] as string | undefined;
    if (code === undefined) {
      for (const body of block) {
        const attr = body.match(LINT_ATTRIBUTE);
        if (attr !== null) {
          code = attr[1] as string;
          break;
        }
      }
    }
    out.push({
      level: header[1] as "error" | "warning",
      ...(code === undefined ? {} : { code }),
      message,
      ...(location === null ? {} : { file: normalizePath(location[1] as string) }),
      ...(location === null ? {} : { line: Number(location[2]) }),
      ...(location === null ? {} : { column: Number(location[3]) }),
      detail: detailOf(block),
    });
    i += block.length;
  }
  return out;
}

function dedupe(diagnostics: readonly Diagnostic[]): Diagnostic[] {
  const seen = new Set<string>();
  const out: Diagnostic[] = [];
  for (const d of diagnostics) {
    const key = [d.level, d.code ?? "", d.message, d.file ?? "", d.line ?? "", d.column ?? ""].join(
      " ",
    );
    if (seen.has(key)) continue;
    seen.add(key);
    out.push(d);
  }
  return out;
}

/** Errors first (emission order), then warnings. */
export function orderDiagnostics(diagnostics: readonly Diagnostic[]): Diagnostic[] {
  return [
    ...diagnostics.filter((d) => d.level === "error"),
    ...diagnostics.filter((d) => d.level !== "error"),
  ];
}

const TEST_EXECUTION =
  /^\s*running \d+ tests?$|^\s*test .+? \.\.\. |^\s*Starting \d+ tests? across|^\s*(?:PASS|FAIL) \[/m;

const DIAGNOSTIC_SUBCOMMANDS = new Set(["build", "check", "clippy", "test", "nextest"]);

/** Extract rustc/clippy diagnostics from a cargo run. JSON mode reads stdout
 * as compiler-message lines and stderr as rendered text (cargo's own errors
 * — a bad manifest, a missing subcommand — are rendered there); rendered
 * mode reads both streams. Test commands are claimed only when the build
 * failed: error-level diagnostics and no test execution at all. */
export function extractCargoDiagnostics(output: RunOutput): Diagnostic[] | undefined {
  const sub = cargoSubcommand(output.command);
  if (sub === undefined || !DIAGNOSTIC_SUBCOMMANDS.has(sub)) return undefined;
  const json = usesJsonMessages(output.command);
  const raw = json
    ? [...parseJsonDiagnostics(output.stdout), ...parseRenderedDiagnostics(output.stderr)]
    : parseRenderedDiagnostics(`${output.stdout}\n${output.stderr}`);
  const diagnostics = orderDiagnostics(dedupe(raw));
  if (sub === "test" || sub === "nextest") {
    const compileFailed =
      diagnostics.some((d) => d.level === "error") &&
      !TEST_EXECUTION.test(`${output.stdout}\n${output.stderr}`);
    return compileFailed ? diagnostics : undefined;
  }
  if (diagnostics.length === 0 && output.exitCode !== 0) return undefined;
  return diagnostics;
}

export const cargoDiagnosticExtractor: DiagnosticExtractor = {
  name: "cargo",
  canExtract: (line) => {
    const sub = cargoSubcommand(line);
    return hasToken(line, "cargo") && sub !== undefined && DIAGNOSTIC_SUBCOMMANDS.has(sub);
  },
  extract: extractCargoDiagnostics,
};

/** "2 errors, 1 warning" | "no diagnostics". */
export function summarizeDiagnostics(diagnostics: readonly Diagnostic[]): string {
  const errors = diagnostics.filter((d) => d.level === "error").length;
  const warnings = diagnostics.length - errors;
  if (diagnostics.length === 0) return "no diagnostics";
  const parts: string[] = [];
  if (errors > 0) parts.push(`${errors} error${errors === 1 ? "" : "s"}`);
  if (warnings > 0) parts.push(`${warnings} warning${warnings === 1 ? "" : "s"}`);
  return parts.join(", ");
}
