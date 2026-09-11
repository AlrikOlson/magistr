/**
 * Pure-TS unified diff (SDD §3) — diffs are a public output format: the
 * write engine's dry-run preview, its applied-diff report, and the
 * PARSE_REJECTED payload all speak this dialect. Myers' O(ND) greedy walk
 * over lines, rendered as standard unified hunks, kept dependency-light on
 * purpose. The output is a report format first; it is patch-shaped, but
 * nothing in the server ever re-applies one.
 */

import { contentBytes, contentEqual, contentHash, type FileContent } from "./file-content";

const DIFF_CONTEXT_DEFAULT = 3;
// Hard bounds on the quadratic search, independent of machine speed. The
// fallback is linear, complete, and explicitly identified in the receipt.
const DIFF_TRACE_BYTES_MAX = 8 * 1024 * 1024;
const DIFF_SEARCH_STEPS_MAX = 1_000_000;

const NO_NEWLINE_MARKER = "\\ No newline at end of file";

type EditKind = "eq" | "del" | "ins";

interface Edit {
  readonly kind: EditKind;
  /** The line's text, newline-free. */
  readonly text: string;
}

interface SplitContent {
  readonly lines: readonly string[];
  readonly newlineAtEnd: boolean;
}

function splitLines(content: string): SplitContent {
  if (content === "") return { lines: [], newlineAtEnd: true };
  const newlineAtEnd = content.endsWith("\n");
  const lines = content.split("\n");
  if (newlineAtEnd) lines.pop();
  return { lines, newlineAtEnd };
}

/**
 * Myers greedy shortest-edit-script over lines: forward D-path search with a
 * per-D snapshot of the furthest-reaching x per diagonal, then backtracking
 * from the end point. Linear-ish in practice for the mostly-equal inputs a
 * write batch produces.
 */
function editScript(a: readonly string[], b: readonly string[]): Edit[] | null {
  const n = a.length;
  const m = b.length;
  const max = n + m;
  if (max === 0) return [];
  if (n === 0) return b.map((text) => ({ kind: "ins", text }));
  if (m === 0) return a.map((text) => ({ kind: "del", text }));
  const snapshotBytes = (2 * max + 2) * Int32Array.BYTES_PER_ELEMENT;
  if (snapshotBytes > DIFF_TRACE_BYTES_MAX / 2) return null;
  const offset = max;
  const v = new Int32Array(2 * max + 2);
  const trace: Int32Array[] = [];
  let found = false;
  let steps = 0;
  for (let d = 0; d <= max && !found; d++) {
    if ((trace.length + 2) * snapshotBytes > DIFF_TRACE_BYTES_MAX) return null;
    trace.push(v.slice());
    for (let k = -d; k <= d; k += 2) {
      if (++steps > DIFF_SEARCH_STEPS_MAX) return null;
      let x =
        k === -d || (k !== d && (v[offset + k - 1] as number) < (v[offset + k + 1] as number))
          ? (v[offset + k + 1] as number)
          : (v[offset + k - 1] as number) + 1;
      let y = x - k;
      while (x < n && y < m && a[x] === b[y]) {
        if (++steps > DIFF_SEARCH_STEPS_MAX) return null;
        x++;
        y++;
      }
      v[offset + k] = x;
      if (x >= n && y >= m) {
        found = true;
        break;
      }
    }
  }

  const edits: Edit[] = [];
  let x = n;
  let y = m;
  for (let d = trace.length - 1; d >= 0; d--) {
    const vd = trace[d] as Int32Array;
    const k = x - y;
    const prevK =
      k === -d || (k !== d && (vd[offset + k - 1] as number) < (vd[offset + k + 1] as number))
        ? k + 1
        : k - 1;
    const prevX = vd[offset + prevK] as number;
    const prevY = prevX - prevK;
    while (x > prevX && y > prevY) {
      edits.push({ kind: "eq", text: a[x - 1] as string });
      x--;
      y--;
    }
    if (d > 0) {
      if (x === prevX) {
        edits.push({ kind: "ins", text: b[y - 1] as string });
        y--;
      } else {
        edits.push({ kind: "del", text: a[x - 1] as string });
        x--;
      }
    }
  }
  edits.reverse();
  return edits;
}

interface Hunk {
  /** Half-open [start, end) range into the edit script. */
  readonly start: number;
  readonly end: number;
}

/** Change clusters expanded by `context` equal lines, overlapping merged. */
function hunksOf(edits: readonly Edit[], context: number): Hunk[] {
  const hunks: Hunk[] = [];
  let start = -1;
  let lastChange = -1;
  for (let i = 0; i < edits.length; i++) {
    if ((edits[i] as Edit).kind === "eq") continue;
    if (start === -1 || i - lastChange > 2 * context) {
      if (start !== -1) {
        hunks.push({ start, end: Math.min(edits.length, lastChange + context + 1) });
      }
      start = Math.max(0, i - context);
    }
    lastChange = i;
  }
  if (start !== -1) {
    hunks.push({ start, end: Math.min(edits.length, lastChange + context + 1) });
  }
  return hunks;
}

function hunkRange(start: number, count: number): string {
  // Unified format: an empty range names the line *before* the change.
  return count === 0 ? `${start},0` : `${start + 1},${count}`;
}

export interface UnifiedDiffInit {
  /** Workspace-relative path, rendered as `a/<path>` and `b/<path>`. */
  readonly path: string;
  /** null: the file did not exist (creation) — rendered as /dev/null. */
  readonly before: FileContent | null;
  /** null: the file is deleted — rendered as /dev/null. */
  readonly after: FileContent | null;
  /** Equal lines kept around each change; default 3. */
  readonly context?: number;
}

/**
 * Render the unified diff between two file states. Returns "" iff the
 * contents are identical (including both-null). A missing trailing newline
 * is honest: the standard `\ No newline at end of file` marker follows the
 * final rendered line of the side that lacks one.
 */
export function unifiedDiff(init: UnifiedDiffInit): string {
  const { path, before, after } = init;
  const context = init.context ?? DIFF_CONTEXT_DEFAULT;
  if (!Number.isInteger(context) || context < 0) {
    throw new RangeError(`diff context must be a non-negative integer, got ${context}`);
  }
  if (contentEqual(before, after)) return "";

  const fromName = before === null ? "/dev/null" : `a/${path}`;
  const toName = after === null ? "/dev/null" : `b/${path}`;
  if (
    (before !== null && typeof before !== "string") ||
    (after !== null && typeof after !== "string")
  ) {
    const describe = (content: FileContent | null): string =>
      content === null
        ? "absent"
        : `${contentBytes(content).byteLength} bytes, sha256 ${contentHash(content)}`;
    return `Binary files ${fromName} and ${toName} differ\nBefore: ${describe(before)}\nAfter: ${describe(after)}\n`;
  }

  const a = splitLines(before ?? "");
  const b = splitLines(after ?? "");
  const script = editScript(a.lines, b.lines);
  let edits: Edit[] = script ?? [
    ...a.lines.map((text): Edit => ({ kind: "del", text })),
    ...b.lines.map((text): Edit => ({ kind: "ins", text })),
  ];

  // A newline-at-end-only change has an all-equal edit script; surface the
  // real difference by rewriting the final line as a delete + insert pair.
  if (
    edits.length > 0 &&
    edits.every((e) => e.kind === "eq") &&
    a.newlineAtEnd !== b.newlineAtEnd
  ) {
    const last = edits[edits.length - 1] as Edit;
    edits = [
      ...edits.slice(0, -1),
      { kind: "del", text: last.text },
      { kind: "ins", text: last.text },
    ];
  }
  if (edits.every((e) => e.kind === "eq")) return "";

  const out: string[] = [`--- ${fromName}`, `+++ ${toName}`];
  if (script === null)
    out.unshift("# Diff search budget exceeded; showing a complete replacement hunk.");

  // 0-based a/b line offsets consumed before each edit.
  const aPosAt = new Array<number>(edits.length + 1);
  const bPosAt = new Array<number>(edits.length + 1);
  let aPos = 0;
  let bPos = 0;
  edits.forEach((edit, i) => {
    aPosAt[i] = aPos;
    bPosAt[i] = bPos;
    if (edit.kind !== "ins") aPos++;
    if (edit.kind !== "del") bPos++;
  });
  aPosAt[edits.length] = aPos;
  bPosAt[edits.length] = bPos;

  for (const hunk of hunksOf(edits, context)) {
    const aCount = (aPosAt[hunk.end] as number) - (aPosAt[hunk.start] as number);
    const bCount = (bPosAt[hunk.end] as number) - (bPosAt[hunk.start] as number);
    out.push(
      `@@ -${hunkRange(aPosAt[hunk.start] as number, aCount)} ` +
        `+${hunkRange(bPosAt[hunk.start] as number, bCount)} @@`,
    );
    for (let i = hunk.start; i < hunk.end; i++) {
      const edit = edits[i] as Edit;
      const sigil = edit.kind === "eq" ? " " : edit.kind === "del" ? "-" : "+";
      out.push(`${sigil}${edit.text}`);
      const consumesLastA = edit.kind !== "ins" && (aPosAt[i + 1] as number) === a.lines.length;
      const consumesLastB = edit.kind !== "del" && (bPosAt[i + 1] as number) === b.lines.length;
      if ((consumesLastA && !a.newlineAtEnd) || (consumesLastB && !b.newlineAtEnd)) {
        out.push(NO_NEWLINE_MARKER);
      }
    }
  }
  return `${out.join("\n")}\n`;
}
