# Implementation Plan: Index Build Without the Quadratic Delete

**Branch**: `017-index-fts-delete` | **Date**: 2026-08-24 | **Spec**: [spec.md](spec.md)

**Input**: Feature specification from `/specs/017-index-fts-delete/spec.md`

## Summary

Align each file's `content_fts` row id with its `files` row id at insert, delete the full-text
row by that id (looked up through the `files` primary key) before the `files` row, and return
early from `deleteFileRows` when the path has no `files` row — so a fresh build issues no
deletes and every incremental delete is O(log n). Bump `INDEX_SCHEMA_VERSION` to 3 so a
schema-2 index is discarded and rebuilt (cache-class, no migration). Pin the cost shape and
the alignment in `test/indexer.test.ts`, then re-record the 016 benches over rust-lang/rust
and the synthetic profiles. Substrate pinned in think:189/190 (R1–R6).

## Technical Context

**Language/Version**: TypeScript (bun); `bun:sqlite` FTS5.

**Primary Dependencies**: `src/adapters/engine-ipc/indexer.ts` (`writeFile`,
`deleteFileRows`, `INDEX_SCHEMA_VERSION`), `src/data/versioned-db.ts` (cache-class discard),
`scripts/bench-*.ts` (re-record).

**Storage**: `.workbench/index.db` schema stamp 2 → 3; table shapes unchanged; a new
invariant `content_fts.rowid = files.rowid`.

**Testing**: `bun test test/indexer.test.ts` (stub core), the migrations and doctor suites,
then the benches with the native core.

**Target Platform**: unchanged.

**Performance Goals**: per-file write cost flat in index size; rust-lang/rust reaches tier 1
inside 900 s; the 1M synthetic profile within 1.5× the 500k figure per 500k LOC.

**Constraints**: no query-side behaviour change; no port / IPC / envelope change; the
`content_fts.path` column stays for the MATCH-time join.

**Scale/Scope**: ~20 lines in one source file, one constant, one test block, re-recorded
JSON and README rows.

## Constitution Check

*GATE: evaluated pre-Phase-0, re-checked post-design — PASS.*

- **I (grep's cost structure)**: this is the principle the finding violated — a 62k-file
  repository never became useful at tier 1; the fix restores "500k LOC to tier 1 in under
  60 s" as a rate that holds at scale.
- **V (versioned contracts)**: the index is cache-class; the stamp bumps and the discard rule
  already in place handles the old shape.
- **VI (no number, no merge)**: the acceptance is the re-recorded bench.

## Project Structure

### Documentation (this feature)

```text
specs/017-index-fts-delete/
├── plan.md
├── research.md           # R1–R6
├── data-model.md
├── quickstart.md
├── checklists/requirements.md
└── tasks.md
```

### Source Code (repository root)

```text
src/adapters/engine-ipc/indexer.ts   # writeFile insert with rowid; deleteFileRows by rowid, early return; version 3
test/indexer.test.ts                 # cost-shape + alignment + no-stale-rows tests; stamp expectations follow the constant
bench/results/*.json, bench/README.md   # re-recorded
```

## Deviations

Recorded during the build unit.

1. **The cost-shape bound was restated** (R5 / FR-006 / SC-001 originally said "2N files under
   1.5× N files"). A linear build doubles its total time when the files double; the flat
   quantity is the per-file cost. The test now asserts the per-file cost at 4N under 1.5× the
   per-file cost at N (N = 2,000, min of three runs). Measured after the fix: ~1.15× per file
   for 4× the files (12–13k files/s flat from 2k to 16k with the stub core); the old code
   measured ~13×.
2. **N = 2,000, not 4,000**: at 4,000 vs 16,000 files the three-run fixtures (60k files) made
   the suite's `afterEach` removal time out on Windows; the fixtures are now removed inside
   the test and N halved. The separation from the old behaviour is still an order of
   magnitude.
3. **Timing noise is real at this scale**: single sub-second builds vary by up to 2× between
   runs (GC, FTS5 segment merges); the minimum of three runs is the cost floor the shape is
   about, and the test says so.
4. **The references gate over rust-lang/rust FAILS after the fix** (US3 scenario 2 says
   "recorded with pass or fail"; it is fail): p95 342.57 ms against 50 ms — hot names ~300 ms,
   single-definition names ~36 ms, the miss 2.9 ms. Recorded as data and filed as
   `chunk:references-p95-real-repo` (added to the Rust dogfood gate's deps); attribution is that
   chunk's first task, not guessed here.
5. **The recorded JSON's `workbenchCommit` is the parent commit** (`5b7364c`): the runs were
   taken on the working tree before the commit that landed them. The README says so.
