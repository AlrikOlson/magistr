/**
 * The journal as rows (SDD §9.8, §7.6) — the read half of `undo`.
 *
 * `undo action:"list"` and the lock-free `workbench-mcp journal` CLI both
 * render the log through this one pure function, so the served answer and
 * the beside-a-live-server answer cannot drift into two shapes.
 *
 * One row per file an op touched, every row carrying its op's `seq`: the seq
 * is what `undo ops:[…]` names, so a reader never has to invent an id or
 * count backwards from "the newest" to aim a rewind. An op that changed no
 * file — the bookkeeping op an undo appends when the rewind lands on
 * identical bytes — still gets its row, with no `path`, because history that
 * silently omits an entry is worse than history with a thin one.
 *
 * For text rows `parseOk` records admission through the write pipeline: the
 * §7.5 choke point rejects a write that introduces a parse error BEFORE it
 * reaches the journal (constitution III), so a journaled op is a
 * parse-verified op. The field exists because a Changes view wants to show
 * it; its value is a property of the pipeline, and the note says so. Binary
 * rows omit it because opaque bytes are never parsed.
 */

import { unifiedDiff } from "../envelope";
import type { JournalFileKind, JournalLogEntry } from "../ports";

/** One journaled file change, as a reader sees it. */
export interface JournalRow {
  readonly kind: "op";
  /** The op's journal id — what `undo ops:[seq]` names. */
  readonly seq: number;
  /** Milliseconds since the Unix epoch, journal-assigned. */
  readonly at: number;
  /** The tool that originated the op: edit, files, script, write, undo. */
  readonly tool: string;
  /** The session that owns the op; "" is the anonymous identity. */
  readonly sessionId: string;
  /** Whether `undo ops:[seq]` would still accept this op. */
  readonly undoable: boolean;
  /** The undo-op that consumed this one; absent while it stands. */
  readonly undoneBy?: number;
  /** Set when this op IS an undo: the seqs it reverted. */
  readonly undoOf?: readonly number[];
  /** The file, workspace-relative; absent on an op that touched none. */
  readonly path?: string;
  readonly change?: JournalFileKind;
  readonly added: number;
  readonly removed: number;
  /** Text pipeline admission — absent for opaque binary content. */
  readonly parseOk?: boolean;
  readonly binary?: true;
  /** The journal blob fell to the §7.6 size cap: history, not a rewind. */
  readonly evicted?: true;
  /** Unified diff body; absent when `diff:false`, or when the blob is gone. */
  readonly diff?: string;
  /** The diff was cut at the per-row character cap. */
  readonly diffTruncated?: true;
}

export interface JournalViewOptions {
  /** Render each row's unified diff. */
  readonly diff: boolean;
  /** Per-row diff character cap; a longer diff is cut and flagged. */
  readonly diffCharCap: number;
}

export const JOURNAL_DIFF_CHARS_MAX = 20_000;

/** Lines a diff would apply: its `+`/`-` rows (file headers excluded). */
function changedLines(diff: string): { added: number; removed: number } {
  let added = 0;
  let removed = 0;
  for (const line of diff.split("\n")) {
    if (line.startsWith("+++") || line.startsWith("---")) continue;
    if (line.startsWith("+")) added += 1;
    else if (line.startsWith("-")) removed += 1;
  }
  return { added, removed };
}

/**
 * Render journal entries (newest first, as the port returns them) as rows.
 * Pure: same entries and options → same rows.
 */
export function journalRows(
  entries: readonly JournalLogEntry[],
  options: JournalViewOptions,
): readonly JournalRow[] {
  const rows: JournalRow[] = [];
  for (const entry of entries) {
    const op = entry.op;
    const common = {
      kind: "op",
      seq: op.seq,
      at: op.at,
      tool: op.tool,
      sessionId: op.sessionId,
      undoable: entry.undoable,
      ...(entry.undoneBy === null ? {} : { undoneBy: entry.undoneBy }),
      ...(op.undoOf === undefined ? {} : { undoOf: op.undoOf }),
    } as const;
    if (op.files.length === 0) {
      rows.push({ ...common, parseOk: true, added: 0, removed: 0 });
      continue;
    }
    for (const file of op.files) {
      // "" is a real stored value (a creation's before, a deletion's after);
      // null means the blob was evicted. Only the second is unrenderable.
      const evicted =
        (file.kind !== "delete" && file.after === null) ||
        (file.kind !== "create" && file.before === null);
      if (evicted) {
        rows.push({
          ...common,
          path: file.path,
          change: file.kind,
          added: 0,
          removed: 0,
          evicted: true,
        });
        continue;
      }
      const body = unifiedDiff({
        path: file.path,
        before: file.kind === "create" ? null : file.before,
        after: file.kind === "delete" ? null : file.after,
      });
      const counts = changedLines(body);
      const binary =
        (file.before !== null && typeof file.before !== "string") ||
        (file.after !== null && typeof file.after !== "string");
      const cut = body.length > options.diffCharCap;
      rows.push({
        ...common,
        ...(binary ? { binary: true as const } : { parseOk: true }),
        path: file.path,
        change: file.kind,
        added: counts.added,
        removed: counts.removed,
        ...(options.diff
          ? {
              diff: cut ? body.slice(0, options.diffCharCap) : body,
              ...(cut ? { diffTruncated: true } : {}),
            }
          : {}),
      });
    }
  }
  return rows;
}
