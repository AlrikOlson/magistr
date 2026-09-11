# Tasks: Index Build Without the Quadratic Delete

**Input**: Design documents from `specs/017-index-fts-delete/` (plan.md, research.md R1–R6,
data-model.md, quickstart.md). Substrate pinned in think:189/190.

**Tests**: the cost-shape and alignment tests are written first and fail on the current code
(the ratio test measures ~13× per 4× files today).

**Organization**: phases map to the spec's user stories; execute in order.

## Phase 1: User Story 1 — the write path (P1) 🎯 MVP

- [x] T001 [US1] `test/indexer.test.ts`: the cost-shape test (N vs 4N files through
      `ChildIndexer` with the stub core, per-file cost at 4N under 1.5× that at N, min of three runs, N = 2,000) and the
      alignment/no-stale-rows test over build → upsert → remove → vanished → changed
      (`content_fts.rowid = files.rowid` for every row; one FTS row per content-bearing file;
      none for a removed path). Both fail on the current code (SC-001).
- [x] T002 [US1] `src/adapters/engine-ipc/indexer.ts`: `writeFile` inserts `content_fts`
      with `rowid = (SELECT rowid FROM files WHERE path = ?1)`; `deleteFileRows` looks the
      `files` row id up first, returns when absent, deletes the FTS row by row id before the
      other four deletes (R1, R2, R4) (FR-001–003, FR-005).

## Phase 2: User Story 2 — the stamp (P1)

- [x] T003 [US2] `INDEX_SCHEMA_VERSION = 3`; comment updated; `test/indexer.test.ts` /
      `test/migrations.test.ts` / doctor suite green through the constant (R3) (FR-004).

## Phase 3: User Story 3 — the number (P2)

- [x] T004 [US3] Re-record `bench/results/index-rust-lang-rust.json`,
      `refs-rust-lang-rust.json`, `index-synthetic-500k.json`, `refs-synthetic-500k.json`,
      `index-synthetic-1m.json`, `refs-synthetic-1m.json`; update `bench/README.md` rows and
      the finding section (R6) (FR-007, SC-003, SC-004).

## Phase 4: Polish

- [x] T005 Update `plan.md` Deviations, tick this file, server-verified gate sweep via
      `ship_check` (bun test, tsc, lint quartet, cargo test), `ship_finalize`,
      `roadmap_complete_chunk`, commit (SC-005).
