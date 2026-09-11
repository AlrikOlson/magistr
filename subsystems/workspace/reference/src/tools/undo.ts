/**
 * `undo` (SDD §9.8, §7.6) — session-scoped journal rewind with conflict
 * surfacing, built ON the write engine rather than beside it: every
 * restoration is a write batch through the §7.5 choke point (constitution
 * III), with `format: false` because the contract is exact-bytes restoration
 * and `expectedHash` anchors so a concurrent mutation fails closed with
 * STALE_ANCHOR instead of clobbering.
 *
 * Scope defaults to the calling session (§7.6: two agents sharing a
 * workspace must never silently revert each other); `scope: "workspace"`
 * undoes globally. Identity arrives on the request (`_meta`) and is matched
 * against the tag the journal stored — no in-memory session state.
 *
 * Conflicts — a later op (or an outside edit) changed a file this undo would
 * restore — are detected by content hash and surfaced as a three-way choice
 * in the envelope, never as a silent clobber and never as a new taxonomy
 * code: keep the current content (`onConflict: "keepCurrent"`), restore the
 * journaled version anyway (`onConflict: "force"`), or merge by hand with
 * the hashes provided. The default (`"surface"`) applies nothing.
 *
 * `action: "list"` is the read half: the journal as rows, newest op first,
 * one per file an op touched, each carrying the op `seq` that `ops: [seq]`
 * then names. It writes nothing and takes no lock, so a Changes view can
 * tail it. The same rows are what the lock-free `workbench-mcp journal` CLI
 * prints (src/tools/journal-view).
 */

import {
  contentEqual,
  type FileContent,
  okEnvelope,
  type ResponseEnvelope,
  truncateResults,
} from "../envelope";
import type { JournalOp } from "../ports";
import { JOURNAL_DIFF_CHARS_MAX, type JournalRow, journalRows } from "./journal-view";
import {
  readWorkspaceContent,
  runWriteBatch,
  type WriteEngineDeps,
  type WriteOp,
  type WriteReport,
} from "./write-engine";

export type UndoScope = "session" | "workspace";
export type UndoConflictPolicy = "surface" | "force" | "keepCurrent";
export type UndoAction = "rewind" | "list";

export interface UndoInput {
  /** rewind (default) rewinds ops; list reads the journal and writes nothing. */
  readonly action?: UndoAction;
  /** How many ops (write batches, not files) to rewind. Default 1. */
  readonly steps?: number;
  /** rewind: the exact ops to rewind, by journal seq (as `list` reports
   * them), instead of the newest `steps`. An op named here that is not a
   * candidate — an undo-op, already undone, or another session's under
   * session scope — is reported in the notes and skipped. */
  readonly ops?: readonly number[];
  /** Defaults to session for rewind (§7.6) and workspace for list: reading
   * another session's history reverts nothing. */
  readonly scope?: UndoScope;
  readonly onConflict?: UndoConflictPolicy;
  /** list: how many ops to return, newest first. Default 20. */
  readonly limit?: number;
  /** list: include each row's unified diff. Default true. */
  readonly diff?: boolean;
  /** list: only ops with `seq >= sinceSeq` — tailing. */
  readonly sinceSeq?: number;
  /** list: only ops with `seq < beforeSeq` — the pagination cursor. */
  readonly beforeSeq?: number;
  readonly budget?: number;
}

/** A file a later change touched: the §7.6 three-way choice, as data. */
export interface UndoConflictRow {
  readonly kind: "conflict";
  readonly path: string;
  readonly opSeq: number;
  /** sha256 the journal expected the file to have now (the op's after). */
  readonly expectedHash: string;
  /** sha256 of what is actually there ("" = the file is absent). */
  readonly currentHash: string;
  /** sha256 of what undo would restore (the op's before). */
  readonly restoreHash: string;
}

/** A file whose journal blob fell to the §7.6 size cap — not restorable. */
export interface UndoEvictedRow {
  readonly kind: "evicted";
  readonly path: string;
  readonly opSeq: number;
}

/** A conflicted file deliberately left as-is under `keepCurrent`. */
export interface UndoKeptRow {
  readonly kind: "kept";
  readonly path: string;
  readonly opSeq: number;
}

export type UndoRow = WriteReport | UndoConflictRow | UndoEvictedRow | UndoKeptRow | JournalRow;

export const UNDO_STEPS_MAX = 100;
export const UNDO_LIST_LIMIT_MAX = 200;
export const UNDO_LIST_LIMIT_DEFAULT = 20;
const UNDO_LIST_BUDGET_DEFAULT = 8000;

function sha256(content: FileContent): string {
  return new Bun.CryptoHasher("sha256").update(content).digest("hex");
}

function isMissingFile(error: unknown): boolean {
  const code = (error as NodeJS.ErrnoException).code;
  return code === "ENOENT" || code === "ENOTDIR" || code === "EISDIR";
}

const CHARS_PER_TOKEN = 4;
const UNDO_BUDGET_DEFAULT = 4000;

function finish(
  v: number,
  budget: number,
  results: readonly UndoRow[],
  notes: readonly string[],
): ResponseEnvelope<UndoRow> {
  return okEnvelope({
    v,
    freshness: "warm",
    budget: {
      requested: budget,
      used: Math.ceil(JSON.stringify(results).length / CHARS_PER_TOKEN),
    },
    results,
    ...(notes.length === 0 ? {} : { notes }),
  });
}

/**
 * Run one undo through the journal and the write engine. `sessionId` is the
 * caller's identity from `_meta` ("" = anonymous). Throws RangeError on
 * schema-level misuse; everything an agent can act on comes back typed in
 * the envelope.
 */
export async function runUndo(
  deps: WriteEngineDeps,
  input: UndoInput,
  v: number,
  sessionId: string,
): Promise<ResponseEnvelope<UndoRow>> {
  if ((input.action ?? "rewind") === "list") return runJournalList(deps, input, v, sessionId);
  const steps = input.steps ?? 1;
  if (!Number.isInteger(steps) || steps < 1 || steps > UNDO_STEPS_MAX) {
    throw new RangeError(`undo: steps must be an integer in [1, ${UNDO_STEPS_MAX}]`);
  }
  const scope: UndoScope = input.scope ?? "session";
  const onConflict: UndoConflictPolicy = input.onConflict ?? "surface";
  const budget = input.budget ?? UNDO_BUDGET_DEFAULT;
  const notes: string[] = [];

  if (scope === "session" && sessionId === "") {
    notes.push(
      "undo: no workbench/sessionId was supplied via _meta — session scope matches only " +
        'ops journaled anonymously; pass a session id or scope: "workspace"',
    );
  }

  // Named ops or the newest ones: one eligibility predicate either way, so a
  // seq a caller read off `list` can still have gone stale under them.
  const named = input.ops;
  if (named !== undefined) {
    if (named.length === 0) throw new RangeError("undo: ops must name at least one op");
    if (named.length > UNDO_STEPS_MAX) {
      throw new RangeError(`undo: ops may name at most ${UNDO_STEPS_MAX} ops`);
    }
    for (const seq of named) {
      if (!Number.isInteger(seq) || seq < 1) {
        throw new RangeError(`undo: ops must be positive integer journal seqs, got ${seq}`);
      }
    }
    if (input.steps !== undefined) {
      notes.push("undo: steps is ignored when ops names the ops to rewind");
    }
  }
  const wanted = named === undefined ? steps : named.length;
  const candidates = await deps.journal.undoCandidates({
    ...(scope === "session" ? { sessionId } : {}),
    limit: wanted,
    ...(named === undefined ? {} : { seqs: named }),
  });
  if (candidates.length === 0) {
    notes.push(
      named === undefined
        ? `undo: nothing to undo in ${scope} scope`
        : `undo: op(s) ${named.join(", ")} are not undoable in ${scope} scope — already undone, ` +
            "an undo's own op, or another session's",
    );
    return finish(v, budget, [], notes);
  }
  if (candidates.length < wanted) {
    if (named === undefined) {
      notes.push(
        `undo: only ${candidates.length} of the requested ${steps} op(s) are undoable in ${scope} scope`,
      );
    } else {
      const missing = named.filter((seq) => !candidates.some((op) => op.seq === seq));
      notes.push(
        `undo: op(s) ${missing.join(", ")} are not undoable in ${scope} scope and were skipped`,
      );
    }
  }

  // Simulate newest→oldest: each file's current content must match the op's
  // journaled after-content (by hash) for a clean rewind to its before.
  const actual = new Map<string, FileContent | null>();
  const readActual = async (path: string): Promise<FileContent | null> => {
    const cached = actual.get(path);
    if (cached !== undefined || actual.has(path)) return cached ?? null;
    let content: FileContent | null;
    try {
      content = await readWorkspaceContent(deps.workspace, path);
    } catch (error) {
      if (!isMissingFile(error)) throw error;
      content = null;
    }
    actual.set(path, content);
    return content;
  };

  const sim = new Map<string, FileContent | null>();
  const conflicts: UndoConflictRow[] = [];
  const evictedRows: UndoEvictedRow[] = [];
  const keptRows: UndoKeptRow[] = [];
  const undoOf: number[] = [];

  for (const op of candidates) {
    if (opHasEvictedBlob(op)) {
      for (const file of op.files) {
        evictedRows.push({ kind: "evicted", path: file.path, opSeq: op.seq });
      }
      notes.push(
        `undo: op ${op.seq} is no longer undoable — its journal blobs were evicted under ` +
          "the size cap (§7.6); it was skipped and remains in the log",
      );
      continue;
    }
    for (const file of op.files) {
      const path = file.path;
      const current = sim.has(path) ? (sim.get(path) ?? null) : await readActual(path);
      if (!sim.has(path)) sim.set(path, current);
      // What the workspace should hold now if nothing intervened…
      const expected = file.kind === "delete" ? null : file.after;
      // …and what undo would put back.
      const target = file.kind === "create" ? null : file.before;
      const clean = contentEqual(current, expected);
      if (!clean && onConflict === "surface") {
        conflicts.push({
          kind: "conflict",
          path,
          opSeq: op.seq,
          expectedHash: expected === null ? "" : file.afterHash,
          currentHash: current === null ? "" : sha256(current),
          restoreHash: target === null ? "" : file.beforeHash,
        });
        continue;
      }
      if (!clean && onConflict === "keepCurrent") {
        keptRows.push({ kind: "kept", path, opSeq: op.seq });
        continue;
      }
      // Clean, or forced through a conflict.
      if (!clean) {
        notes.push(
          `undo: ${path} changed after op ${op.seq}; restored the journaled version anyway (force)`,
        );
      }
      sim.set(path, target);
    }
    undoOf.push(op.seq);
  }

  if (conflicts.length > 0) {
    notes.push(
      "undo: conflicts detected — a later change touched these files; nothing was applied. " +
        'Three ways forward: re-run with onConflict: "keepCurrent" to leave the conflicted ' +
        'files as they are, onConflict: "force" to restore the journaled versions anyway, ' +
        "or read the file and merge by hand using the hashes in each conflict row",
    );
    return finish(v, budget, [...conflicts, ...evictedRows], notes);
  }

  // Everything that actually differs from disk becomes one write batch.
  const writes: WriteOp[] = [];
  for (const [path, target] of sim) {
    const current = actual.get(path) ?? null;
    if (contentEqual(target, current)) continue;
    writes.push({
      path,
      content: target,
      expectedHash: sha256(current ?? ""),
    });
  }

  const argsHash = sha256(JSON.stringify({ onConflict, scope, steps }));
  if (writes.length === 0) {
    // Nothing to touch (all kept, or the rewind lands on identical bytes) —
    // still mark the choice as consumed so these ops stop being candidates.
    if (undoOf.length > 0) {
      const appended = await deps.journal.append({
        tool: "undo",
        argsHash,
        sessionId,
        files: [],
        undoOf,
      });
      notes.push(...appended.notes);
      notes.push(`undo: op(s) ${undoOf.join(", ")} marked undone; no file content changed`);
    }
    return finish(v, budget, [...keptRows, ...evictedRows], notes);
  }

  const envelope = await runWriteBatch(
    deps,
    {
      writes,
      budget,
      format: false,
      origin: { tool: "undo", argsHash, sessionId, undoOf },
    },
    v,
  );
  if (!envelope.ok) {
    // Typed failure from the choke point (STALE_ANCHOR race, PARSE_REJECTED
    // when the restored before-content is broken, ENGINE_*): pass it through
    // with the undo context attached. Nothing was applied or journaled.
    return {
      ...envelope,
      notes: [
        ...(envelope.notes ?? []),
        `undo: the restoration batch was refused; op(s) ${undoOf.join(", ")} remain undoable`,
      ],
    };
  }
  notes.push(`undo: reverted op(s) ${undoOf.join(", ")} in ${scope} scope`);
  notes.push(...(envelope.notes ?? []));
  return finish(v, budget, [...envelope.results, ...keptRows, ...evictedRows], notes);
}

function opHasEvictedBlob(op: JournalOp): boolean {
  return op.files.some(
    (file) =>
      (file.kind !== "delete" && file.after === null) ||
      (file.kind !== "create" && file.before === null),
  );
}

/**
 * The read half: the journal as rows, newest op first. Writes nothing, takes
 * no lock, journals nothing — a Changes view tails this. Scope defaults to
 * the whole workspace because reading another session's history reverts
 * nothing; `scope: "session"` narrows it to the caller's own ops.
 */
async function runJournalList(
  deps: WriteEngineDeps,
  input: UndoInput,
  v: number,
  sessionId: string,
): Promise<ResponseEnvelope<UndoRow>> {
  const limit = input.limit ?? UNDO_LIST_LIMIT_DEFAULT;
  if (!Number.isInteger(limit) || limit < 1 || limit > UNDO_LIST_LIMIT_MAX) {
    throw new RangeError(`undo: limit must be an integer in [1, ${UNDO_LIST_LIMIT_MAX}]`);
  }
  for (const [name, value] of [
    ["sinceSeq", input.sinceSeq],
    ["beforeSeq", input.beforeSeq],
  ] as const) {
    if (value !== undefined && (!Number.isInteger(value) || value < 0)) {
      throw new RangeError(`undo: ${name} must be a non-negative integer journal seq`);
    }
  }
  const scope: UndoScope = input.scope ?? "workspace";
  const budget = input.budget ?? UNDO_LIST_BUDGET_DEFAULT;
  const notes: string[] = [];
  if (scope === "session" && sessionId === "") {
    notes.push(
      "undo: no workbench/sessionId was supplied via _meta — session scope lists only ops " +
        'journaled anonymously; pass a session id or scope: "workspace"',
    );
  }
  // One op beyond the limit, so "there is more" is a measured fact and the
  // cursor it produces is real rather than optimistic.
  const entries = await deps.journal.recent({
    ...(scope === "session" ? { sessionId } : {}),
    limit: limit + 1,
    ...(input.sinceSeq === undefined ? {} : { sinceSeq: input.sinceSeq }),
    ...(input.beforeSeq === undefined ? {} : { beforeSeq: input.beforeSeq }),
  });
  const kept = entries.slice(0, limit);
  if (kept.length === 0) {
    notes.push(`undo: the journal has no ops in ${scope} scope`);
    return finish(v, budget, [], notes);
  }
  const options = {
    diff: input.diff ?? true,
    diffCharCap: JOURNAL_DIFF_CHARS_MAX,
  };
  // Rendered over everything fetched, INCLUDING the probe op: what the cut
  // reports as dropped is then a count of real rows, not an estimate.
  const rows = journalRows(entries, options);
  const keptSeqs = new Set(kept.map((entry) => entry.op.seq));
  const withinLimit = rows.filter((row) => keptSeqs.has(row.seq)).length;

  // Budget next (a diff-carrying row is large). Rows are cut on op
  // boundaries, so a listed op is never half-shown.
  const charBudget = CHARS_PER_TOKEN * budget;
  let chars = 0;
  let keep = withinLimit;
  for (let i = 0; i < withinLimit; i++) {
    chars += JSON.stringify(rows[i]).length + 1;
    if (chars > charBudget) {
      const seq = rows[i]?.seq;
      const opStart = rows.findIndex((row) => row.seq === seq);
      // Never return zero rows: one over-budget op is better than a silent
      // empty log, and the note says the budget was the reason.
      keep = opStart <= 0 ? rows.filter((row) => row.seq === seq).length : opStart;
      break;
    }
  }
  const overBudget = keep < withinLimit;
  const lastSeq = rows[keep - 1]?.seq ?? kept[kept.length - 1]?.op.seq ?? 0;
  const cut = truncateResults(rows, keep, {
    reason: overBudget ? "budget" : "ceiling",
    narrow: ["limit", "diff:false", `beforeSeq:${lastSeq}`],
    cursor: `beforeSeq:${lastSeq}`,
  });
  if (overBudget) {
    notes.push(
      `undo: the listing was cut to fit the ${budget}-token budget — continue with ` +
        `beforeSeq: ${lastSeq}, or pass diff: false for a leaner log`,
    );
  } else if (cut.truncation !== undefined) {
    notes.push(`undo: older ops remain — continue with beforeSeq: ${lastSeq}`);
  }
  return okEnvelope({
    v,
    freshness: "warm",
    budget: {
      requested: budget,
      used: Math.ceil(JSON.stringify(cut.kept).length / CHARS_PER_TOKEN),
    },
    results: cut,
    ...(notes.length === 0 ? {} : { notes }),
  });
}
