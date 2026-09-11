# Phase 0 Research: Index Build Without the Quadratic Delete

Grounded in `src/adapters/engine-ipc/indexer.ts` (`runBuild` builds a `stored` map of every
prior `files` row, purges vanished paths through `deleteFileRows`, and hands changed files to
`writeFile`; `writeFile` calls `deleteFileRows` unconditionally then inserts `files`,
`content_fts`, `symbols`, `imports`, `test_ranges`; `deleteFileRows` runs five DELETEs of
which four hit indexes (`symbols_path`, `test_ranges_path`, `imports_path`, the `files`
primary key) and one — `DELETE FROM content_fts WHERE path = ?` — filters an `UNINDEXED` FTS5
column; `upsert` and `remove` reuse the same two helpers; `verifyAnswerPaths` calls `remove`
for vanished files and `upsert` for changed ones; `INDEX_SCHEMA_VERSION = 2` stamped as
`PRAGMA user_version` through `openVersionedDatabase({ dataClass: "cache", migrations: [] })`,
which discards and rebuilds any database no chain reaches), `src/doctor/probes.ts` (reports
`userVersion` and flags only `> INDEX_SCHEMA_VERSION`), `test/indexer.test.ts` (pins the stamp
and that a version-1 database is discarded), `test/migrations.test.ts`, the 016 record
(`bench/results/index-rust-lang-rust.json`, README) and two in-memory probes run this unit.
Pinned in think:189/190.

## R1 — Address the full-text row by row id, aligned to `files.rowid`

**Decision**: insert the full-text row with an explicit row id equal to the file's `files`
row id — `INSERT INTO content_fts (rowid, path, content) VALUES ((SELECT rowid FROM files
WHERE path = ?1), ?1, ?2)` after the `files` insert in the same transaction — and delete it
with `DELETE FROM content_fts WHERE rowid = (SELECT rowid FROM files WHERE path = ?)` before
the `files` row is deleted. The `path` column stays (`UNINDEXED`) because the MATCH-time
queries read it back.

**Measured** (in-memory SQLite, this machine, 64,000 rows): delete by path 19.84 ms each;
delete by row id 0.010 ms each; inserting 64,000 `files` + aligned `content_fts` rows in one
transaction 0.33 s; the alignment holds (`files.rowid = content_fts.rowid` for a sampled path)
and MATCH still answers `path`. Earlier probe (chunk description): by-path delete 0.23 ms at
2k rows → 9.8 ms at 32k, linear in rows.

**Rationale**: FTS5 tables are row-id tables; row id lookup is the one O(log n) access they
offer without a second index. Aligning to `files.rowid` costs nothing (the `files` insert
precedes the FTS insert already) and needs no new column.

**Rejected**: an external-content FTS5 table (`content='files'`) — `files` does not store
content, and moving content there doubles the schema change for the same win; a separate
`path → rowid` map table — one more table to keep consistent for no gain over the `files`
primary key; making `path` an indexed FTS column — it would tokenise paths into the term
index and change MATCH semantics.

## R2 — A path with no `files` row has nothing to delete

**Decision**: `deleteFileRows` first reads `SELECT rowid FROM files WHERE path = ?` (primary
key, O(log n)); when there is no row it returns without issuing any DELETE. Every table is
written under one transaction keyed by the `files` row, so a path without a `files` row has no
other rows either. A fresh build therefore runs zero deletes; `runBuild`'s `stored` map is left
as it is (the lookup is cheaper than threading a flag through `FileWork`).

**Rationale**: one helper, every caller (`writeFile`, the build's vanished purge, `upsert`,
`remove`) gets the fast path; the invariant is checked where it is relied on.

## R3 — Schema stamp 2 → 3, no migration chain

**Decision**: `INDEX_SCHEMA_VERSION = 3`. A schema-2 index has arbitrary `content_fts` row ids;
adopting it would make R1's delete remove the wrong row or none. The existing cache-class rule
(specs/009 R7) already discards and rebuilds any database no chain reaches, so no migration
code is written. The doctor probe compares `userVersion > INDEX_SCHEMA_VERSION` and needs no
change; `test/indexer.test.ts` and `test/migrations.test.ts` read the constant.

**Rationale**: SDD §10.2 — cache-class data is discarded and rebuilt rather than misread.

## R4 — Order inside `deleteFileRows`

**Decision**: full-text row first (it needs the `files` row id), then `symbols`, `test_ranges`,
`imports`, then `files`. SQLite may reuse a deleted row id for a later insert; deleting the FTS
row before its `files` row means a reused id never points at stale content.

## R5 — The proof is a cost-shape test, not a timing threshold

**Decision**: `test/indexer.test.ts` gains a test that builds N and 4N small files through the
real `ChildIndexer` with the stub core (min of three runs each) and asserts the per-file cost
at 4N is under 1.5× the per-file cost at N, N = 2,000 (a linear build costs ~4× for 4× the
files; the old code measured ~13× here),
plus a statement-level assertion: a fresh build issues no `DELETE FROM content_fts` (observed
through `db.query` tracing is not available in bun:sqlite, so the test asserts the effect:
after a fresh build `content_fts` has exactly one row per content-bearing file and
`files.rowid = content_fts.rowid` for every row). Upsert / remove / vanished / changed paths
assert no stale rows by path and the alignment after each mutation.

**Rationale**: a ratio between two runs in the same process is stable across machines; an
absolute millisecond bound is not.

## R6 — Re-record 016's benches and replace the failing record

**Decision**: after the fix, re-run `bench:index` and `bench:refs` over `D:/Code/_ext/rust`
(same checkout, `da51146`) and the synthetic 500k / 1M profiles, overwrite the JSON under
`bench/results/`, and update the README table and its "what the run found" section to say the
finding is fixed and by how much. The IPC record is unaffected and stays.

**Rationale**: the acceptance of the previous chunk was the number; the acceptance of this one
is the number moving.
