/**
 * JournalPort — the durable, versioned, append-only op log (SDD §6.4, §7.6).
 *
 * Powers `undo`; per-workspace. One op is one write-engine batch — §10.1's
 * `ops(op_id, ts, tool, args_hash, session_id, files[], …)` — so
 * `undo(steps: 1)` means "one mutation", not "one file". The port assigns
 * sequence numbers; callers never invent them.
 *
 * Identity travels on the request, state lives on disk (§5.1): every op is
 * tagged with the client/session identity supplied via `_meta`, and the
 * session-scoped undo default reads exactly that tag back.
 */

import type { FileContent } from "../envelope";
import type { PORT_VERSIONS } from "./versions";

/** What one op did to one file. `create` had no before; `delete` no after. */
export type JournalFileKind = "create" | "write" | "delete";

export interface JournalFileChange {
  readonly kind: JournalFileKind;
  readonly path: string;
  /** Full before-content; "" for `create`. */
  readonly before: FileContent;
  /** Full after-content; "" for `delete`. */
  readonly after: FileContent;
}

export interface JournalOpInit {
  /** Originating tool (edit, files, script, undo — §7.5). */
  readonly tool: string;
  /** sha256 of the originating tool's arguments. */
  readonly argsHash: string;
  /** Client/session identity from `_meta`; "" is the anonymous identity. */
  readonly sessionId: string;
  readonly files: readonly JournalFileChange[];
  /** Set when this op IS an undo: the seqs it reverted. Ops named here stop
   * being undo candidates, and this op never becomes one (no redo, §7.6). */
  readonly undoOf?: readonly number[];
}

export interface JournalFileRecord {
  readonly kind: JournalFileKind;
  readonly path: string;
  /** sha256 of the uncompressed before-content (the CAS blob id, §10.1). */
  readonly beforeHash: string;
  readonly afterHash: string;
  /** null when the blob was evicted under the size cap (§7.6) — the op is
   * still visible in the log but no longer undoable. */
  readonly before: FileContent | null;
  readonly after: FileContent | null;
}

export interface JournalOp {
  /** Monotonic, journal-assigned. */
  readonly seq: number;
  /** Milliseconds since the Unix epoch, journal-assigned. */
  readonly at: number;
  readonly tool: string;
  readonly argsHash: string;
  readonly sessionId: string;
  readonly undoOf?: readonly number[];
  readonly files: readonly JournalFileRecord[];
}

export interface JournalAppendResult {
  readonly op: JournalOp;
  /** Degradations to surface on the envelope — §7.6 requires oldest-blob
   * eviction to carry a note, never happen silently. */
  readonly notes: readonly string[];
}

export interface UndoCandidateQuery {
  /** Restrict to one session's ops (§7.6 session scope); absent = workspace. */
  readonly sessionId?: string;
  readonly limit: number;
  /** Restrict to these op seqs — id-addressed undo. Eligibility still
   * decides: a seq named here that is an undo-op, already undone, or outside
   * the session scope simply does not come back. Absent = the newest ops. */
  readonly seqs?: readonly number[];
}

export interface JournalLogQuery {
  /** Restrict to one session's ops; absent = the whole workspace log. */
  readonly sessionId?: string;
  /** How many ops to return, newest first. */
  readonly limit: number;
  /** Only ops with `seq >= sinceSeq`; 0 means the whole log. */
  readonly sinceSeq?: number;
  /** Only ops with `seq < beforeSeq` — the pagination cursor. */
  readonly beforeSeq?: number;
}

/** One op as the log shows it: the record plus the two facts an undo
 * candidate has and a log entry needs spelled out — whether it is still
 * undoable, and which op consumed it if not. */
export interface JournalLogEntry {
  readonly op: JournalOp;
  /** True when this op is still an undo candidate (not an undo-op itself,
   * not already undone). Blob eviction is visible on the files. */
  readonly undoable: boolean;
  /** The undo-op that consumed this one; null when it stands. */
  readonly undoneBy: number | null;
}

export interface JournalPort {
  readonly version: typeof PORT_VERSIONS.journal;
  append(op: JournalOpInit): Promise<JournalAppendResult>;
  /** Ops with `seq >= sinceSeq` in ascending order; 0 means the whole log. */
  list(sinceSeq: number): Promise<readonly JournalOp[]>;
  /** The log as a reader sees it — newest first, bounded, with each op's
   * undo eligibility resolved. `list` is the raw append order; this is what
   * a Changes view tails. */
  recent(query: JournalLogQuery): Promise<readonly JournalLogEntry[]>;
  get(seq: number): Promise<JournalOp | null>;
  /** The newest ops still eligible for undo — not undo-ops themselves and
   * not already undone — newest first, session-filtered when asked. */
  undoCandidates(query: UndoCandidateQuery): Promise<readonly JournalOp[]>;
}
