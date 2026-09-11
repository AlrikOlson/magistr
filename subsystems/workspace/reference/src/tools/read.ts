/**
 * The `read` tool (SDD §9.3) — replaces cat, head, tail and sed -n. One call
 * returns a unit of meaning: the whole file, an explicit line range, or a
 * symbol resolved via the index — with precise 1-based line anchors and a
 * content hash `edit` can later use as a staleness anchor.
 *
 * Freshness (§7.3.3) is by construction plus verification: content is read
 * live and served only when the file's mtime+size were identical before and
 * after the read (one retry, then the answer is honestly labeled `cold`).
 * Symbol anchors come from the index's own verified outline path, and the
 * resolved start line is additionally checked against the served content —
 * no stale line number leaves the server.
 *
 * Redaction is not this module's job: every envelope passes `sealEnvelope`
 * at the protocol layer (§7.1), the single choke point where secrets become
 * visible `[REDACTED:<kind>]` markers.
 *
 * This layer sees only ports and the envelope contract (import-graph rule);
 * file access is injected by the composing layer, which owns the jail check.
 */

import {
  type ErrorEnvelope,
  type Freshness,
  okEnvelope,
  type ResponseEnvelope,
  taxonomyError,
  truncateResults,
} from "../envelope";
import type { IndexPort, IndexStatus, SymbolEntry } from "../ports";
import { RANKING_WEIGHTS } from "./ranking";

/** Lines per content segment row: line anchors stay cheap to derive and a
 * budget cut lands between segments, never mid-line. */
export const READ_SEGMENT_LINES = 32;

/** How far into the file the surround:"imports" lexical scan looks. */
export const READ_IMPORT_SCAN_LINES = 200;

/** Verified file access the composing layer injects (Tier-0 capable). The
 * jail decision happens before these are built; both operate on the one
 * admitted file, so the tool holds no path authority of its own. */
export interface ReadWorkspaceAccess {
  read(path: string): Promise<string>;
  stat(path: string): Promise<{ readonly mtimeMs: number; readonly size: number }>;
}

export interface ReadDeps {
  readonly index: IndexPort;
  readonly workspace: ReadWorkspaceAccess;
}

/** Wire shape of one tool call, mirroring schemas/tools/read.input.schema.json. */
export interface ReadInput {
  readonly file: string;
  readonly symbol?: string;
  readonly lines?: { readonly start: number; readonly end: number };
  readonly surround?: string;
  readonly budget?: number;
}

export const READ_SURROUNDS = ["none", "enclosing", "imports"] as const;
export type ReadSurround = (typeof READ_SURROUNDS)[number];

interface ReadSymbolRef {
  readonly name: string;
  readonly kind: string;
  readonly line: number;
  readonly endLine?: number;
  /** First line of the item's doc comment, when it has one (specs/009 R6). */
  readonly doc?: string;
}

/** The first result row: what was resolved, where it sits, and the anchor. */
export interface ReadUnit {
  readonly kind: "unit";
  readonly file: string;
  /** 1-based inclusive resolved range; {start:0,end:0} for an empty file. */
  readonly range: { readonly start: number; readonly end: number };
  readonly totalLines: number;
  /** sha256 of the full file content as served — edit's staleness anchor. */
  readonly contentHash: string;
  /** Symbol mode: the resolved symbol. */
  readonly symbol?: ReadSymbolRef;
  /** surround:"enclosing": the parent symbol the range was widened to. */
  readonly enclosing?: ReadSymbolRef;
}

/** One span of content. `text` is `end - start + 1` raw lines joined by \n. */
export interface ReadSegment {
  readonly kind: "lines" | "imports";
  readonly file: string;
  readonly start: number;
  readonly end: number;
  readonly text: string;
}

export type ReadResult = ReadUnit | ReadSegment;

async function indexStatusOrNull(index: IndexPort): Promise<IndexStatus | null> {
  try {
    return await index.status();
  } catch {
    // No index yet: the honest Tier-0 answer.
    return null;
  }
}

const FRESHNESS_RANK: Readonly<Record<Freshness, number>> = { cold: 0, warming: 1, warm: 2 };

/** The envelope may never claim fresher than its least-fresh ingredient. */
function worstFreshness(a: Freshness, b: Freshness): Freshness {
  return FRESHNESS_RANK[a] <= FRESHNESS_RANK[b] ? a : b;
}

/** Normalize the input path to the workspace-relative posix form the index
 * stores, so outline lookups and result rows speak one path dialect. */
function normalizeRelPath(path: string): string {
  return path
    .replaceAll("\\", "/")
    .split("/")
    .filter((p) => p !== "" && p !== ".")
    .join("/");
}

interface Snapshot {
  readonly content: string;
  readonly freshness: Freshness;
  readonly note?: string;
}

/**
 * The §7.3.3 sandwich: stat → read → stat. Identical mtime+size on both
 * sides proves the served content — and every line anchor derived from it —
 * describes the file as it is now (`warm`). A torn read gets one retry;
 * still torn, the last read is served honestly labeled `cold`.
 */
async function readVerified(workspace: ReadWorkspaceAccess, path: string): Promise<Snapshot> {
  let content = "";
  for (let attempt = 0; attempt < 2; attempt++) {
    const before = await workspace.stat(path);
    content = await workspace.read(path);
    const after = await workspace.stat(path);
    if (before.mtimeMs === after.mtimeMs && before.size === after.size) {
      return { content, freshness: "warm" };
    }
  }
  return {
    content,
    freshness: "cold",
    note:
      "the file changed while being read (mtime+size mismatch, retried once); " +
      "content is the most recent read, labeled cold",
  };
}

function isMissingFile(error: unknown): boolean {
  const code = (error as NodeJS.ErrnoException).code;
  return code === "ENOENT" || code === "ENOTDIR" || code === "EISDIR";
}

/** §4.3 FILE_NOT_FOUND (v1.2 amendment): the §7.1 jail admits nonexistent
 * in-root paths by design, so ENOENT surfaces here at the tool layer. */
function fileNotFound(v: number, file: string): ResponseEnvelope<ReadResult> {
  return taxonomyError({
    v,
    code: "FILE_NOT_FOUND",
    message:
      `no such file in the workspace: ${JSON.stringify(file)}; ` +
      "check the path (search mode:files locates files by name)",
  });
}

/** §9.3: symbol addressing strictly requires the index — refuse with progress. */
function indexUnavailable(v: number, status: IndexStatus | null): ResponseEnvelope<ReadResult> {
  const progress =
    status === null
      ? "no index exists yet for this workspace"
      : status.building === true
        ? `index build in progress: ${status.filesIndexed ?? 0}/${status.filesTotal ?? 0} files`
        : "the Tier-1 index build has not completed";
  return taxonomyError({
    v,
    code: "INDEX_UNAVAILABLE",
    message:
      "read with a symbol address requires the Tier-1 index and it is not up yet " +
      "(SDD §9.3, Constitution I); retry once the build completes, or address by lines",
    notes: [progress],
  });
}

export type SymbolResolution =
  | { readonly ok: true; readonly symbol: SymbolEntry }
  | { readonly ok: false; readonly envelope: ErrorEnvelope };

/**
 * Resolve a symbol address within one file's line-ordered outline rows.
 * A dotted address ("UserService.authenticate") first tries the literal
 * name, then resolves member-inside-parent by extent containment (exact
 * when the index knows endLine, start-line precedence otherwise).
 * Shared with `edit` (§9.4): both tools speak one symbol-address dialect.
 */
export function resolveSymbol(
  v: number,
  rows: readonly SymbolEntry[],
  address: string,
  file: string,
): SymbolResolution {
  let candidates = rows.filter((row) => row.name === address);
  if (candidates.length === 0) {
    // The dialect is separator-agnostic: `.` for the curly-brace languages,
    // `::` for Rust (specs/009 R3). Literal matching above already resolves a
    // qualified Rust name; this split serves the member/parent fallback.
    const segments = address.split(/::|\./).filter((s) => s.length > 0);
    const member = segments[segments.length - 1] ?? address;
    const parent = segments.length >= 2 ? segments[segments.length - 2] : undefined;
    candidates = rows.filter((row) => row.name === member);
    if (candidates.length === 0) {
      // A member the index stores QUALIFIED (`Config::load`) must still answer
      // its bare address (`load`) — qualification is a precision gain, not a
      // new thing to type (specs/009 FR-004). Several owners with the same
      // member name is a real ambiguity, and the candidate list names them.
      candidates = rows.filter(
        (row) => row.name.endsWith(`::${member}`) || row.name.endsWith(`.${member}`),
      );
    }
    if (candidates.length === 0) {
      candidates = rows.filter((row) => row.name.toLowerCase() === member.toLowerCase());
    }
    if (parent !== undefined && candidates.length > 1) {
      const parents = rows.filter((row) => row.name === parent);
      const contained = candidates.filter((row) =>
        parents.some(
          (p) =>
            p.location.line < row.location.line &&
            (p.endLine === undefined || p.endLine >= row.location.line),
        ),
      );
      if (contained.length > 0) candidates = contained;
    }
  }
  if (candidates.length === 0) {
    const known = [...new Set(rows.map((row) => row.name))].slice(0, 8);
    return {
      ok: false,
      envelope: taxonomyError({
        v,
        code: "SYMBOL_NOT_FOUND",
        message:
          `no symbol ${JSON.stringify(address)} in ${file}; ` +
          "revise the address or read by lines",
        ...(known.length === 0
          ? {}
          : { notes: [`symbols in this file include: ${known.join(", ")}`] }),
      }),
    };
  }
  if (candidates.length > 1) {
    return {
      ok: false,
      envelope: taxonomyError({
        v,
        code: "AMBIGUOUS_SYMBOL",
        message:
          `${JSON.stringify(address)} matches ${candidates.length} symbols in ${file}; ` +
          "pick one via lines: {start, end}",
        candidates: candidates.map(
          (row) => `${qualifiedName(row)} (${row.kind}) line ${row.location.line}`,
        ),
      }),
    };
  }
  return { ok: true, symbol: candidates[0] as SymbolEntry };
}

/**
 * How a candidate is named back to the caller. When the index knows the
 * member came from a trait implementation, the fully-qualified Rust form
 * distinguishes it from an inherent member of the same name — the whole
 * reason `detail` is stored (specs/009 R2).
 */
function qualifiedName(row: SymbolEntry): string {
  if (row.detail === undefined || row.detail.length === 0) return row.name;
  const [type, member] = row.name.split("::");
  if (type === undefined || member === undefined) return row.name;
  return `<${type} as ${row.detail}>::${member}`;
}

/** A symbol's extent: the index's endLine when known, else start-to-next-row. */
export function extentOf(
  symbol: SymbolEntry,
  rows: readonly SymbolEntry[],
  totalLines: number,
): { start: number; end: number } {
  const start = symbol.location.line;
  if (symbol.endLine !== undefined && symbol.endLine >= start) {
    return { start, end: Math.min(symbol.endLine, totalLines) };
  }
  const next = rows.find((row) => row.location.line > start);
  return {
    start,
    end: Math.min(next === undefined ? totalLines : next.location.line - 1, totalLines),
  };
}

function symbolRef(symbol: SymbolEntry): ReadSymbolRef {
  return {
    name: symbol.name,
    kind: symbol.kind,
    line: symbol.location.line,
    ...(symbol.endLine === undefined ? {} : { endLine: symbol.endLine }),
    ...(symbol.doc === undefined ? {} : { doc: symbol.doc }),
  };
}

/** Lexical import-line shapes across the bundled-grammar languages. A
 * heuristic by design: the port exposes no import rows, and the envelope
 * says so in a note rather than overclaiming. */
const IMPORT_LINE =
  /^\s*(?:import[\s("'{]|export\s+.*\sfrom\s|from\s+\S+\s+import[\s(]|use\s+[\w:]+.*;|require\s*\(|#include[\s<"]|using\s+[\w.]+\s*;)/;

/** The file's leading import block: first to last import-ish line within the
 * scan window, gaps (comments, blank lines) included. */
function importBlock(lines: readonly string[]): { start: number; end: number } | null {
  const window = Math.min(lines.length, READ_IMPORT_SCAN_LINES);
  let first = 0;
  let last = 0;
  for (let i = 0; i < window; i++) {
    if (IMPORT_LINE.test(lines[i] as string)) {
      if (first === 0) first = i + 1;
      last = i + 1;
    }
  }
  return first === 0 ? null : { start: first, end: last };
}

/** ≈tokens one wire row costs, same estimator the ranking trim uses. */
function rowTokens(row: ReadResult): number {
  return Math.ceil(JSON.stringify(row).length / RANKING_WEIGHTS.charsPerToken);
}

function segmentRows(
  file: string,
  lines: readonly string[],
  range: { start: number; end: number },
): ReadSegment[] {
  const segments: ReadSegment[] = [];
  for (let start = range.start; start <= range.end; start += READ_SEGMENT_LINES) {
    const end = Math.min(start + READ_SEGMENT_LINES - 1, range.end);
    segments.push({
      kind: "lines",
      file,
      start,
      end,
      text: lines.slice(start - 1, end).join("\n"),
    });
  }
  return segments;
}

/**
 * Answer one read call with a ResponseEnvelope. The caller owns the jail
 * decision, negotiates the envelope version, and seals (redacts) the result
 * before it reaches a wire.
 */
export async function runRead(
  deps: ReadDeps,
  input: ReadInput,
  v: number,
): Promise<ResponseEnvelope<ReadResult>> {
  if (input.symbol !== undefined && input.lines !== undefined) {
    throw new RangeError("read: symbol and lines are mutually exclusive (schema `not`)");
  }
  const budget = input.budget ?? 4000;
  const surround: ReadSurround = (READ_SURROUNDS as readonly string[]).includes(
    input.surround ?? "",
  )
    ? (input.surround as ReadSurround)
    : "enclosing";
  const file = normalizeRelPath(input.file);
  const notes: string[] = [];

  let snapshot: Snapshot;
  try {
    snapshot = await readVerified(deps.workspace, input.file);
  } catch (error) {
    if (isMissingFile(error)) return fileNotFound(v, file);
    throw error;
  }
  let freshness = snapshot.freshness;
  if (snapshot.note !== undefined) notes.push(snapshot.note);

  const lines = snapshot.content.split("\n");
  if (lines[lines.length - 1] === "") lines.pop();
  const totalLines = lines.length;

  let range: { start: number; end: number };
  let symbol: ReadSymbolRef | undefined;
  let enclosing: ReadSymbolRef | undefined;

  if (input.symbol !== undefined) {
    const status = await indexStatusOrNull(deps.index);
    if (status === null || status.tier === 0) {
      return indexUnavailable(v, status);
    }
    let outline: Awaited<ReturnType<IndexPort["queryOutline"]>>;
    try {
      outline = await deps.index.queryOutline({ paths: [file] });
    } catch {
      return indexUnavailable(v, status);
    }
    freshness = worstFreshness(freshness, outline.freshness);
    const rows = outline.files.find((f) => f.path === file)?.symbols ?? [];
    const resolution = resolveSymbol(v, rows, input.symbol, file);
    if (!resolution.ok) return resolution.envelope;
    const resolved = resolution.symbol;
    range = extentOf(resolved, rows, totalLines);
    symbol = symbolRef(resolved);

    // Belt and suspenders (§7.3.3): the anchor must describe the content we
    // are actually serving, even if the index raced a writer.
    const member = resolved.name.split(".").pop() ?? resolved.name;
    const anchorText = lines[range.start - 1];
    if (anchorText === undefined || !anchorText.includes(member)) {
      freshness = "cold";
      notes.push(
        `the symbol anchor (line ${range.start}) did not match the current content; ` +
          "line numbers may be approximate until the index catches up",
      );
    }

    if (surround === "enclosing") {
      const symbolEnd = resolved.endLine ?? range.start;
      const parents = rows.filter(
        (row) =>
          row !== resolved &&
          row.location.line <= range.start &&
          row.endLine !== undefined &&
          row.endLine >= symbolEnd,
      );
      const parent = parents[parents.length - 1];
      if (parent !== undefined) {
        enclosing = symbolRef(parent);
        range = {
          start: parent.location.line,
          end: Math.min(parent.endLine as number, totalLines),
        };
      }
    }
  } else if (input.lines !== undefined) {
    const start = Math.max(1, Math.min(input.lines.start, input.lines.end));
    const end = Math.max(input.lines.start, input.lines.end);
    if (start !== input.lines.start || end !== input.lines.end) {
      notes.push("lines: start and end were swapped/clamped to a valid ascending range");
    }
    if (start > totalLines && totalLines > 0) {
      notes.push(`lines.start ${start} is past the end of the file (${totalLines} lines)`);
    }
    if (end > totalLines) {
      notes.push(`lines.end clamped to the end of the file (${totalLines} lines)`);
    }
    range = { start: Math.min(start, totalLines), end: Math.min(end, totalLines) };
  } else {
    range = { start: Math.min(1, totalLines), end: totalLines };
  }
  if (totalLines === 0) {
    range = { start: 0, end: 0 };
    notes.push("the file is empty");
  }

  const contentHash = new Bun.CryptoHasher("sha256").update(snapshot.content).digest("hex");
  const unit: ReadUnit = {
    kind: "unit",
    file,
    range,
    totalLines,
    contentHash,
    ...(symbol === undefined ? {} : { symbol }),
    ...(enclosing === undefined ? {} : { enclosing }),
  };

  const rows: ReadResult[] = [unit];
  if (surround === "imports" && totalLines > 0) {
    const block = importBlock(lines);
    if (block === null) {
      notes.push("surround:imports found no import lines in the leading scan window");
    } else if (block.start < range.start || block.end > range.end) {
      rows.push({
        kind: "imports",
        file,
        start: block.start,
        end: block.end,
        text: lines.slice(block.start - 1, block.end).join("\n"),
      });
      notes.push(
        "the imports block is a lexical scan (leading import-shaped lines), not resolver output",
      );
    }
  }
  if (range.start >= 1 && range.end >= range.start) {
    rows.push(...segmentRows(file, lines, range));
  }

  let used = 0;
  let keep = 0;
  for (const row of rows) {
    const cost = rowTokens(row);
    if (used + cost > budget) break;
    used += cost;
    keep += 1;
  }
  if (keep === 0) {
    return taxonomyError({
      v,
      code: "BUDGET_TOO_SMALL",
      message:
        `budget ${budget} cannot fit even the unit header ` +
        `(≈${rowTokens(unit)} tokens); raise the budget`,
    });
  }
  const cursorRow = rows[keep];
  const cut = truncateResults(rows, keep, {
    reason: "budget",
    narrow: ["lines: {start, end} a narrower range", 'surround: "none"', "raise budget"],
    cursor:
      cursorRow === undefined
        ? "-"
        : cursorRow.kind === "imports"
          ? "imports"
          : cursorRow.kind === "lines"
            ? `line:${cursorRow.start}`
            : "-",
  });

  return okEnvelope<ReadResult>({
    v,
    freshness,
    budget: { requested: budget, used },
    results: cut,
    ...(notes.length === 0 ? {} : { notes }),
  });
}
