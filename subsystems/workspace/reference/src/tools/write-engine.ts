/**
 * The write engine (SDD §7.5, law #3) — the single mutation choke point.
 * Every mutation from any tool (edit, files, script commits, undo) runs one
 * pipeline: lock → resolve targets → dry-run diff → shadow apply → parse-
 * verify → format → re-verify → journal → atomic swap → report the applied
 * diff plus any new diagnostics.
 *
 * Atomicity is structural, not procedural: the shadow copy is in memory and
 * every verification (parse, well-formedness, post-format re-verify) runs on
 * content strings through ports BEFORE the first byte reaches the
 * filesystem. A new syntax error (PARSE_REJECTED) or an engine-child death
 * mid-verify therefore rolls the whole batch back by touching nothing. Only
 * the final swap mutates files; a swap failure mid-batch compensates by
 * restoring the already-swapped files from their journaled before-content.
 *
 * Later M2 chunks plug into seams, not rewrites: the per-workspace write
 * lock (m2-write-lock) implements `WriteLock`, the MRTR confirm flow
 * (m2-mrtr-confirm) sits between `dryRun` and apply, the durable journal
 * (m2-journal-undo) implements `JournalPort`, and the edit verbs
 * (m2-edit-verbs) resolve their targets into `WriteOp`s.
 *
 * This layer sees only ports and the envelope contract (import-graph rule);
 * file access is injected by the composing layer, which owns the jail check.
 */

import {
  contentEqual,
  type FileContent,
  okEnvelope,
  type ResponseEnvelope,
  taxonomyError,
  unifiedDiff,
} from "../envelope";
import type {
  EnginePort,
  FormatterPort,
  JournalFileChange,
  JournalFileKind,
  JournalPort,
  WriteLock,
} from "../ports";

export type { WriteLock } from "../ports";

/** Mutating file access the composing layer injects, jail-checked before it
 * is built. `write` must replace-or-create atomically per file (temp +
 * rename); the batch-level all-or-nothing property is this module's job. */
export interface WriteWorkspaceAccess {
  read(path: string): Promise<string>;
  write(path: string, content: string): Promise<void>;
  /** Lossless file operations; text-only test adapters can omit these. */
  readContent?(path: string): Promise<FileContent>;
  writeContent?(path: string, content: FileContent): Promise<void>;
  remove(path: string): Promise<void>;
}

export function readWorkspaceContent(
  workspace: WriteWorkspaceAccess,
  path: string,
): Promise<FileContent> {
  return workspace.readContent ? workspace.readContent(path) : workspace.read(path);
}

function writeWorkspaceContent(
  workspace: WriteWorkspaceAccess,
  path: string,
  content: FileContent,
): Promise<void> {
  if (workspace.writeContent) return workspace.writeContent(path, content);
  if (typeof content === "string") return workspace.write(path, content);
  throw new Error("workspace adapter does not support binary writes");
}

export interface WriteEngineDeps {
  readonly engine: EnginePort;
  readonly formatter: FormatterPort;
  readonly journal: JournalPort;
  readonly workspace: WriteWorkspaceAccess;
  readonly lock?: WriteLock;
}

/** One intended file state. Content is the full proposed file; null deletes. */
export interface WriteOp {
  readonly path: string;
  readonly content: FileContent | null;
  /** read's contentHash staleness anchor (§9.3); mismatch → STALE_ANCHOR. */
  readonly expectedHash?: string;
  /** Baseline content for the §7.5 new-syntax-error decision, when it is not
   * this path's own before-content: a `files` move/copy passes the source
   * file's content so relocating an already-broken file reports diagnostics
   * instead of rejecting — relocation introduces no NEW error. */
  readonly baseline?: FileContent;
}

/** Who this batch is for the journal's benefit (§7.6): identity from the
 * request, never from server state. Absent origin journals as anonymous. */
export interface WriteOrigin {
  /** Originating tool (edit, files, script, undo). */
  readonly tool: string;
  /** sha256 of the originating tool's arguments. */
  readonly argsHash: string;
  /** Client/session identity from `_meta`; "" is the anonymous identity. */
  readonly sessionId: string;
  /** Journal seqs this batch reverts — set only by `undo`. */
  readonly undoOf?: readonly number[];
}

export interface WriteBatchInput {
  readonly writes: readonly WriteOp[];
  /** Preview: run the full pipeline through re-verify, touch nothing. */
  readonly dryRun?: boolean;
  readonly budget?: number;
  readonly origin?: WriteOrigin;
  /** false skips the §7.5 format step. `undo` restores exact journaled
   * bytes — re-formatting would break the afterHash chain that undo's
   * conflict detection depends on. Defaults to true. */
  readonly format?: boolean;
}

export type WriteAction = "created" | "modified" | "deleted" | "unchanged";

/** One result row per write, in input order. */
export interface WriteReport {
  readonly kind: "write";
  readonly path: string;
  readonly action: WriteAction;
  /** false on dryRun and for unchanged writes: the file was not touched. */
  readonly applied: boolean;
  /** Unified diff of what was (or would be) applied; "" when unchanged. */
  readonly diff: string;
  /** sha256 of the final content as swapped in; absent for deletions. */
  readonly contentHash?: string;
  /** Journal sequence number of the recorded op; absent on dryRun. */
  readonly journalSeq?: number;
  /** Parser diagnostics that REMAIN after an allowed write to a file whose
   * before-content was already syntactically broken (§7.5: new syntax
   * errors reject; pre-existing ones are reported, not silently blessed). */
  readonly diagnostics?: readonly string[];
}

const WRITE_BUDGET_DEFAULT = 4000;
/** Same chars-per-token estimator the ranking trim uses. */
const CHARS_PER_TOKEN = 4;

/** Mirror of read.ts: the workspace-relative posix dialect results speak. */
function normalizeRelPath(path: string): string {
  return path
    .replaceAll("\\", "/")
    .split("/")
    .filter((p) => p !== "" && p !== ".")
    .join("/");
}

function sha256(content: FileContent): string {
  return new Bun.CryptoHasher("sha256").update(content).digest("hex");
}

function isMissingFile(error: unknown): boolean {
  const code = (error as NodeJS.ErrnoException).code;
  return code === "ENOENT" || code === "ENOTDIR" || code === "EISDIR";
}

/** A verify verdict: the engine's parse plus this module's own
 * well-formedness pass for the §7.5 non-source formats. */
interface Verdict {
  readonly ok: boolean;
  readonly message?: string;
}

const WELL_FORMED_EXTENSIONS = ["json", "yaml", "yml", "toml"] as const;

/**
 * §7.5: non-source files skip parse-verify but still get JSON/YAML/TOML
 * well-formedness checks. The engine child covers json (bundled grammar)
 * but answers {ok:true} for grammars it does not bundle, so yaml/toml are
 * this module's to check — and running json here too keeps the guarantee
 * adapter-independent. Returns null for formats with no checker.
 */
function wellFormedness(path: string, content: string): Verdict | null {
  const ext = path.slice(path.lastIndexOf(".") + 1).toLowerCase();
  if (!(WELL_FORMED_EXTENSIONS as readonly string[]).includes(ext)) return null;
  try {
    if (ext === "json") JSON.parse(content);
    else if (ext === "toml") Bun.TOML.parse(content);
    else Bun.YAML.parse(content);
    return { ok: true };
  } catch (error) {
    const message = error instanceof Error ? error.message : String(error);
    return { ok: false, message: `${ext} is not well-formed: ${message}` };
  }
}

async function verify(deps: WriteEngineDeps, path: string, content: string): Promise<Verdict> {
  const parsed = await deps.engine.parse({ path, content });
  if (!parsed.ok) {
    return { ok: false, message: parsed.parserMessage ?? "syntax error" };
  }
  return wellFormedness(path, content) ?? { ok: true };
}

/** The §4.3 mapping for a verify-phase engine failure: a typed code travels
 * through (the supervisor's ENGINE_UNAVAILABLE / INPUT_QUARANTINED), and
 * anything else degrades honestly to ENGINE_UNAVAILABLE — in every case the
 * batch has touched nothing, which is the §7.5 rollback. */
function engineFailure(
  v: number,
  error: unknown,
  batchSize: number,
): ResponseEnvelope<WriteReport> {
  const message = error instanceof Error ? error.message : String(error);
  const code = (error as { code?: unknown }).code;
  const notes = [
    `the engine failed during parse-verify; the whole batch of ${batchSize} write(s) rolled back — nothing was applied`,
  ];
  if (code === "INPUT_QUARANTINED") {
    return taxonomyError({ v, code: "INPUT_QUARANTINED", message, notes });
  }
  if (code === "ENGINE_BUSY") {
    const backoffMs = (error as { backoffMs?: unknown }).backoffMs;
    return taxonomyError({
      v,
      code: "ENGINE_BUSY",
      message,
      backoffMs: typeof backoffMs === "number" ? backoffMs : 250,
      notes,
    });
  }
  return taxonomyError({ v, code: "ENGINE_UNAVAILABLE", message, notes });
}

/** One write's resolved, verified state as it moves through the pipeline. */
interface PlannedWrite {
  readonly op: WriteOp;
  readonly path: string;
  readonly before: FileContent | null;
  /** The content to swap in (post-format); null for deletions. */
  final: FileContent | null;
  readonly action: WriteAction;
  diff: string;
  diagnostics: string[];
  journalSeq?: number;
}

function journalKind(action: WriteAction): JournalFileKind {
  return action === "created" ? "create" : action === "deleted" ? "delete" : "write";
}

function report(planned: PlannedWrite, applied: boolean): WriteReport {
  return {
    kind: "write",
    path: planned.path,
    action: planned.action,
    applied,
    diff: planned.diff,
    ...(planned.final === null ? {} : { contentHash: sha256(planned.final) }),
    ...(planned.journalSeq === undefined ? {} : { journalSeq: planned.journalSeq }),
    ...(planned.diagnostics.length === 0 ? {} : { diagnostics: planned.diagnostics }),
  };
}

/**
 * Run one write batch through the §7.5 pipeline and answer with a
 * ResponseEnvelope. The caller owns the jail decision per path, negotiates
 * the envelope version, and seals (redacts) the result before it reaches a
 * wire. Throws RangeError on schema-level misuse (empty batch, duplicate
 * paths); returns taxonomy errors for everything the agent can recover from.
 */
export async function runWriteBatch(
  deps: WriteEngineDeps,
  input: WriteBatchInput,
  v: number,
): Promise<ResponseEnvelope<WriteReport>> {
  if (input.writes.length === 0) {
    throw new RangeError("write batch: at least one write is required");
  }
  const seen = new Set<string>();
  for (const op of input.writes) {
    const path = normalizeRelPath(op.path);
    if (path === "") throw new RangeError("write batch: a write names an empty path");
    if (seen.has(path)) {
      throw new RangeError(`write batch: duplicate path ${JSON.stringify(path)}`);
    }
    seen.add(path);
  }
  const lock: WriteLock = deps.lock ?? { withLock: (fn) => fn() };
  try {
    return await lock.withLock(() => pipeline(deps, input, v));
  } catch (error) {
    // §7.7 typed backpressure: a lock that cannot be acquired within its
    // bound answers ENGINE_BUSY with a suggested backoff instead of hanging.
    if ((error as { code?: unknown }).code === "ENGINE_BUSY") {
      const backoffMs = (error as { backoffMs?: unknown }).backoffMs;
      return taxonomyError({
        v,
        code: "ENGINE_BUSY",
        message: error instanceof Error ? error.message : String(error),
        backoffMs: typeof backoffMs === "number" ? backoffMs : 250,
        notes: [`the workspace write lock is held; nothing from this batch was applied`],
      });
    }
    throw error;
  }
}

async function pipeline(
  deps: WriteEngineDeps,
  input: WriteBatchInput,
  v: number,
): Promise<ResponseEnvelope<WriteReport>> {
  const dryRun = input.dryRun === true;
  const notes: string[] = [];

  // Resolve targets: read every before-content and check staleness anchors.
  const planned: PlannedWrite[] = [];
  for (const op of input.writes) {
    const path = normalizeRelPath(op.path);
    let before: FileContent | null;
    try {
      before = await readWorkspaceContent(deps.workspace, path);
    } catch (error) {
      if (!isMissingFile(error)) throw error;
      before = null;
    }
    if (op.content === null && before === null) {
      return taxonomyError({
        v,
        code: "FILE_NOT_FOUND",
        message:
          `cannot delete ${JSON.stringify(path)}: no such file in the workspace; ` +
          "nothing from this batch was applied",
      });
    }
    if (op.expectedHash !== undefined && sha256(before ?? "") !== op.expectedHash) {
      return taxonomyError({
        v,
        code: "STALE_ANCHOR",
        message:
          `${JSON.stringify(path)} changed since the write was prepared ` +
          "(expectedHash mismatch); re-read the file and regenerate the write — " +
          "nothing from this batch was applied",
        contentHash: sha256(before ?? ""),
      });
    }
    const action: WriteAction =
      op.content === null
        ? "deleted"
        : before === null
          ? "created"
          : contentEqual(op.content, before)
            ? "unchanged"
            : "modified";
    planned.push({
      op,
      path,
      before,
      final: op.content,
      action,
      diff: "",
      diagnostics: [],
    });
  }

  // Shadow apply is the in-memory `final`; verify every changed file before
  // any fs mutation. PARSE_REJECTED is reserved for NEW syntax errors: a
  // file whose before-content already fails to parse stays writable, with
  // the remaining breakage reported as diagnostics instead of blessed.
  const changed = planned.filter((p) => p.action !== "unchanged");
  for (const p of changed) {
    p.diff = unifiedDiff({ path: p.path, before: p.before, after: p.final });
    if (typeof p.final !== "string") continue;
    let baseline: Verdict;
    let candidate: Verdict;
    try {
      const baselineContent = p.op.baseline ?? p.before;
      baseline =
        typeof baselineContent !== "string"
          ? { ok: true }
          : await verify(deps, p.path, baselineContent);
      candidate = await verify(deps, p.path, p.final);
    } catch (error) {
      return engineFailure(v, error, input.writes.length);
    }
    if (!candidate.ok) {
      if (baseline.ok) {
        return taxonomyError({
          v,
          code: "PARSE_REJECTED",
          message:
            `the write to ${JSON.stringify(p.path)} would introduce a syntax error; ` +
            `the whole batch of ${input.writes.length} write(s) rolled back — nothing was applied`,
          parserMessage: candidate.message ?? "syntax error",
          rejectedDiff: p.diff,
        });
      }
      p.diagnostics.push(candidate.message ?? "syntax error");
      notes.push(
        `${p.path}: before-content already failed to parse; the write is allowed ` +
          "and the remaining breakage is reported in diagnostics (§7.5)",
      );
    }
  }

  // Format touched files, then re-verify anything the formatter moved. A
  // formatter that breaks verified content is a formatter bug; the pipeline
  // still never applies broken content — it falls back to the verified
  // unformatted candidate and says so. `format: false` (undo) skips the pass
  // wholesale: exact-bytes restoration is the contract there.
  for (const p of input.format === false ? [] : changed) {
    if (typeof p.final !== "string" || p.diagnostics.length > 0) continue;
    let formatted: { content: string; changed: boolean };
    try {
      formatted = await deps.formatter.format({ path: p.path, content: p.final });
    } catch (error) {
      // specs/012 R6: the note carries the formatter's own message, so
      // "not installed" and "rejected the content" call for different fixes.
      const message = error instanceof Error ? error.message : String(error);
      notes.push(
        `${p.path}: the formatter failed (${message}); applied the verified unformatted content`,
      );
      continue;
    }
    if (!formatted.changed) continue;
    let reVerified: Verdict;
    try {
      reVerified = await verify(deps, p.path, formatted.content);
    } catch (error) {
      return engineFailure(v, error, input.writes.length);
    }
    if (reVerified.ok) {
      p.final = formatted.content;
      p.diff = unifiedDiff({ path: p.path, before: p.before, after: p.final });
    } else {
      notes.push(
        `${p.path}: formatting broke the parse (${reVerified.message ?? "syntax error"}); ` +
          "applied the verified unformatted content",
      );
    }
  }

  const budget = {
    requested: input.budget ?? WRITE_BUDGET_DEFAULT,
    used: 0,
  };
  const finish = (results: readonly WriteReport[]): ResponseEnvelope<WriteReport> =>
    okEnvelope({
      v,
      freshness: "warm",
      budget: {
        requested: budget.requested,
        used: Math.ceil(JSON.stringify(results).length / CHARS_PER_TOKEN),
      },
      // A write's report is never cut: the applied diff is the receipt.
      results,
      ...(notes.length === 0 ? {} : { notes }),
    });

  if (dryRun) {
    notes.push("dry run: the pipeline verified the batch and touched nothing");
    return finish(planned.map((p) => report(p, false)));
  }

  // §7.5 order: journal before swap, so undo can always see the op. One op
  // per batch (§10.1): every changed file rides the same journal seq, tagged
  // with the origin identity from the request (§7.6).
  if (changed.length > 0) {
    const files: JournalFileChange[] = changed.map((p) => ({
      kind: journalKind(p.action),
      path: p.path,
      before: p.before ?? "",
      after: p.final ?? "",
    }));
    const origin = input.origin;
    const appended = await deps.journal.append({
      tool: origin?.tool ?? "write",
      argsHash: origin?.argsHash ?? "",
      sessionId: origin?.sessionId ?? "",
      files,
      ...(origin?.undoOf === undefined ? {} : { undoOf: origin.undoOf }),
    });
    for (const p of changed) p.journalSeq = appended.op.seq;
    notes.push(...appended.notes);
  }

  // Atomic swap. Everything is verified; per-file writes are atomic by the
  // injected access's contract. If one still fails mid-batch, restore the
  // files already swapped from their before-content (best effort), so the
  // batch stays all-or-nothing on disk too.
  const swapped: PlannedWrite[] = [];
  for (const p of changed) {
    try {
      if (p.final === null) await deps.workspace.remove(p.path);
      else await writeWorkspaceContent(deps.workspace, p.path, p.final);
      swapped.push(p);
    } catch (error) {
      for (const done of swapped.reverse()) {
        try {
          if (done.before === null) await deps.workspace.remove(done.path);
          else await writeWorkspaceContent(deps.workspace, done.path, done.before);
        } catch {
          // Compensation is best effort; the journal still holds the truth.
        }
      }
      throw error;
    }
  }

  return finish(planned.map((p) => report(p, p.action !== "unchanged")));
}
