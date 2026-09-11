/**
 * WriteLock — the per-workspace mutation serialization seam (SDD §7.7).
 *
 * The write engine (§7.5) runs its whole pipeline inside `withLock`, making
 * the workspace the concurrency domain: mutations from any entry point — a
 * second Workbench process, the CLI — serialize here, with the journal as
 * the serialization witness. Content-hash anchors (STALE_ANCHOR) close the
 * read-modify-write races the lock does not.
 *
 * Production adapter is the advisory lock file in src/adapters/workspace-lock;
 * tests may pass a pass-through. An adapter that cannot acquire within its
 * bound must throw an error carrying `code: "ENGINE_BUSY"` and a suggested
 * `backoffMs` — never hang (§7.7 typed backpressure).
 */

export interface WriteLock {
  withLock<T>(fn: () => Promise<T>): Promise<T>;
}
