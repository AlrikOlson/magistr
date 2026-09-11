# Feature Specification: Index Build Without the Quadratic Delete

**Feature Branch**: `017-index-fts-delete`

**Created**: 2026-08-24

**Status**: Draft

**Input**: User description (chunk:index-build-quadratic-fts-delete): "Tier-1 build is O(n²)
in file count: writeFile → deleteFileRows runs `DELETE FROM content_fts WHERE path = ?` on
every file and `path` is an UNINDEXED FTS5 column, so each delete scans the whole FTS table —
rust-lang/rust (62,197 files) reached 17,856 files in 900 s and never hit tier 1; fix the
delete path so a fresh build never scans and an incremental delete is O(log n)."

> Scope note: constitution Development Workflow rule 8 binds a feature spec to exactly one
> roadmap chunk. This spec covers `chunk:index-build-quadratic-fts-delete` only: the engine
> child's index write path and its schema stamp, the tests that pin them, and the re-recorded
> benchmark. It does NOT change any tool, the envelope, the index port, the IPC frames, the
> query side of the index, or what the index stores.

## User Scenarios & Testing *(mandatory)*

### User Story 1 - A large repository reaches tier 1 (Priority: P1)

An agent opens workbench on a repository with tens of thousands of files. Today the tier-1
build slows as it progresses (183 files/s at 2k files, 91 at 4k, 20 over rust-lang/rust)
because every file written first deletes its full-text row by path, and the full-text table
cannot look a path up: each delete reads the whole table. After this feature the per-file
cost does not grow with the index: a fresh build deletes nothing (there is nothing to
delete), and a re-index of one file removes its old rows by row id.

**Why this priority**: the Rust dogfood gate depends on a real Rust workspace indexing at all;
the 016 record shows the largest one does not.

**Independent Test**: `bun run bench:index -- --root D:/Code/_ext/rust` reaches tier 1 within
the 900 s deadline, and the measured rate over the synthetic 500k and 1M profiles no longer
halves when the corpus doubles.

**Acceptance Scenarios**:

1. **Given** a fresh index (no prior rows), **When** the build writes N files, **Then** no
   full-text delete statement runs at all and the write cost per file is independent of N.
2. **Given** an index holding a file, **When** the file is re-indexed (a watcher upsert, a
   changed file on the next build, a query-time re-parse), **Then** its previous symbol,
   import, test-range and full-text rows are gone and the new ones are present — and the
   full-text delete addressed one row by id, not a scan.
3. **Given** an index holding a file, **When** the file is removed (`index/remove`, a vanished
   file found by verification or by the next build's scan), **Then** every row of that file is
   gone, including its full-text row, with the same O(log n) cost.
4. **Given** the synthetic 1M profile, **When** the bench runs, **Then** tier-1 seconds per
   500k LOC (a rate) are within 1.5× the 500k profile's (today 2×: 21.97 s vs 10.98 s).

---

### User Story 2 - An existing index from before the change is not misread (Priority: P1)

The fix relies on a new invariant: a file's full-text row carries the same row id as its
`files` row. An index written before this feature does not satisfy it. After this feature
such an index is recognised by its schema stamp and discarded and rebuilt, as the constitution
allows for cache-class data, rather than adopted and then deleting the wrong rows.

**Independent Test**: open a schema-2 index with the new code; it is rebuilt from the
filesystem and stamped with the new version; queries over it are correct.

**Acceptance Scenarios**:

1. **Given** an index stamped with the previous schema version, **When** the indexer opens
   it, **Then** it is discarded and rebuilt, and the stamp is the new version.
2. **Given** the doctor probe, **When** it reads the new stamp, **Then** it reports it as
   current, not newer-than-known.

---

### User Story 3 - The number is re-recorded (Priority: P2)

The 016 record says tier-1 failed over rust-lang/rust and references were not measured.
After this feature both benches run over the same checkout and the results directory carries
the new index and references reports beside the old failing one's replacement, with the
README table updated.

**Independent Test**: `bench/results/index-rust-lang-rust.json` has `tier1Reached: true`
and `bench/results/refs-rust-lang-rust.json` exists with every gate row carrying `pass`.

**Acceptance Scenarios**:

1. **Given** the fixed indexer, **When** `bench:index -- --root D:/Code/_ext/rust` runs,
   **Then** tier 1 is reached and the tier-1 gate (normalized per 500k LOC) is recorded with
   pass or fail.
2. **Given** tier 1 reached, **When** `bench:refs -- --root D:/Code/_ext/rust` runs, **Then**
   the hot/rare/miss queries sampled from rust-lang/rust's symbols are measured and the
   references p95 gate is recorded with pass or fail.
3. **Given** the synthetic profiles, **When** re-recorded, **Then** their gates still pass and
   the numbers are not worse.

---

### Edge Cases

- A path whose `files` row is missing but stale rows remain in another table (a crash between
  statements is impossible inside one transaction, but a hand-edited index is not): the
  delete-by-row-id path removes nothing for the full-text row; the other four deletes are by
  path as today. No row is ever deleted that belongs to another path.
- Row-id reuse: SQLite may hand a deleted `files` row id to a later insert. The full-text row
  is deleted before the `files` row in the same transaction, so a reused id never inherits
  stale content.
- A file that is indexed without content (binary, oversized, unreadable): no full-text row
  exists; the delete by row id affects nothing.
- Query-time re-parse (`verifyAnswerPaths` → `upsert`) and the build's changed-file path both
  go through the same write, so both benefit.

## Requirements *(mandatory)*

### Functional Requirements

- **FR-001**: The full-text row of a file MUST carry the row id of that file's `files` row,
  established at insert time within the same transaction.
- **FR-002**: Removing a file's rows MUST address the full-text row by row id (looked up
  through the `files` primary key), never by the unindexed path column, and MUST delete the
  full-text row before the `files` row.
- **FR-003**: Writing a file that has no `files` row MUST issue no delete statements at all.
- **FR-004**: The index schema version MUST be bumped so an index written before this feature
  is discarded and rebuilt (cache-class, no migration chain), and the doctor probe MUST accept
  the new stamp.
- **FR-005**: The query side (`content_fts MATCH`, `contentMatches`, references, symbols,
  outline, import graph) MUST be unchanged in behaviour; the `path` column stays on the
  full-text table for the MATCH-time join.
- **FR-006**: A test MUST pin the per-file cost as flat: the per-file cost of writing 4N files
  is under 1.5× the per-file cost of writing N files through the real indexer with the stub
  core (min of three runs), at an N where the old behaviour fails the bound by a wide margin.
- **FR-007**: The 016 benches MUST be re-run over rust-lang/rust and the synthetic profiles and
  the results committed with the README table updated.

### Key Entities

- **Row-id alignment**: the invariant `content_fts.rowid = files.rowid` for every path with
  content.
- **Schema stamp**: `PRAGMA user_version`, 2 → 3.

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: An indexer test shows the per-file write cost flat in index size (per-file cost
  at 4N files under 1.5× that at N) and no full-text delete statement on a fresh build; upsert,
  remove, vanished and changed-file paths leave no stale rows — pinned.
- **SC-002**: A schema-2 index is discarded and rebuilt; the doctor probe accepts schema 3 —
  pinned.
- **SC-003**: `bench:index -- --root D:/Code/_ext/rust` reaches tier 1 within 900 s; the tier-1
  and references gates over rust-lang/rust are recorded (pass or fail) in `bench/results/`.
- **SC-004**: Synthetic 500k / 1M profiles re-recorded; every gate passes; the 1M tier-1 per
  500k LOC is within 1.5× the 500k figure.
- **SC-005**: Full gate sweep green; no query-side behaviour change (existing indexer,
  references, freshness and map tests unchanged and green).

## Assumptions

- The `files` table remains a row-id table (TEXT PRIMARY KEY without `WITHOUT ROWID`), so
  `files.rowid` exists and is stable for the life of the row.
- FTS5 accepts an explicit `rowid` on insert and deletes by `rowid` in O(log n); measured in
  research R1 on this machine.
- The remaining per-file work (parse, four indexed deletes, five inserts) is flat; if the
  re-recorded rate still falls with size, that is a new finding to file, not a reason to
  widen this feature.
